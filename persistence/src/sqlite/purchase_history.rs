use std::collections::HashMap;

use crate::{PurchaseHistoryEntry, PurchaseSummary};
use models::{CardID, CollectionID};
use rusqlite::{Connection, params};

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
