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
    price_per_unit: Option<f64>,
    provider: &str,
    recorded_at: &str,
) -> eyre::Result<()> {
    conn.execute(
        "INSERT INTO purchase_history \
         (collection_id, card_uuid, quantity, foil_quantity, price_per_unit, provider, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            collection_id,
            card_uuid,
            quantity,
            foil_quantity,
            price_per_unit,
            provider,
            recorded_at
        ],
    )?;
    Ok(())
}

pub(super) fn get_history(
    conn: &Connection,
    collection_id: &CollectionID,
    card_uuid: &CardID,
) -> eyre::Result<Vec<PurchaseHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, quantity, foil_quantity, price_per_unit, recorded_at \
         FROM purchase_history \
         WHERE collection_id = ?1 AND card_uuid = ?2 \
         ORDER BY recorded_at DESC",
    )?;
    let entries = stmt
        .query_map(params![collection_id, card_uuid], |row| {
            Ok(PurchaseHistoryEntry {
                id: row.get(0)?,
                quantity: row.get(1)?,
                foil_quantity: row.get(2)?,
                price_per_unit: row.get(3)?,
                recorded_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}

pub(super) fn get_collection_totals(
    conn: &Connection,
    collection_id: &CollectionID,
) -> eyre::Result<HashMap<CardID, PurchaseSummary>> {
    let mut stmt = conn.prepare(
        "SELECT card_uuid, \
                SUM(price_per_unit * (quantity + foil_quantity)), \
                SUM(quantity), \
                SUM(foil_quantity) \
         FROM purchase_history \
         WHERE collection_id = ?1 AND price_per_unit IS NOT NULL \
         GROUP BY card_uuid",
    )?;
    let map = stmt
        .query_map(params![collection_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PurchaseSummary {
                    total_paid: row.get::<_, f64>(1)?,
                    quantity: row.get::<_, i32>(2)?,
                    foil_quantity: row.get::<_, i32>(3)?,
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(map)
}
