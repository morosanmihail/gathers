use std::collections::HashMap;

use crate::{PurchaseHistoryEntry, PurchaseSummary, UpdateEntryResult};
use models::{CardID, CollectionID};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn record_purchase(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
    quantity: i32,
    foil_quantity: i32,
    normal_price_per_unit: Option<f64>,
    foil_price_per_unit: Option<f64>,
    provider: &str,
    recorded_at: &str,
) -> eyre::Result<()> {
    conn.execute(
        "INSERT INTO purchase_history \
         (collection_id, card_uuid, quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit, provider, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            collection_id,
            card_uuid,
            quantity,
            foil_quantity,
            normal_price_per_unit,
            foil_price_per_unit,
            provider,
            recorded_at
        ],
    )?;
    Ok(())
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<PurchaseHistoryEntry> {
    Ok(PurchaseHistoryEntry {
        id: row.get(0)?,
        card_uuid: row.get(1)?,
        quantity: row.get(2)?,
        foil_quantity: row.get(3)?,
        normal_price_per_unit: row.get(4)?,
        foil_price_per_unit: row.get(5)?,
        provider: row.get(6)?,
        recorded_at: row.get(7)?,
    })
}

const SELECT_FIELDS: &str =
    "SELECT id, card_uuid, quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit, provider, recorded_at";

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

pub(super) fn trim_history_to_collection(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
    target_qty: i32,
    target_foil_qty: i32,
) -> eyre::Result<()> {
    trim_by_type(conn, collection_id, None, card_uuid, target_qty, false)?;
    trim_by_type(conn, collection_id, None, card_uuid, target_foil_qty, true)?;
    conn.execute(
        "DELETE FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND quantity <= 0 AND foil_quantity <= 0",
        params![collection_id, card_uuid],
    )?;
    Ok(())
}

pub(super) fn transfer_trimmed_history_to_collection(
    conn: &Connection,
    src_collection: &CollectionID,
    dst_collection: &CollectionID,
    card_uuid: &CardID,
    target_qty: i32,
    target_foil_qty: i32,
) -> eyre::Result<()> {
    trim_by_type(conn, src_collection, Some(dst_collection), card_uuid, target_qty, false)?;
    trim_by_type(conn, src_collection, Some(dst_collection), card_uuid, target_foil_qty, true)?;
    conn.execute(
        "DELETE FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND quantity <= 0 AND foil_quantity <= 0",
        params![src_collection, card_uuid],
    )?;
    Ok(())
}

struct TrimEntry {
    id: i64,
    qty: i32,
    normal_price: Option<f64>,
    foil_price: Option<f64>,
    provider: String,
    recorded_at: String,
}

fn trim_by_type(
    conn: &Connection,
    collection_id: &CollectionID,
    transfer_to: Option<&CollectionID>,
    card_uuid: &CardID,
    target: i32,
    foil: bool,
) -> eyre::Result<()> {
    let (qty_col, price_col) = if foil {
        ("foil_quantity", "foil_price_per_unit")
    } else {
        ("quantity", "normal_price_per_unit")
    };

    let total: i32 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM({qty_col}), 0) FROM purchase_history \
             WHERE collection_id = ?1 AND card_uuid = ?2"
        ),
        params![collection_id, card_uuid],
        |row| row.get(0),
    )?;

    if total <= target {
        return Ok(());
    }

    let mut excess = total - target;

    let mut stmt = conn.prepare(&format!(
        "SELECT id, {qty_col}, normal_price_per_unit, foil_price_per_unit, provider, recorded_at \
         FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 AND {qty_col} > 0 \
         ORDER BY {price_col} ASC NULLS FIRST, id ASC"
    ))?;
    let entries: Vec<TrimEntry> = stmt
        .query_map(params![collection_id, card_uuid], |row| {
            Ok(TrimEntry {
                id: row.get(0)?,
                qty: row.get(1)?,
                normal_price: row.get(2)?,
                foil_price: row.get(3)?,
                provider: row.get(4)?,
                recorded_at: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    for entry in entries {
        if excess <= 0 {
            break;
        }
        let remove = entry.qty.min(excess);
        conn.execute(
            &format!("UPDATE purchase_history SET {qty_col} = {qty_col} - ?1 WHERE id = ?2"),
            params![remove, entry.id],
        )?;
        if let Some(dst) = transfer_to {
            let (new_qty, new_foil_qty, new_normal_price, new_foil_price) = if foil {
                (0i32, remove, None::<f64>, entry.foil_price)
            } else {
                (remove, 0i32, entry.normal_price, None::<f64>)
            };
            conn.execute(
                "INSERT INTO purchase_history \
                 (collection_id, card_uuid, quantity, foil_quantity, \
                  normal_price_per_unit, foil_price_per_unit, provider, recorded_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    dst,
                    card_uuid,
                    new_qty,
                    new_foil_qty,
                    new_normal_price,
                    new_foil_price,
                    entry.provider,
                    entry.recorded_at,
                ],
            )?;
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

pub(super) fn update_entry(
    conn: &Connection,
    collection_id: &CollectionID,
    entry_id: i64,
    quantity: i32,
    foil_quantity: i32,
    normal_price_per_unit: Option<f64>,
    foil_price_per_unit: Option<f64>,
) -> eyre::Result<UpdateEntryResult> {
    let card_uuid: Option<String> = conn
        .query_row(
            "SELECT card_uuid FROM purchase_history WHERE id = ?1 AND collection_id = ?2",
            params![entry_id, collection_id],
            |row| row.get(0),
        )
        .optional()?;

    let Some(card_uuid) = card_uuid else {
        return Ok(UpdateEntryResult::NotFound);
    };

    let (col_qty, col_foil_qty): (i32, i32) = conn
        .query_row(
            "SELECT COALESCE(quantity, 0), COALESCE(foilquantity, 0) \
             FROM cards WHERE collection = ?1 AND uuid = ?2",
            params![collection_id, &card_uuid],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0));

    let (other_qty, other_foil_qty): (i32, i32) = conn.query_row(
        "SELECT COALESCE(SUM(quantity), 0), COALESCE(SUM(foil_quantity), 0) \
         FROM purchase_history WHERE card_uuid = ?1 AND collection_id = ?2 AND id != ?3",
        params![&card_uuid, collection_id, entry_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let new_total = other_qty + quantity;
    let new_foil_total = other_foil_qty + foil_quantity;

    if new_total > col_qty {
        return Ok(UpdateEntryResult::ValidationError(format!(
            "Cannot record {new_total} copies — collection only has {col_qty}"
        )));
    }
    if new_foil_total > col_foil_qty {
        return Ok(UpdateEntryResult::ValidationError(format!(
            "Cannot record {new_foil_total} foil copies — collection only has {col_foil_qty}"
        )));
    }

    conn.execute(
        "UPDATE purchase_history \
         SET quantity = ?1, foil_quantity = ?2, normal_price_per_unit = ?3, foil_price_per_unit = ?4 \
         WHERE id = ?5 AND collection_id = ?6",
        params![quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit, entry_id, collection_id],
    )?;
    Ok(UpdateEntryResult::Updated)
}

pub(super) fn get_collection_totals(
    conn: &Connection,
    collection_id: &CollectionID,
) -> eyre::Result<HashMap<CardID, PurchaseSummary>> {
    let mut stmt = conn.prepare(
        "SELECT card_uuid, \
                SUM(COALESCE(normal_price_per_unit, 0.0) * quantity), \
                SUM(COALESCE(foil_price_per_unit, 0.0) * foil_quantity), \
                SUM(CASE WHEN normal_price_per_unit IS NOT NULL THEN quantity ELSE 0 END), \
                SUM(CASE WHEN foil_price_per_unit IS NOT NULL THEN foil_quantity ELSE 0 END) \
         FROM purchase_history \
         WHERE collection_id = ?1 \
         GROUP BY card_uuid \
         HAVING SUM(CASE WHEN normal_price_per_unit IS NOT NULL THEN quantity ELSE 0 END) > 0 \
             OR SUM(CASE WHEN foil_price_per_unit IS NOT NULL THEN foil_quantity ELSE 0 END) > 0",
    )?;
    let map = stmt
        .query_map(params![collection_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PurchaseSummary {
                    total_normal_paid: row.get::<_, f64>(1)?,
                    total_foil_paid: row.get::<_, f64>(2)?,
                    quantity: row.get::<_, i32>(3)?,
                    foil_quantity: row.get::<_, i32>(4)?,
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(map)
}
