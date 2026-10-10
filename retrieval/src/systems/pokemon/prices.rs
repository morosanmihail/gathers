//! Pokémon prices: a snapshot of TCGplayer's current prices, as republished
//! daily by [TCGCSV](https://tcgcsv.com). The snapshot is one self-contained
//! sqlite file keyed by TCGplayer product id (the cards db's `idTCGP`), so it
//! can be built, mirrored and replaced independently of the cards db.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use ::models::{CardPrices, RetailerPrices, parse_price_date};
use chrono::{DateTime, NaiveDate};
use rusqlite::{Connection, params};
use serde::Deserialize;
use tracing::{info, warn};

use crate::systems::sql_helpers::sql_placeholders;

const TCGCSV_BASE: &str = "https://tcgcsv.com";
/// TCGplayer's category id for (English) Pokémon.
const POKEMON_CATEGORY: u32 = 3;
/// TCGCSV asks scrapers to pace themselves; this is its sample code's delay.
const REQUEST_DELAY: Duration = Duration::from_millis(250);
const MAX_ATTEMPTS: u32 = 3;
/// Default file name of the prices db, and its stem on mirrors. Distinct
/// from pokedata's `pokemon_prices.sqlite`, so a mirror or install still
/// holding that older format is never mistaken for this one.
pub const POKEMON_PRICES_FILE: &str = "pokemon_prices_tcgcsv.sqlite";
/// Key prices are listed under in `CardPrices::paper`.
const RETAILER: &str = "tcgplayer";

/// TCGplayer printings ("subtypes") in the order they're preferred as a
/// card's normal price. A card printed only in holofoil has `["Holofoil"]` as
/// its finishes, and every finish but "foil" is valued at the normal price
/// (see `server::collections`), so holofoil has to be a candidate here too.
const NORMAL_SUBTYPES: &[&str] = &[
    "Normal",
    "Unlimited",
    "1st Edition",
    "Holofoil",
    "Unlimited Holofoil",
    "1st Edition Holofoil",
    "Reverse Holofoil",
];
/// Printings preferred as a card's foil price, skipping whichever one was
/// already taken as the normal price.
const FOIL_SUBTYPES: &[&str] = &[
    "Reverse Holofoil",
    "Holofoil",
    "Unlimited Holofoil",
    "1st Edition Holofoil",
];

const SCHEMA: &str = "
CREATE TABLE prices (
    productId      INTEGER NOT NULL,
    subTypeName    TEXT NOT NULL,
    lowPrice       REAL,
    midPrice       REAL,
    highPrice      REAL,
    marketPrice    REAL,
    directLowPrice REAL,
    PRIMARY KEY (productId, subTypeName)
) WITHOUT ROWID;
-- 'updated': when TCGCSV last refreshed its data, RFC 3339.
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

#[derive(Debug, Deserialize)]
struct TcgcsvResponse<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<String>,
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    group_id: i64,
}

/// One printing's prices for a TCGplayer product, as TCGCSV lists them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PriceRow {
    pub product_id: i64,
    pub sub_type_name: String,
    pub low_price: Option<f64>,
    pub mid_price: Option<f64>,
    pub high_price: Option<f64>,
    pub market_price: Option<f64>,
    pub direct_low_price: Option<f64>,
}

/// Opens the prices db at `path`, or `None` when there's no file or it isn't
/// a TCGCSV snapshot (a leftover pokedata db, until the next price update
/// replaces it).
pub(super) fn open_prices_db(path: &str) -> eyre::Result<Option<Connection>> {
    if !PathBuf::from(path).exists() {
        return Ok(None);
    }
    let conn = Connection::open(path)?;
    let current = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('prices') WHERE name = 'productId')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false);
    if !current {
        warn!(path, "Pokemon prices db is in an old format; ignored until prices are updated");
        return Ok(None);
    }
    Ok(Some(conn))
}

/// Prices of the given TCGplayer products, as listed in the snapshot. Each
/// product's `CardPrices::uuid` is left empty for the caller to fill in.
pub(super) fn prices_for_products(
    conn: &Connection,
    product_ids: &[i64],
) -> eyre::Result<HashMap<i64, CardPrices>> {
    if product_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let date: Option<NaiveDate> = conn
        .query_row("SELECT value FROM meta WHERE key = 'updated'", [], |row| row.get::<_, String>(0))
        .ok()
        .as_deref()
        .and_then(parse_price_date);

    // An unsold printing has no market price; its median listing is the
    // next best thing.
    let mut stmt = conn.prepare(&format!(
        "SELECT productId, subTypeName, COALESCE(marketPrice, midPrice) AS price FROM prices \
         WHERE productId IN ({}) AND price > 0",
        sql_placeholders(product_ids.len())
    ))?;
    let mut by_product: HashMap<i64, HashMap<String, f64>> = HashMap::new();
    let rows = stmt.query_map(rusqlite::params_from_iter(product_ids), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
    })?;
    for row in rows {
        let (product_id, sub_type, price) = row?;
        by_product.entry(product_id).or_default().insert(sub_type, price);
    }

    Ok(by_product
        .into_iter()
        .filter_map(|(product_id, subtypes)| {
            let retail = retailer_prices(&subtypes, date)?;
            Some((
                product_id,
                CardPrices {
                    uuid: String::new(),
                    paper: HashMap::from([(RETAILER.to_string(), retail)]),
                },
            ))
        })
        .collect())
}

/// Folds a product's printings into a normal and a foil price (see
/// `NORMAL_SUBTYPES`/`FOIL_SUBTYPES`). A product listed only under printings
/// neither list knows is priced at the cheapest of them.
fn retailer_prices(subtypes: &HashMap<String, f64>, date: Option<NaiveDate>) -> Option<RetailerPrices> {
    let (normal_name, normal) = NORMAL_SUBTYPES
        .iter()
        .find_map(|name| subtypes.get(*name).map(|p| (Some(*name), *p)))
        .or_else(|| subtypes.values().copied().min_by(f64::total_cmp).map(|p| (None, p)))?;
    let foil = FOIL_SUBTYPES
        .iter()
        .filter(|name| Some(**name) != normal_name)
        .find_map(|name| subtypes.get(*name).copied());
    Some(RetailerPrices {
        normal: Some(normal),
        foil,
        currency: "USD".to_string(),
        date,
    })
}

/// Writes a snapshot db at `path` (replacing any file there) from TCGCSV's
/// `updated` timestamp and price rows. Built beside `path` and renamed into
/// place, so readers never see a half-written db.
pub(super) fn write_prices_db(
    path: &Path,
    updated: &str,
    rows: impl IntoIterator<Item = PriceRow>,
) -> eyre::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut conn = Connection::open(staging.path())?;
        conn.execute_batch(SCHEMA)?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO meta (key, value) VALUES ('updated', ?1)", [updated])?;
        {
            let mut insert = tx.prepare(
                "INSERT OR REPLACE INTO prices
                 (productId, subTypeName, lowPrice, midPrice, highPrice, marketPrice, directLowPrice)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for r in rows {
                insert.execute(params![
                    r.product_id,
                    r.sub_type_name,
                    r.low_price,
                    r.mid_price,
                    r.high_price,
                    r.market_price,
                    r.direct_low_price
                ])?;
            }
        }
        tx.commit()?;
    }
    staging.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// TCGCSV's "last updated" stamp (`2026-10-09T20:05:19+0000`) as RFC 3339.
fn normalize_updated(raw: &str) -> eyre::Result<String> {
    let raw = raw.trim();
    let parsed = DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%z")
        .or_else(|_| DateTime::parse_from_rfc3339(raw))
        .map_err(|e| eyre::eyre!("unreadable TCGCSV last-updated stamp {raw:?}: {e}"))?;
    Ok(parsed.to_rfc3339())
}

async fn get_with_retries(client: &reqwest::Client, url: &str) -> eyre::Result<reqwest::Response> {
    let mut last_err = eyre::eyre!("no attempts made");
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(response) => return Ok(response),
            Err(e) => {
                warn!(url, attempt, error = %e, "TCGCSV request failed");
                last_err = e.into();
                tokio::time::sleep(REQUEST_DELAY * 4 * attempt).await;
            }
        }
    }
    Err(last_err)
}

async fn get_results<T: serde::de::DeserializeOwned>(client: &reqwest::Client, url: &str) -> eyre::Result<Vec<T>> {
    let response: TcgcsvResponse<T> = get_with_retries(client, url).await?.json().await?;
    if !response.success {
        eyre::bail!("TCGCSV reported failure for {url}: {:?}", response.errors);
    }
    Ok(response.results)
}

/// Builds the prices db at `path` straight from TCGCSV, never from a mirror
/// — the source the mirror itself publishes. Fails without touching `path`
/// if any group's prices can't be fetched, rather than publishing a snapshot
/// missing whole sets.
pub(crate) async fn build_pokemon_prices(path: &Path) -> eyre::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "GatheRs/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/morosanmihail/gathers)"
        ))
        .timeout(Duration::from_secs(60))
        .build()?;

    let updated = normalize_updated(
        &get_with_retries(&client, &format!("{TCGCSV_BASE}/last-updated.txt"))
            .await?
            .text()
            .await?,
    )?;
    let groups: Vec<Group> =
        get_results(&client, &format!("{TCGCSV_BASE}/tcgplayer/{POKEMON_CATEGORY}/groups")).await?;
    if groups.is_empty() {
        eyre::bail!("TCGCSV listed no Pokemon groups");
    }
    info!(groups = groups.len(), updated, "Fetching Pokemon prices from TCGCSV");

    let mut rows = Vec::new();
    for group in &groups {
        tokio::time::sleep(REQUEST_DELAY).await;
        let url = format!(
            "{TCGCSV_BASE}/tcgplayer/{POKEMON_CATEGORY}/{}/prices",
            group.group_id
        );
        rows.extend(get_results::<PriceRow>(&client, &url).await?);
    }

    info!(rows = rows.len(), dest = ?path, "Writing Pokemon prices");
    write_prices_db(path, &updated, rows)?;
    Ok(())
}

/// Updates the prices db at `path`: from the first configured mirror that
/// has it, else straight from TCGCSV.
pub async fn download_pokemon_prices(path: &str) -> eyre::Result<()> {
    let target = PathBuf::from(path);
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    if crate::mirror::try_mirrors(POKEMON_PRICES_FILE, &target, None).await {
        return Ok(());
    }
    build_pokemon_prices(&target).await?;
    info!(dest = ?target, "Pokemon prices saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(product_id: i64, sub_type: &str, market: Option<f64>, mid: Option<f64>) -> PriceRow {
        PriceRow {
            product_id,
            sub_type_name: sub_type.to_string(),
            low_price: None,
            mid_price: mid,
            high_price: None,
            market_price: market,
            direct_low_price: None,
        }
    }

    fn prices(subtypes: &[(&str, f64)]) -> Option<(Option<f64>, Option<f64>)> {
        let map = subtypes.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        retailer_prices(&map, None).map(|r| (r.normal, r.foil))
    }

    #[test]
    fn parses_tcgcsv_prices_response() {
        let body = r#"{"success": true, "errors": [], "results": [
            {"productId": 83439, "lowPrice": 0.21, "midPrice": 0.5, "highPrice": 14.06, "marketPrice": 0.73, "directLowPrice": 0.44, "subTypeName": "Normal"},
            {"productId": 83439, "lowPrice": 0.5, "midPrice": 2.44, "highPrice": 34.82, "marketPrice": null, "directLowPrice": null, "subTypeName": "Reverse Holofoil"}
        ]}"#;
        let parsed: TcgcsvResponse<PriceRow> = serde_json::from_str(body).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.results.len(), 2);
        assert_eq!(parsed.results[0].market_price, Some(0.73));
        assert_eq!(parsed.results[1].sub_type_name, "Reverse Holofoil");
        assert_eq!(parsed.results[1].market_price, None);
    }

    #[test]
    fn normalizes_last_updated_stamp() {
        assert_eq!(normalize_updated("2026-10-09T20:05:19+0000\n").unwrap(), "2026-10-09T20:05:19+00:00");
        assert_eq!(
            parse_price_date(&normalize_updated("2026-10-09T23:30:00-0200").unwrap()),
            NaiveDate::from_ymd_opt(2026, 10, 10)
        );
        assert!(normalize_updated("yesterday").is_err());
    }

    #[test]
    fn normal_and_reverse_holo() {
        assert_eq!(prices(&[("Normal", 1.0), ("Reverse Holofoil", 2.0)]), Some((Some(1.0), Some(2.0))));
    }

    #[test]
    fn holofoil_only_is_the_normal_price() {
        assert_eq!(prices(&[("Holofoil", 5.0)]), Some((Some(5.0), None)));
        assert_eq!(prices(&[("Holofoil", 5.0), ("Reverse Holofoil", 3.0)]), Some((Some(5.0), Some(3.0))));
    }

    #[test]
    fn first_edition_and_unlimited() {
        assert_eq!(
            prices(&[("1st Edition Holofoil", 90.0), ("Unlimited Holofoil", 30.0)]),
            Some((Some(30.0), Some(90.0)))
        );
        assert_eq!(prices(&[("1st Edition", 4.0), ("Unlimited", 1.0)]), Some((Some(1.0), None)));
    }

    #[test]
    fn unknown_printings_fall_back_to_cheapest() {
        assert_eq!(prices(&[("Cosmos Holofoil", 4.0), ("Cracked Ice", 3.0)]), Some((Some(3.0), None)));
        assert_eq!(prices(&[]), None);
    }

    #[test]
    fn writes_and_reads_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/prices.sqlite");
        write_prices_db(
            &path,
            "2026-10-09T20:05:19+00:00",
            [
                row(1, "Normal", Some(0.5), Some(0.6)),
                row(1, "Reverse Holofoil", None, Some(1.5)),
                row(2, "Holofoil", Some(0.0), None),
                row(3, "Normal", None, None),
            ],
        )
        .unwrap();

        let conn = open_prices_db(path.to_str().unwrap()).unwrap().unwrap();
        let got = prices_for_products(&conn, &[1, 2, 3, 4]).unwrap();
        // 2 is only quoted at zero and 3 not at all: neither is priced.
        assert_eq!(got.len(), 1);
        let tcgp = &got[&1].paper[RETAILER];
        assert_eq!(tcgp.normal, Some(0.5));
        // No market price: the median listing stands in.
        assert_eq!(tcgp.foil, Some(1.5));
        assert_eq!(tcgp.currency, "USD");
        assert_eq!(tcgp.date, NaiveDate::from_ymd_opt(2026, 10, 9));
    }

    #[test]
    fn rewriting_replaces_the_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.sqlite");
        write_prices_db(&path, "2026-10-08T20:00:00+00:00", [row(1, "Normal", Some(1.0), None)]).unwrap();
        write_prices_db(&path, "2026-10-09T20:00:00+00:00", [row(2, "Normal", Some(2.0), None)]).unwrap();

        let conn = open_prices_db(path.to_str().unwrap()).unwrap().unwrap();
        let got = prices_for_products(&conn, &[1, 2]).unwrap();
        assert_eq!(got.keys().collect::<Vec<_>>(), vec![&2]);
        assert_eq!(got[&2].paper[RETAILER].date, NaiveDate::from_ymd_opt(2026, 10, 9));
    }

    #[test]
    fn old_pokedata_db_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.sqlite");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE prices (date TEXT, cardId TEXT, variant TEXT, rawPrice REAL, gradedPriceTen REAL, gradedPriceNine REAL);",
            )
            .unwrap();
        assert!(open_prices_db(path.to_str().unwrap()).unwrap().is_none());
        assert!(open_prices_db(dir.path().join("missing.sqlite").to_str().unwrap()).unwrap().is_none());
    }
}
