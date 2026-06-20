use std::collections::HashMap;

use ::models::{CardPrices, RetailerPrices};
use rusqlite::Connection;
use tracing::info;

pub(super) fn load_prices_file(path: &str) -> eyre::Result<HashMap<String, CardPrices>> {
    info!(path, "Loading MTG prices");
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT uuid, provider, finish, price FROM prices WHERE source = 'paper' AND priceType = 'retail'",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    let mut paper_map: HashMap<String, HashMap<String, RetailerPrices>> = HashMap::new();
    for row in rows.flatten() {
        let (uuid, retailer, finish, price) = row;
        let rp = paper_map
            .entry(uuid)
            .or_default()
            .entry(retailer)
            .or_insert(RetailerPrices {
                normal: None,
                foil: None,
            });
        match finish.as_str() {
            "normal" => rp.normal = Some(price),
            "foil" => rp.foil = Some(price),
            _ => {}
        }
    }

    let map = paper_map
        .into_iter()
        .map(|(uuid, paper)| (uuid.clone(), CardPrices { uuid, paper }))
        .collect();
    info!("MTG prices loaded");
    Ok(map)
}
