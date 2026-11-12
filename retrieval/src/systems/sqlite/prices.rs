use std::collections::{HashMap, HashSet};

use ::models::{CardPrices, RetailerPrices};
use rusqlite::{Connection, params_from_iter};
use tracing::{info, warn};

use crate::systems::sql_helpers::sql_placeholders;

/// Most uuids bound into one `IN (...)` lookup; stays well under SQLite's variable limit.
const LOOKUP_CHUNK: usize = 500;

/// Opens the MTG prices database for on-demand lookups.
///
/// Prices used to be loaded whole into a `HashMap` (~150MB for AllPricesToday), rebuilt on
/// every daily price update; the allocator never handed the freed maps back, so the server's
/// memory crept up day after day. Querying by uuid instead keeps memory flat. MTGJSON ships
/// `idx_prices_uuid`; it's created here when missing so lookups never scan the table.
pub(super) fn open_prices(path: &str) -> eyre::Result<Connection> {
    info!(path, "Opening MTG prices");
    let conn = Connection::open(path)?;
    if let Err(e) = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_prices_uuid ON prices (uuid)") {
        warn!(path, error = %e, "Couldn't index MTG prices by uuid; lookups will be slow");
    }
    Ok(conn)
}

/// Current paper retail prices for each of `uuids` that has any, keyed by uuid.
pub(super) fn query_prices(conn: &Connection, uuids: &[String]) -> eyre::Result<HashMap<String, CardPrices>> {
    let unique: Vec<&String> = uuids.iter().collect::<HashSet<_>>().into_iter().collect();
    let mut paper_map: HashMap<String, HashMap<String, RetailerPrices>> = HashMap::new();

    for chunk in unique.chunks(LOOKUP_CHUNK) {
        let sql = format!(
            "SELECT uuid, provider, finish, price, currency, date FROM prices \
             WHERE uuid IN ({}) AND source = 'paper' AND priceType = 'retail'",
            sql_placeholders(chunk.len())
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params_from_iter(chunk.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;

        for row in rows.flatten() {
            let (uuid, retailer, finish, price, currency, date) = row;
            let rp = paper_map
                .entry(uuid)
                .or_default()
                .entry(retailer)
                .or_insert(RetailerPrices {
                    normal: None,
                    foil: None,
                    currency: currency
                        .filter(|c| !c.is_empty())
                        .unwrap_or_else(|| ::models::DEFAULT_CURRENCY.to_string()),
                    date: None,
                });
            if let Some(date) = date.as_deref().and_then(::models::parse_price_date)
                && rp.date.is_none_or(|current| current < date)
            {
                rp.date = Some(date);
            }
            match finish.as_str() {
                "normal" => rp.normal = Some(price),
                "foil" => rp.foil = Some(price),
                _ => {}
            }
        }
    }

    Ok(paper_map
        .into_iter()
        .map(|(uuid, paper)| (uuid.clone(), CardPrices { uuid, paper }))
        .collect())
}
