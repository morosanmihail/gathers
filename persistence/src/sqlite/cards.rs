use crate::{CollectionCard, CollectionCardsParams, CollectionSortField};
use models::CollectionID;
use models::filters::SortOrder;
use rusqlite::Connection;

pub(super) fn add_cards(
    conn: &Connection,
    collection_id: &CollectionID,
    cards: &[CollectionCard],
) -> eyre::Result<Vec<CollectionCard>> {
    if cards.is_empty() {
        return Ok(vec![]);
    }

    // Rows with no existing (uuid, finish) in this collection get their
    // initial quantity/want_quantity floored at 0 in Rust before the INSERT —
    // otherwise removing (or un-wanting) a (uuid, finish) that was never
    // added would insert a negative-quantity "ghost" row instead of being a
    // no-op: the ON CONFLICT clamp below only clamps `existing + delta`, and
    // there's no existing row here for it to clamp against. This can't be
    // done in the VALUES clause itself (e.g. `MAX(?, 0)`) because that
    // expression also becomes `EXCLUDED.quantity`, which the ON CONFLICT
    // branch needs un-clamped to correctly compute `existing + delta`.
    let existing: std::collections::HashSet<(String, String)> = {
        let mut stmt = conn.prepare("SELECT uuid, finish FROM cards WHERE collection = ?1")?;
        stmt.query_map(rusqlite::params![collection_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .flatten()
        .collect()
    };

    let placeholders = cards
        .iter()
        .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?)")
        .collect::<Vec<_>>()
        .join(",");
    let mut query_params: Vec<String> = vec![];
    for c in cards {
        let is_new = !existing.contains(&(c.uuid.clone(), c.finish.clone()));
        let quantity = if is_new { c.quantity.max(0) } else { c.quantity };
        let want_quantity = if is_new { c.want_quantity.max(0) } else { c.want_quantity };
        query_params.push(c.uuid.clone());
        query_params.push(c.finish.clone());
        query_params.push(collection_id.clone());
        query_params.push(quantity.to_string());
        query_params.push(want_quantity.to_string());
        query_params.push(c.time_added.clone());
        query_params.push(c.time_added.clone()); // timeupdated = timeadded on creation
        query_params.push(c.provider.clone());
    }
    let query = format!(
        "INSERT INTO cards (uuid, finish, collection, quantity, want_quantity, timeadded, timeupdated, provider)
VALUES {}
ON CONFLICT (uuid, finish, collection) DO UPDATE SET
 quantity = max(cards.quantity + EXCLUDED.quantity, 0),
 want_quantity = max(cards.want_quantity + EXCLUDED.want_quantity, 0),
 timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
RETURNING uuid, finish, collection, quantity, want_quantity, timeadded, provider",
        placeholders
    );
    let mut stmt = conn.prepare(&query)?;
    let result: Vec<CollectionCard> = stmt
        .query_map(rusqlite::params_from_iter(query_params.iter()), |row| {
            Ok(CollectionCard {
                uuid: row.get(0)?,
                finish: row.get(1)?,
                collection: row.get(2)?,
                quantity: row.get(3)?,
                want_quantity: row.get(4)?,
                time_added: row.get(5)?,
                provider: row.get(6)?,
            })
        })?
        .flatten()
        .collect();

    conn.execute("DELETE FROM cards WHERE quantity = 0 AND want_quantity = 0", [])?;

    Ok(result)
}

pub(super) fn get_paginated(
    conn: &Connection,
    collection_id: &CollectionID,
    params: CollectionCardsParams,
) -> eyre::Result<Vec<CollectionCard>> {
    let mut conditions = vec!["collection = ?1".to_string()];
    let mut query_params: Vec<String> = vec![collection_id.clone()];
    let mut i = 2usize;

    if let Some(provider) = &params.provider {
        conditions.push(format!("provider = ?{i}"));
        query_params.push(provider.clone());
        i += 1;
    } else if !params.providers.is_empty() {
        let placeholders: Vec<String> = params
            .providers
            .iter()
            .enumerate()
            .map(|(j, _)| format!("?{}", i + j))
            .collect();
        conditions.push(format!("provider IN ({})", placeholders.join(", ")));
        query_params.extend(params.providers.clone());
        i += params.providers.len();
    }

    // Sorts (and paginates) over individual (uuid, finish) rows, not
    // distinct cards — a card with several finishes contributes one row
    // per finish. Grouping those back into "one card, several finishes"
    // is a display-layer concern (see `models::CollectionCard::finish`).
    let sort_col = match &params.sort_by {
        Some(CollectionSortField::Quantity) => "quantity",
        Some(CollectionSortField::WantQuantity) => "want_quantity",
        Some(CollectionSortField::Provider) => "provider",
        _ => "timeadded",
    };
    let sort_dir = if matches!(&params.sort_order, Some(SortOrder::Desc)) {
        "DESC"
    } else {
        "ASC"
    };

    let query = format!(
        "SELECT uuid, finish, quantity, want_quantity, timeadded, provider \
         FROM cards WHERE {} ORDER BY {} {} LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "),
        sort_col,
        sort_dir,
        i,
        i + 1,
    );
    query_params.push(params.limit.to_string());
    query_params.push(params.offset.to_string());

    let collection_id = collection_id.clone();
    let mut stmt = conn.prepare(&query)?;
    let cards: Vec<CollectionCard> = stmt
        .query_map(rusqlite::params_from_iter(query_params.iter()), |row| {
            Ok(CollectionCard {
                uuid: row.get(0)?,
                finish: row.get(1)?,
                quantity: row.get(2)?,
                want_quantity: row.get(3)?,
                time_added: row.get(4)?,
                collection: collection_id.clone(),
                provider: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(cards)
}
