use models::{
    CardAvailability, CardCopies, CardHolding, CardID, CollectionID, DEFAULT_BOARDS, DeckBoard,
    DeckCard, DeckMeta,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::BoardRenameResult;

/// Changes one card's owned and wanted counts on one board of a deck.
///
/// Same change model as `cards::add_cards`: positive adds, negative removes, each count is floored
/// at 0, and a row left with neither is deleted. A change to a board the card isn't on yet is
/// floored before insert, so removing from nothing is a no-op rather than a negative ghost row.
///
/// Declares the board if the deck hasn't got one, so a free-form name can be used without creating
/// it first.
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
    want_quantity: i32,
    time_added: &str,
    provider: &str,
) -> eyre::Result<Option<DeckCard>> {
    // optional()? rather than ok(): a genuine query failure must not be indistinguishable from "no
    // such row", which would take the no-row branch and report a removal as done
    let existing: Option<(i32, i32)> = conn
        .query_row(
            "SELECT quantity, want_quantity FROM deck_cards
             WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
            params![deck, uuid, finish, board],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let (delta, want_delta) = match existing {
        // No row yet: flooring here rather than in SQL keeps the ON CONFLICT branch able to compute
        // existing + delta un-clamped
        None => (quantity.max(0), want_quantity.max(0)),
        Some(_) => (quantity, want_quantity),
    };

    if existing.is_none() && delta == 0 && want_delta == 0 {
        return Ok(None);
    }

    ensure_board(conn, deck, board)?;

    conn.execute(
        "INSERT INTO deck_cards (deck, uuid, finish, board, quantity, want_quantity, provider, timeadded, timeupdated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT (deck, uuid, finish, board) DO UPDATE SET
           -- Saturating at both ends, as cards::add_cards does. SQLite stores the 64-bit sum
           -- happily and every read of this column is an i32, so one overflowing row would make
           -- the deck unreadable and unrepairable through the API
           quantity = max(min(deck_cards.quantity + EXCLUDED.quantity, 2147483647), 0),
           want_quantity = max(min(deck_cards.want_quantity + EXCLUDED.want_quantity, 2147483647), 0),
           timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        params![deck, uuid, finish, board, delta, want_delta, provider, time_added],
    )?;

    conn.execute(
        "DELETE FROM deck_cards
         WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4
           AND quantity = 0 AND want_quantity = 0",
        params![deck, uuid, finish, board],
    )?;

    get_card(conn, deck, uuid, finish, board)
}

/// Adds a board to a deck unless it already has one by that name, placing it last.
pub(super) fn ensure_board(
    conn: &Connection,
    deck: &CollectionID,
    board: &str,
) -> eyre::Result<()> {
    conn.execute(
        "INSERT INTO deck_boards (deck, board, position)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position) + 1, 0) FROM deck_boards WHERE deck = ?1))
         ON CONFLICT (deck, board) DO NOTHING",
        params![deck, board],
    )?;
    Ok(())
}

/// Gives a deck the boards it starts life with.
pub(super) fn create_default_boards(conn: &Connection, deck: &CollectionID) -> eyre::Result<()> {
    for board in DEFAULT_BOARDS {
        ensure_board(conn, deck, board)?;
    }
    Ok(())
}

/// A deck's boards, in display order.
pub(super) fn list_boards(conn: &Connection, deck: &CollectionID) -> eyre::Result<Vec<DeckBoard>> {
    let mut stmt = conn.prepare(
        "SELECT deck, board, position FROM deck_boards WHERE deck = ?1 ORDER BY position, board",
    )?;
    let boards = stmt
        .query_map(params![deck], |r| {
            Ok(DeckBoard {
                deck: r.get(0)?,
                board: r.get(1)?,
                position: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(boards)
}

/// Renames one of a deck's boards.
///
/// The cards follow through the board foreign key's ON UPDATE CASCADE rather than a second
/// statement, so they can't be stranded on a name that no longer exists.
///
/// Refuses to merge two boards: renaming onto a name the deck already uses would silently fold one
/// board's cards into another's, and the primary key would reject the collisions anyway.
pub(super) fn rename_board(
    conn: &Connection,
    deck: &CollectionID,
    from: &str,
    to: &str,
) -> eyre::Result<BoardRenameResult> {
    // Trimmed before anything looks at it, so the name checked is the name stored
    let to = to.trim();
    if to.is_empty() {
        return Ok(BoardRenameResult::InvalidName);
    }
    if from == to {
        return Ok(BoardRenameResult::Renamed);
    }

    let exists = |board: &str| -> eyre::Result<bool> {
        Ok(conn
            .query_row(
                "SELECT 1 FROM deck_boards WHERE deck = ?1 AND board = ?2",
                params![deck, board],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    };

    if !exists(from)? {
        return Ok(BoardRenameResult::NoSuchBoard);
    }
    if exists(to)? {
        return Ok(BoardRenameResult::NameTaken);
    }

    conn.execute(
        "UPDATE deck_boards SET board = ?3 WHERE deck = ?1 AND board = ?2",
        params![deck, from, to],
    )?;
    Ok(BoardRenameResult::Renamed)
}

/// Removes a board from a deck, and with it every card on that board.
///
/// The cards go through the board foreign key's ON DELETE CASCADE. A deck is allowed to end up
/// with no boards at all: the next card added declares one.
pub(super) fn delete_board(
    conn: &Connection,
    deck: &CollectionID,
    board: &str,
) -> eyre::Result<bool> {
    let removed = conn.execute(
        "DELETE FROM deck_boards WHERE deck = ?1 AND board = ?2",
        params![deck, board],
    )?;
    Ok(removed > 0)
}

/// Reorders a deck's boards to the given sequence.
///
/// Names the deck doesn't have are ignored, and any board the caller leaves out keeps its place
/// after the ones listed, so a partial order can't drop a board off the deck.
pub(super) fn reorder_boards(
    conn: &mut Connection,
    deck: &CollectionID,
    boards: &[String],
) -> eyre::Result<Vec<DeckBoard>> {
    let tx = conn.transaction()?;

    // The whole deck is renumbered from zero rather than the listed boards being shuffled past
    // the others, so positions stay dense however often a deck is reordered. Nothing here has to
    // dodge a collision: position carries no uniqueness constraint, the key being (deck, board).
    let current = list_boards(&tx, deck)?;
    let mut order: Vec<&str> = Vec::with_capacity(current.len());
    for name in boards.iter().map(String::as_str) {
        // A name the deck hasn't got, or one listed twice, would otherwise take a position and
        // leave a gap where a real board should be
        if current.iter().any(|b| b.board == name) && !order.contains(&name) {
            order.push(name);
        }
    }
    order.extend(
        current
            .iter()
            .map(|b| b.board.as_str())
            .filter(|name| !boards.iter().any(|listed| listed == name)),
    );

    for (position, board) in order.iter().enumerate() {
        tx.execute(
            "UPDATE deck_boards SET position = ?3 WHERE deck = ?1 AND board = ?2",
            params![deck, board, i64::try_from(position)?],
        )?;
    }
    tx.commit()?;
    list_boards(conn, deck)
}

pub(super) fn get_card(
    conn: &Connection,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    board: &str,
) -> eyre::Result<Option<DeckCard>> {
    let mut stmt = conn.prepare(
        "SELECT deck, uuid, finish, board, quantity, want_quantity, provider, timeadded, timeupdated
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
        "SELECT dc.deck, dc.uuid, dc.finish, dc.board, dc.quantity, dc.want_quantity,
                dc.provider, dc.timeadded, dc.timeupdated
         FROM deck_cards dc
         JOIN deck_boards db ON db.deck = dc.deck AND db.board = dc.board
         WHERE dc.deck = ?1 ORDER BY db.position, dc.uuid, dc.finish",
    )?;
    let cards = stmt
        .query_map(params![deck], row_to_deck_card)?
        .collect::<Result<_, _>>()?;
    Ok(cards)
}

/// Slots filled in the deck, across every board and finish.
///
/// Counts copies rather than distinct cards, unlike a collection's count: a deck is described by
/// how many cards are in it, and the 60 of a Standard deck counts four Lightning Bolts as four.
///
/// Each row counts the greater of its owned and wanted counts, never the sum: wanting four
/// Tarmogoyf and already holding three of them is four cards of the 60, not seven.
pub(super) fn count_cards(conn: &Connection, deck: &CollectionID) -> eyre::Result<i64> {
    let total = conn.query_row(
        "SELECT COALESCE(SUM(max(quantity, want_quantity)), 0) FROM deck_cards WHERE deck = ?1",
        params![deck],
        |r| r.get(0),
    )?;
    Ok(total)
}

/// Moves copies of a card from one board of a deck to another.
///
/// Both sides are adjusted in one transaction, so a move can't half-apply and duplicate or lose
/// copies. Moving more than the source board holds moves what's there.
///
/// Owned and wanted copies move independently: a board holding one Tarmogoyf and wanting two more
/// can send either across without disturbing the other.
#[allow(clippy::too_many_arguments)]
pub(super) fn move_between_boards(
    conn: &mut Connection,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    from_board: &str,
    to_board: &str,
    quantity: u32,
    want_quantity: u32,
) -> eyre::Result<()> {
    // Either count may be zero while the other moves, so it is the pair that has to say something
    if from_board == to_board || (quantity == 0 && want_quantity == 0) {
        return Ok(());
    }

    let tx = conn.transaction()?;
    // An absent source row is a legitimate no-op; a query failure is not, and collapsing both to
    // "the board holds 0 copies" would report a move as done when the read never succeeded
    let Some((available, want_available, provider, time_added)) = tx
        .query_row(
            "SELECT quantity, want_quantity, provider, COALESCE(timeadded, '') FROM deck_cards
             WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
            params![deck, uuid, finish, from_board],
            |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(());
    };

    let moved = copies_available(quantity, available);
    let want_moved = copies_available(want_quantity, want_available);
    if moved == 0 && want_moved == 0 {
        return Ok(());
    }

    adjust_card(&tx, deck, uuid, finish, from_board, -moved, -want_moved, &time_added, &provider)?;
    adjust_card(&tx, deck, uuid, finish, to_board, moved, want_moved, &time_added, &provider)?;
    tx.commit()?;
    Ok(())
}

/// How many copies a move can actually shift: what was asked for, capped at what is there.
fn copies_available(requested: u32, available: i32) -> i32 {
    // A stored count below zero would be corruption rather than an empty row, so it is floored
    // rather than trusted; a non-negative i32 always fits in a u32
    let available = available.max(0) as u32;
    // Capped before narrowing, so the result is at most a count that came out of an i32 column
    // and fits by construction. Narrowing first would wrap a large request to a negative
    requested.min(available) as i32
}

/// Moves copies of a card out of a collection and onto a board of a deck.
///
/// A transfer, unlike `adjust_card`: the copies leave the collection. Allocating a card to a deck
/// while keeping it in the collection is what `adjust_card` is for, and stays the normal way to
/// build a list; this is for the case where the cards physically move.
///
/// Moving more than the collection holds moves what's there. Returns the number of copies moved.
#[allow(clippy::too_many_arguments)]
pub(super) fn move_from_collection(
    conn: &mut Connection,
    collection: &CollectionID,
    deck: &CollectionID,
    uuid: &CardID,
    finish: &str,
    board: &str,
    quantity: u32,
) -> eyre::Result<i32> {
    let tx = conn.transaction()?;
    let Some((available, provider, time_added)) = tx
        .query_row(
            "SELECT quantity, provider, COALESCE(timeadded, '') FROM cards
             WHERE collection = ?1 AND uuid = ?2 AND finish = ?3",
            params![collection, uuid, finish],
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
        return Ok(0);
    };

    let moved = copies_available(quantity, available);
    if moved == 0 {
        return Ok(0);
    }

    tx.execute(
        "UPDATE cards SET quantity = quantity - ?4,
           timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE collection = ?1 AND uuid = ?2 AND finish = ?3",
        params![collection, uuid, finish, moved],
    )?;
    // A row with nothing owned and nothing wanted is no longer a fact about the collection
    tx.execute(
        "DELETE FROM cards
         WHERE collection = ?1 AND uuid = ?2 AND finish = ?3
           AND quantity = 0 AND want_quantity = 0",
        params![collection, uuid, finish],
    )?;
    adjust_card(&tx, deck, uuid, finish, board, moved, 0, &time_added, &provider)?;
    tx.commit()?;
    Ok(moved)
}

/// Moves copies of a card off a deck's board and back into a collection.
///
/// The reverse of [`move_from_collection`]; see there for why this is a transfer rather than an
/// adjustment. Only copies the deck holds move: a wanted slot is not a card, so there is nothing
/// to give back.
#[allow(clippy::too_many_arguments)]
pub(super) fn move_to_collection(
    conn: &mut Connection,
    deck: &CollectionID,
    collection: &CollectionID,
    uuid: &CardID,
    finish: &str,
    board: &str,
    quantity: u32,
) -> eyre::Result<i32> {
    let tx = conn.transaction()?;
    let Some((available, provider, time_added)) = tx
        .query_row(
            "SELECT quantity, provider, COALESCE(timeadded, '') FROM deck_cards
             WHERE deck = ?1 AND uuid = ?2 AND finish = ?3 AND board = ?4",
            params![deck, uuid, finish, board],
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
        return Ok(0);
    };

    let moved = copies_available(quantity, available);
    if moved == 0 {
        return Ok(0);
    }

    adjust_card(&tx, deck, uuid, finish, board, -moved, 0, &time_added, &provider)?;
    tx.execute(
        "INSERT INTO cards (uuid, finish, collection, quantity, want_quantity, provider, timeadded, timeupdated)
         VALUES (?2, ?3, ?1, ?4, 0, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         ON CONFLICT (uuid, finish, collection) DO UPDATE SET
           quantity = max(min(cards.quantity + EXCLUDED.quantity, 2147483647), 0),
           timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        params![collection, uuid, finish, moved, provider, time_added],
    )?;
    tx.commit()?;
    Ok(moved)
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
/// Only copies a deck actually holds are committed. A wanted copy is a slot still to fill, so it
/// ties up nothing and is left out of both the total and the holdings.
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

    // Per collection as well as in total, so a deck can say where the copies it is short of are
    // to be found rather than only how many exist
    let mut copies_by_uuid: std::collections::HashMap<CardID, Vec<CardCopies>> =
        std::collections::HashMap::new();
    {
        let query = format!(
            "SELECT uuid, collection, SUM(quantity) FROM cards
             WHERE uuid IN ({placeholders}) AND quantity > 0
             GROUP BY uuid, collection ORDER BY collection"
        );
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(uuids.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                CardCopies {
                    collection: r.get(1)?,
                    quantity: r.get(2)?,
                },
            ))
        })?;
        for row in rows {
            let (uuid, copies) = row?;
            copies_by_uuid.entry(uuid).or_default().push(copies);
        }
    }

    let mut holdings_by_uuid: std::collections::HashMap<CardID, Vec<CardHolding>> =
        std::collections::HashMap::new();
    {
        let query = format!(
            "SELECT dc.uuid, dc.deck, dm.location, dc.board, dc.finish, dc.quantity
             FROM deck_cards dc
             JOIN deck_meta dm ON dm.deck = dc.deck
             WHERE dc.uuid IN ({placeholders}) AND dm.built != 0 AND dc.quantity > 0
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
                copies: copies_by_uuid.remove(uuid).unwrap_or_default(),
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
        want_quantity: row.get(5)?,
        provider: row.get(6)?,
        time_added: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        time_updated: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
    })
}

/// Every `(uuid, provider)` that appears in a collection or a deck.
///
/// Availability has to know which stored cards are *the same card* as the one being asked about,
/// and the only cross-printing handle in the data is the card's name, which lives in `retrieval`
/// rather than here. Rather than resolve names in the persistence layer, this hands back the ids
/// in play so the caller (which has retrieval) can group them.
///
/// Ids only, so this stays a cheap index scan rather than a full row read.
pub(super) fn uuids_in_use(conn: &Connection) -> eyre::Result<Vec<(CardID, String)>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, provider FROM cards
         UNION
         SELECT uuid, provider FROM deck_cards",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
