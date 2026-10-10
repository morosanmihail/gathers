use crate::{PriceHistoryEntry, PricePoint};
use rusqlite::Connection;

/// Upserts one row per point. Non-finite and non-positive prices are
/// skipped. Returns how many rows were written.
pub(super) fn record(
    conn: &mut Connection,
    provider: &str,
    prices: &[PricePoint],
) -> eyre::Result<usize> {
    let tx = conn.transaction()?;
    let mut written = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO price_history (provider, card_uuid, retailer, finish, price, currency, recorded_on)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (provider, card_uuid, retailer, finish, recorded_on) DO UPDATE SET
              price = EXCLUDED.price,
              currency = EXCLUDED.currency",
        )?;
        for point in prices.iter().filter(|p| p.price.is_finite() && p.price > 0.0) {
            stmt.execute(rusqlite::params![
                provider,
                point.card_uuid,
                point.retailer,
                point.finish,
                point.price,
                point.currency,
                point.recorded_on,
            ])?;
            written += 1;
        }
    }
    tx.commit()?;
    Ok(written)
}

pub(super) fn has_any(conn: &Connection, provider: &str) -> eyre::Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM price_history WHERE provider = ?1)",
        rusqlite::params![provider],
        |row| row.get(0),
    )?)
}

/// Oldest first.
pub(super) fn get(
    conn: &Connection,
    provider: &str,
    card_uuid: &str,
) -> eyre::Result<Vec<PriceHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT retailer, finish, price, currency, recorded_on FROM price_history
         WHERE provider = ?1 AND card_uuid = ?2
         ORDER BY recorded_on, retailer, finish",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![provider, card_uuid], |row| {
            Ok(PriceHistoryEntry {
                retailer: row.get(0)?,
                finish: row.get(1)?,
                price: row.get(2)?,
                currency: row.get(3)?,
                recorded_on: row.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(entries)
}
