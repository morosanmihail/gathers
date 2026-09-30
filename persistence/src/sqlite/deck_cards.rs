use models::{CardAvailability, CardHolding, CardID, CollectionID, DeckCard, DeckMeta};
use rusqlite::{Connection, OptionalExtension, params};

/// Changes one card's quantity on one board of a deck.
///
/// Same change model as `cards::add_cards`: positive adds, negative removes, the result is floored
/// at 0, and a row reaching 0 is deleted. A change to a board the card isn't on yet is floored
/// before insert, so removing from nothing is a no-op rather than a negative ghost row.
///
/// Never touches `cards`: allocating a card to a deck doesn't change how many copies are owned.
#[allow(clippy::too_many_arguments)]
pub(super) fn adjust_card(
    conn: &Connection,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    board: &str,
    quantity: i32,
    time_added: &str,
    provider: &str,
) -> eyre::Result<Option<DeckCard>> {
    // optional()? rather than ok(): a genuine query failure must not be indistinguishable from "no
    // such row", which would take the no-row branch and report a removal as done
    let existing: Option<i32> = conn
        .query_row(
            "SELECT quantity FROM deck_cards WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
            params![deck, uuid, finish, board],
            |r| r.get(0),
        )
        .optional()?;

    let delta = match existing {
        // No row yet: flooring here rather than in SQL keeps the ON CONFLICT branch able to compute
        // existing + delta un-clamped
        None => quantity.max(0),
        Some(_) => quantity,
    };

    if existing.is_none() && delta == 0 {
        return Ok(None);
    }

    conn.execute(
        "INSERT INTO deck_cards (deck, uuid, finish, board, quantity, provider, timeadded, timeupdated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT (deck, uuid, finish, board) DO UPDATE SET
           -- Saturating at both ends, as cards::add_cards does. SQLite stores the 64-bit sum
           -- happily and every read of this column is an i32, so one overflowing row would make
           -- the deck unreadable and unrepairable through the API
           quantity = max(min(deck_cards.quantity + EXCLUDED.quantity, 2147483647), 0),
           timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        params![deck, uuid, finish, board, delta, provider, time_added],
    )?;

    conn.execute(
        "DELETE FROM deck_cards WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4 AND quantity = 0",
        params![deck, uuid, finish, board],
    )?;

    get_card(conn, deck, uuid, finish, board)
}

pub(super) fn get_card(
    conn: &Connection,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    board: &str,
) -> eyre::Result<Option<DeckCard>> {
    let mut stmt = conn.prepare(
        "SELECT deck, uuid, finish, board, quantity, provider, timeadded, timeupdated
         FROM deck_cards WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
    )?;
    let card = stmt
        .query_map(params![deck, uuid, finish, board], row_to_deck_card)?
        .next()
        .transpose()?;
    Ok(card)
}

/// Every card in a deck, across all boards.
///
/// Decks are bounded lists (40 cards for Riftbound, 60 for MTG), so this returns the whole thing
/// rather than paginating; the caller splits by board for display.
pub(super) fn list_cards(conn: &Connection, deck: &CollectionID) -> eyre::Result<Vec<DeckCard>> {
    let mut stmt = conn.prepare(
        "SELECT deck, uuid, finish, board, quantity, provider, timeadded, timeupdated
         FROM deck_cards WHERE deck = ?1 ORDER BY board, uuid, finish",
    )?;
    let cards = stmt
        .query_map(params![deck], row_to_deck_card)?
        .collect::<Result<_, _>>()?;
    Ok(cards)
}

/// Copies in the deck, summed across every board and finish.
///
/// Counts copies rather than distinct cards, unlike a collection's count: a deck is described by
/// how many cards are in it, and the 60 of a Standard deck counts four Lightning Bolts as four.
pub(super) fn count_cards(conn: &Connection, deck: &CollectionID) -> eyre::Result<i64> {
    let total = conn.query_row(
        "SELECT COALESCE(SUM(quantity), 0) FROM deck_cards WHERE deck = ?1",
        params![deck],
        |r| r.get(0),
    )?;
    Ok(total)
}

/// Moves copies of a card from one board of a deck to another.
///
/// Both sides are adjusted in one transaction, so a move can't half-apply and duplicate or lose
/// copies. Moving more than the source board holds moves what's there.
pub(super) fn move_between_boards(
    conn: &mut Connection,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    from_board: &str,
    to_board: &str,
    quantity: i32,
) -> eyre::Result<()> {
    if from_board == to_board || quantity <= 0 {
        return Ok(());
    }

    let tx = conn.transaction()?;
    // An absent source row is a legitimate no-op; a query failure is not, and collapsing both to
    // "the board holds 0 copies" would report a move as done when the read never succeeded
    let Some((available, provider, time_added)) = tx
        .query_row(
            "SELECT quantity, provider, COALESCE(timeadded, '') FROM deck_cards
             WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
            params![deck, uuid, finish, from_board],
            |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(());
    };

    let moved = quantity.min(available);
    if moved <= 0 {
        return Ok(());
    }

    adjust_card(&tx, deck, uuid, finish, from_board, -moved, &time_added, &provider)?;
    adjust_card(&tx, deck, uuid, finish, to_board, moved, &time_added, &provider)?;
    tx.commit()?;
    Ok(())
}

pub(super) fn get_meta(conn: &Connection, deck: &CollectionID) -> eyre::Result<DeckMeta> {
    let meta = conn
        .query_row(
            "SELECT deck, built, location, notes FROM deck_meta WHERE deck = ?1",
            params![deck],
            |r| {
                Ok(DeckMeta {
                    deck: r.get(0)?,
                    built: r.get::<_, i64>(1)? != 0,
                    location: r.get(2)?,
                    notes: r.get(3)?,
                })
            },
        )
        // No row means "not built": deck_meta is populated lazily, so absence is a valid state. A
        // query failure is not: reporting a built deck as unbuilt would have the UI render the
        // toggle off, and the next edit would write that back
        .optional()?
        .unwrap_or_else(|| DeckMeta {
            deck: deck.clone(),
            ..Default::default()
        });
    Ok(meta)
}

pub(super) fn set_meta(
    conn: &Connection,
    deck: &CollectionID,
    built: bool,
    location: Option<&str>,
    notes: Option<&str>,
) -> eyre::Result<DeckMeta> {
    conn.execute(
        "INSERT INTO deck_meta (deck, built, location, notes) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (deck) DO UPDATE SET built = ?2, location = ?3, notes = ?4",
        params![deck, i64::from(built), location, notes],
    )?;
    get_meta(conn, deck)
}

/// Where the copies of each requested card currently are.
///
/// `owned` sums `cards` across every collection; `committed` sums `deck_cards` across decks whose
/// `deck_meta.built` is set. An unbuilt deck is only a list, so it commits nothing.
///
/// Aggregated per `uuid` rather than per `(uuid, finish)`: a foil copy in a built deck still
/// locates a playable one. `finish` and `board` stay on each holding as detail.
pub(super) fn card_availability(
    conn: &Connection,
    uuids: &[CardID],
) -> eyre::Result<Vec<CardAvailability>> {
    if uuids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = (1..=uuids.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");

    let owned: std::collections::HashMap<CardID, i32> = {
        let query = format!(
            "SELECT uuid, COALESCE(SUM(quantity), 0) FROM cards
             WHERE uuid IN ({placeholders}) GROUP BY uuid"
        );
        let mut stmt = conn.prepare(&query)?;
        stmt.query_map(rusqlite::params_from_iter(uuids.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?))
        })?
        .collect::<Result<_, _>>()?
    };

    let mut holdings_by_uuid: std::collections::HashMap<CardID, Vec<CardHolding>> =
        std::collections::HashMap::new();
    {
        let query = format!(
            "SELECT dc.uuid, dc.deck, dm.location, dc.board, dc.finish, dc.quantity
             FROM deck_cards dc
             JOIN deck_meta dm ON dm.deck = dc.deck
             WHERE dc.uuid IN ({placeholders}) AND dm.built != 0
             ORDER BY dc.deck, dc.board, dc.finish"
        );
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(uuids.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                CardHolding {
                    deck: r.get(1)?,
                    location: r.get(2)?,
                    board: r.get(3)?,
                    finish: r.get(4)?,
                    quantity: r.get(5)?,
                },
            ))
        })?;
        for row in rows {
            let (uuid, holding) = row?;
            holdings_by_uuid.entry(uuid).or_default().push(holding);
        }
    }

    Ok(uuids
        .iter()
        .map(|uuid| {
            let holdings = holdings_by_uuid.remove(uuid).unwrap_or_default();
            let owned = owned.get(uuid).copied().unwrap_or(0);
            let committed: i32 = holdings.iter().map(|h| h.quantity).sum();
            CardAvailability {
                uuid: uuid.clone(),
                owned,
                committed,
                free: owned - committed,
                over_committed: committed > owned,
                holdings,
            }
        })
        .collect())
}

fn row_to_deck_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeckCard> {
    Ok(DeckCard {
        deck: row.get(0)?,
        uuid: row.get(1)?,
        finish: row.get(2)?,
        board: row.get(3)?,
        quantity: row.get(4)?,
        provider: row.get(5)?,
        time_added: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        time_updated: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
    })
}
