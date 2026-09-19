use std::collections::HashMap;

use crate::{PurchaseHistoryEntry, PurchaseSummary, UpdateEntryResult};
use models::{CardID, CollectionID};
use rusqlite::{Connection, OptionalExtension, params};

fn insert_purchase_row(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
    finish: &str,
    quantity: i32,
    price_per_unit: Option<f64>,
    provider: &str,
    recorded_at: &str,
) -> eyre::Result<()> {
    conn.execute(
        "INSERT INTO purchase_history \
         (collection_id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![collection_id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at],
    )?;
    Ok(())
}

pub(super) fn record_purchase(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
    finish: &str,
    quantity: i32,
    price_per_unit: Option<f64>,
    provider: &str,
    recorded_at: &str,
) -> eyre::Result<()> {
    insert_purchase_row(conn, collection_id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at)
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<PurchaseHistoryEntry> {
    Ok(PurchaseHistoryEntry {
        id: row.get(0)?,
        card_uuid: row.get(1)?,
        finish: row.get(2)?,
        quantity: row.get(3)?,
        price_per_unit: row.get(4)?,
        provider: row.get(5)?,
        recorded_at: row.get(6)?,
    })
}

const SELECT_FIELDS: &str = "SELECT id, card_uuid, finish, quantity, price_per_unit, provider, recorded_at";

pub(super) fn get_history(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
) -> eyre::Result<Vec<PurchaseHistoryEntry>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_FIELDS} FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 \
         ORDER BY recorded_at DESC",
    ))?;
    let entries = stmt
        .query_map(params![collection_id, card_uuid], row_to_entry)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}

pub(super) fn get_all_history(
    conn: &Connection,
    collection_id: &CollectionID,
) -> eyre::Result<Vec<PurchaseHistoryEntry>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_FIELDS} FROM purchase_history \
         WHERE collection_id = ?1 \
         ORDER BY recorded_at DESC",
    ))?;
    let entries = stmt
        .query_map(params![collection_id], row_to_entry)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}

/// Trims recorded purchase-history quantity for one card+finish down to
/// `target_qty` (e.g. after removing owned copies) — the cheapest entries
/// are trimmed first (then oldest), so the remaining recorded cost basis
/// reflects only the copies of that finish still actually owned.
pub(super) fn trim_history_to_collection(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
    finish: &str,
    target_qty: i32,
) -> eyre::Result<()> {
    trim_by_finish(conn, collection_id, None, card_uuid, finish, target_qty)?;
    conn.execute(
        "DELETE FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND finish = ?3 AND quantity <= 0",
        params![collection_id, card_uuid, finish],
    )?;
    Ok(())
}

pub(super) fn transfer_trimmed_history_to_collection(
    conn: &Connection,
    src_collection: &CollectionID,
    dst_collection: &CollectionID,
    card_uuid: &CardID,
    finish: &str,
    target_qty: i32,
) -> eyre::Result<()> {
    trim_by_finish(conn, src_collection, Some(dst_collection), card_uuid, finish, target_qty)?;
    conn.execute(
        "DELETE FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND finish = ?3 AND quantity <= 0",
        params![src_collection, card_uuid, finish],
    )?;
    Ok(())
}

struct TrimEntry {
    id: i64,
    qty: i32,
    price: Option<f64>,
    provider: String,
    recorded_at: String,
}

fn trim_by_finish(
    conn: &Connection,
    collection_id: &CollectionID,
    transfer_to: Option<&CollectionID>,
    card_uuid: &CardID,
    finish: &str,
    target: i32,
) -> eyre::Result<()> {
    let total: i32 = conn.query_row(
        "SELECT COALESCE(SUM(quantity), 0) FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND finish = ?3",
        params![collection_id, card_uuid, finish],
        |row| row.get(0),
    )?;

    if total <= target {
        return Ok(());
    }

    let mut excess = total - target;

    let mut stmt = conn.prepare(
        "SELECT id, quantity, price_per_unit, provider, recorded_at \
         FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND finish = ?3 AND quantity > 0 \
         ORDER BY price_per_unit ASC NULLS FIRST, id ASC",
    )?;
    let entries: Vec<TrimEntry> = stmt
        .query_map(params![collection_id, card_uuid, finish], |row| {
            Ok(TrimEntry {
                id: row.get(0)?,
                qty: row.get(1)?,
                price: row.get(2)?,
                provider: row.get(3)?,
                recorded_at: row.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    for entry in entries {
        if excess <= 0 {
            break;
        }
        let remove = entry.qty.min(excess);
        conn.execute(
            "UPDATE purchase_history SET quantity = quantity - ?1 WHERE id = ?2",
            params![remove, entry.id],
        )?;
        if let Some(dst) = transfer_to {
            insert_purchase_row(conn, dst, card_uuid, finish, remove, entry.price, &entry.provider, &entry.recorded_at)?;
        }
        excess -= remove;
    }

    Ok(())
}

pub(super) fn delete_entry(
    conn: &Connection,
    collection_id: &CollectionID,
    entry_id: i64,
) -> eyre::Result<bool> {
    let rows = conn.execute(
        "DELETE FROM purchase_history WHERE id = ?1 AND collection_id = ?2",
        params![entry_id, collection_id],
    )?;
    Ok(rows > 0)
}

/// The entry's `finish` is fixed at creation and can't be changed here —
/// only how many copies (of that same finish) and at what price.
pub(super) fn update_entry(
    conn: &Connection,
    collection_id: &CollectionID,
    entry_id: i64,
    quantity: i32,
    price_per_unit: Option<f64>,
) -> eyre::Result<UpdateEntryResult> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT card_uuid, finish FROM purchase_history WHERE id = ?1 AND collection_id = ?2",
            params![entry_id, collection_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((card_uuid, finish)) = row else {
        return Ok(UpdateEntryResult::NotFound);
    };

    let col_qty: i32 = conn
        .query_row(
            "SELECT COALESCE(quantity, 0) FROM cards WHERE collection = ?1 AND uuid = ?2 AND finish = ?3",
            params![collection_id, &card_uuid, &finish],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let other_qty: i32 = conn.query_row(
        "SELECT COALESCE(SUM(quantity), 0) FROM purchase_history \
         WHERE card_uuid = ?1 AND finish = ?2 AND collection_id = ?3 AND id != ?4",
        params![&card_uuid, &finish, collection_id, entry_id],
        |row| row.get(0),
    )?;

    let new_total = other_qty + quantity;
    if new_total > col_qty {
        return Ok(UpdateEntryResult::ValidationError(format!(
            "Cannot record {new_total} copies — collection only has {col_qty}"
        )));
    }

    conn.execute(
        "UPDATE purchase_history SET quantity = ?1, price_per_unit = ?2 WHERE id = ?3 AND collection_id = ?4",
        params![quantity, price_per_unit, entry_id, collection_id],
    )?;
    Ok(UpdateEntryResult::Updated)
}

pub(super) fn get_collection_totals(
    conn: &Connection,
    collection_id: &CollectionID,
) -> eyre::Result<HashMap<(CardID, String), PurchaseSummary>> {
    let mut stmt = conn.prepare(
        "SELECT card_uuid, finish, \
                SUM(COALESCE(price_per_unit, 0.0) * quantity), \
                SUM(CASE WHEN price_per_unit IS NOT NULL THEN quantity ELSE 0 END) \
         FROM purchase_history \
         WHERE collection_id = ?1 \
         GROUP BY card_uuid, finish \
         HAVING SUM(CASE WHEN price_per_unit IS NOT NULL THEN quantity ELSE 0 END) > 0",
    )?;
    let map = stmt
        .query_map(params![collection_id], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                PurchaseSummary {
                    total_paid: row.get::<_, f64>(2)?,
                    quantity: row.get::<_, i32>(3)?,
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(map)
}
