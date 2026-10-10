//! TCGplayer prices, as republished daily by [TCGCSV](https://tcgcsv.com).
//!
//! Each game's prices are a snapshot in one self-contained sqlite file:
//! every printing ("subtype") of every product in the game's TCGplayer
//! category, keyed by product id. Games whose cards db doesn't store
//! TCGplayer product ids (Riftbound) also get a `card_products` table,
//! matched when the snapshot is built, so the file can still be built,
//! mirrored and replaced independently of the cards db.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use ::models::{CardPrices, RetailerPrices, parse_price_date};
use chrono::{DateTime, NaiveDate};
use rusqlite::{Connection, params};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::systems::sql_helpers::sql_placeholders;

const TCGCSV_BASE: &str = "https://tcgcsv.com";
/// TCGCSV asks scrapers to pace themselves; this is its sample code's delay.
const REQUEST_DELAY: Duration = Duration::from_millis(250);
const MAX_ATTEMPTS: u32 = 3;
/// Key prices are listed under in `CardPrices::paper`.
pub(crate) const RETAILER: &str = "tcgplayer";

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
-- The TCGplayer product each card id is, for games whose cards db doesn't
-- say. Empty otherwise.
CREATE TABLE card_products (
    cardId    TEXT PRIMARY KEY,
    productId INTEGER NOT NULL
) WITHOUT ROWID;
-- 'updated': when TCGCSV last refreshed its data, RFC 3339.
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

/// How a game's TCGplayer printings fold into `RetailerPrices`' normal and
/// foil prices. Every finish but "foil" is valued at the normal price (see
/// `server::collections`), so `normal` lists whatever a card's default
/// printing may be called, in order of preference. `foil` is tried in
/// order too, skipping whichever printing was taken as the normal price.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Printings {
    pub normal: &'static [&'static str],
    pub foil: &'static [&'static str],
}

/// A game's prices in TCGCSV: its TCGplayer category, how its printings map
/// to normal/foil, and — for games whose cards db has no TCGplayer product
/// ids — how to name the card a product is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Game {
    pub name: &'static str,
    pub category: u32,
    pub printings: Printings,
    /// The card id of `product` in `group`, or `None` when it isn't a card
    /// the game's db would hold. `None` here means the cards db stores
    /// product ids itself, and no products are fetched.
    pub card_id: Option<fn(&Group, &Product) -> Option<String>>,
}

#[derive(Debug, Deserialize)]
struct TcgcsvResponse<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<String>,
    results: Vec<T>,
}

/// A TCGplayer group: a set, or a promo/product line.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Group {
    pub group_id: i64,
    #[serde(default)]
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Product {
    pub product_id: i64,
    #[serde(default)]
    pub extended_data: Vec<ExtendedData>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ExtendedData {
    pub name: String,
    pub value: String,
}

impl Product {
    /// The value of extended data field `name` (e.g. `Number`), if listed.
    pub fn extended(&self, name: &str) -> Option<&str> {
        self.extended_data
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.value.as_str())
    }
}

/// One printing's prices for a TCGplayer product, as TCGCSV lists them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PriceRow {
    pub product_id: i64,
    pub sub_type_name: String,
    pub low_price: Option<f64>,
    pub mid_price: Option<f64>,
    pub high_price: Option<f64>,
    pub market_price: Option<f64>,
    pub direct_low_price: Option<f64>,
}

/// Opens the prices db at `path`, or `None` when there's no file or it isn't
/// a TCGCSV snapshot (e.g. a leftover db from an earlier price source).
pub(crate) fn open_prices_db(path: &str) -> eyre::Result<Option<Connection>> {
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
        warn!(path, "Prices db isn't a TCGCSV snapshot; ignored until prices are updated");
        return Ok(None);
    }
    Ok(Some(conn))
}

/// The TCGplayer product of each of `card_ids` the snapshot's
/// `card_products` table knows.
pub(crate) fn products_for_cards(conn: &Connection, card_ids: &[String]) -> eyre::Result<Vec<(String, i64)>> {
    if card_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT cardId, productId FROM card_products WHERE cardId IN ({})",
        sql_placeholders(card_ids.len())
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(card_ids), |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Prices of the given TCGplayer products, folded into normal/foil by
/// `printings`. Products with no positive price are left out.
pub(crate) fn prices_for_products(
    conn: &Connection,
    product_ids: &[i64],
    printings: &Printings,
) -> eyre::Result<HashMap<i64, RetailerPrices>> {
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
        .filter_map(|(product_id, subtypes)| Some((product_id, retailer_prices(&subtypes, printings, date)?)))
        .collect())
}

/// Folds a product's printings into a normal and a foil price (see
/// `Printings`). A product listed only under printings neither list knows
/// is priced at the cheapest of them.
fn retailer_prices(
    subtypes: &HashMap<String, f64>,
    printings: &Printings,
    date: Option<NaiveDate>,
) -> Option<RetailerPrices> {
    let normal = printings
        .normal
        .iter()
        .find_map(|name| subtypes.get(*name).map(|p| (*name, *p)));
    let foil = printings
        .foil
        .iter()
        .filter(|name| Some(**name) != normal.map(|(n, _)| n))
        .find_map(|name| subtypes.get(*name).copied());
    let normal = normal
        .map(|(_, p)| p)
        .or_else(|| foil.is_none().then(|| subtypes.values().copied().min_by(f64::total_cmp)).flatten());
    if normal.is_none() && foil.is_none() {
        return None;
    }
    Some(RetailerPrices {
        normal,
        foil,
        currency: "USD".to_string(),
        date,
    })
}

/// Writes a snapshot db at `path` (replacing any file there) from TCGCSV's
/// `updated` timestamp, price rows and card→product matches. Built beside
/// `path` and renamed into place, so readers never see a half-written db.
pub(crate) fn write_prices_db(
    path: &Path,
    updated: &str,
    rows: impl IntoIterator<Item = PriceRow>,
    card_products: impl IntoIterator<Item = (String, i64)>,
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
            let mut insert = tx.prepare("INSERT OR REPLACE INTO card_products (cardId, productId) VALUES (?1, ?2)")?;
            for (card_id, product_id) in card_products {
                insert.execute(params![card_id, product_id])?;
            }
        }
        tx.commit()?;
    }
    staging.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Card→product matches from `game.card_id`, leaving out any card id two
/// products claim: better unpriced than priced as the wrong printing.
fn match_cards(game: &Game, products: &[(Group, Vec<Product>)]) -> Vec<(String, i64)> {
    let Some(card_id) = game.card_id else {
        return Vec::new();
    };
    let mut matches: HashMap<String, Option<i64>> = HashMap::new();
    for (group, products) in products {
        for product in products {
            if let Some(id) = card_id(group, product) {
                matches
                    .entry(id)
                    .and_modify(|m| *m = None)
                    .or_insert(Some(product.product_id));
            }
        }
    }
    let ambiguous = matches.values().filter(|m| m.is_none()).count();
    if ambiguous > 0 {
        warn!(game = game.name, ambiguous, "Card ids matching several TCGplayer products left unpriced");
    }
    matches.into_iter().filter_map(|(id, m)| Some((id, m?))).collect()
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
    tokio::time::sleep(REQUEST_DELAY).await;
    let response: TcgcsvResponse<T> = get_with_retries(client, url).await?.json().await?;
    if !response.success {
        eyre::bail!("TCGCSV reported failure for {url}: {:?}", response.errors);
    }
    Ok(response.results)
}

/// Builds `game`'s prices db at `path` straight from TCGCSV, never from a
/// mirror — the source the mirror itself publishes. Fails without touching
/// `path` if any group can't be fetched, rather than publishing a snapshot
/// missing whole sets.
pub(crate) async fn build_prices_db(game: &Game, path: &Path) -> eyre::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "GatheRs/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/morosanmihail/gathers)"
        ))
        .timeout(Duration::from_secs(60))
        .build()?;
    let category = game.category;

    let updated = normalize_updated(
        &get_with_retries(&client, &format!("{TCGCSV_BASE}/last-updated.txt"))
            .await?
            .text()
            .await?,
    )?;
    let groups: Vec<Group> = get_results(&client, &format!("{TCGCSV_BASE}/tcgplayer/{category}/groups")).await?;
    if groups.is_empty() {
        eyre::bail!("TCGCSV listed no {} groups", game.name);
    }
    info!(game = game.name, groups = groups.len(), updated, "Fetching prices from TCGCSV");

    let mut rows = Vec::new();
    let mut products = Vec::new();
    for group in groups {
        let base = format!("{TCGCSV_BASE}/tcgplayer/{category}/{}", group.group_id);
        rows.extend(get_results::<PriceRow>(&client, &format!("{base}/prices")).await?);
        if game.card_id.is_some() {
            let listed = get_results::<Product>(&client, &format!("{base}/products")).await?;
            products.push((group, listed));
        }
    }
    let card_products = match_cards(game, &products);

    info!(game = game.name, rows = rows.len(), cards = card_products.len(), dest = ?path, "Writing prices");
    write_prices_db(path, &updated, rows, card_products)
}

/// Updates `game`'s prices db at `path`: from the first configured mirror
/// that has `mirror_stem`, else straight from TCGCSV.
pub(crate) async fn download_prices_db(game: &Game, mirror_stem: &str, path: &str) -> eyre::Result<()> {
    let target = PathBuf::from(path);
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    if crate::mirror::try_mirrors(mirror_stem, &target, None).await {
        return Ok(());
    }
    build_prices_db(game, &target).await?;
    info!(game = game.name, dest = ?target, "Prices saved");
    Ok(())
}

/// A retrieval system's handle on its prices db: opened lazily (so a file
/// that appears later, or replaces an old-format one, is picked up), and
/// reset after an update so the next lookup reopens the new file.
#[derive(Debug, Clone)]
pub(crate) struct PricesDb {
    path: Option<String>,
    conn: Arc<Mutex<Option<Connection>>>,
}

impl PricesDb {
    pub fn new(path: Option<String>) -> eyre::Result<Self> {
        let conn = match path {
            Some(ref p) => open_prices_db(p)?,
            None => None,
        };
        Ok(Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Runs `f` on the open db, or returns `None` when there's none.
    pub async fn with<T>(&self, f: impl FnOnce(&Connection) -> eyre::Result<T>) -> eyre::Result<Option<T>> {
        let Some(path) = self.path.as_deref() else {
            return Ok(None);
        };
        let mut guard = self.conn.lock().await;
        if guard.is_none() {
            *guard = open_prices_db(path)?;
        }
        guard.as_ref().map(f).transpose()
    }

    /// Closes the db, so the next lookup reopens whatever is at `path` now.
    pub async fn reset(&self) {
        *self.conn.lock().await = None;
    }
}

/// `CardPrices` for each `(card id, product id)` pair whose product is
/// priced. Several cards may share a product, and so its prices.
pub(crate) fn card_prices(
    conn: &Connection,
    products: Vec<(String, i64)>,
    printings: &Printings,
) -> eyre::Result<HashMap<String, CardPrices>> {
    let mut ids: Vec<i64> = products.iter().map(|(_, id)| *id).collect();
    ids.sort_unstable();
    ids.dedup();
    let by_product = prices_for_products(conn, &ids, printings)?;
    Ok(products
        .into_iter()
        .filter_map(|(card_id, product_id)| {
            let retail = by_product.get(&product_id)?.clone();
            Some((
                card_id.clone(),
                CardPrices {
                    uuid: card_id,
                    paper: HashMap::from([(RETAILER.to_string(), retail)]),
                },
            ))
        })
        .collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn row(product_id: i64, sub_type: &str, market: Option<f64>, mid: Option<f64>) -> PriceRow {
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

    const PRINTINGS: Printings = Printings {
        normal: &["Normal", "Holofoil"],
        foil: &["Foil", "Holofoil"],
    };

    fn prices(subtypes: &[(&str, f64)]) -> Option<(Option<f64>, Option<f64>)> {
        let map = subtypes.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        retailer_prices(&map, &PRINTINGS, None).map(|r| (r.normal, r.foil))
    }

    #[test]
    fn parses_tcgcsv_responses() {
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

        let body = r#"{"success": true, "errors": [], "results": [
            {"productId": 684202, "name": "Abandon", "groupId": 24560, "extendedData": [
                {"name": "Rarity", "displayName": "Rarity", "value": "Uncommon"},
                {"name": "Number", "displayName": "Card Number", "value": "131/219"}]},
            {"productId": 1, "name": "Booster Box", "groupId": 24560, "extendedData": []}
        ]}"#;
        let parsed: TcgcsvResponse<Product> = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.results[0].extended("Number"), Some("131/219"));
        assert_eq!(parsed.results[1].extended("Number"), None);
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
    fn folds_printings_into_normal_and_foil() {
        assert_eq!(prices(&[("Normal", 1.0), ("Foil", 2.0)]), Some((Some(1.0), Some(2.0))));
        // Taken as the normal price, so not the foil one too.
        assert_eq!(prices(&[("Holofoil", 5.0)]), Some((Some(5.0), None)));
        assert_eq!(prices(&[("Normal", 1.0), ("Holofoil", 5.0)]), Some((Some(1.0), Some(5.0))));
    }

    #[test]
    fn foil_only_has_no_normal_price() {
        assert_eq!(prices(&[("Foil", 7.0)]), Some((None, Some(7.0))));
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
                row(1, "Foil", None, Some(1.5)),
                row(2, "Normal", Some(0.0), None),
                row(3, "Normal", None, None),
            ],
            [("card-a".to_string(), 1), ("card-b".to_string(), 2)],
        )
        .unwrap();

        let conn = open_prices_db(path.to_str().unwrap()).unwrap().unwrap();
        let products = products_for_cards(&conn, &["card-a".into(), "card-b".into(), "card-c".into()]).unwrap();
        assert_eq!(products.len(), 2);
        let got = card_prices(&conn, products, &PRINTINGS).unwrap();
        // card-b's product is only quoted at zero: unpriced.
        assert_eq!(got.keys().collect::<Vec<_>>(), vec!["card-a"]);
        let tcgp = &got["card-a"].paper[RETAILER];
        assert_eq!(got["card-a"].uuid, "card-a");
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
        write_prices_db(&path, "2026-10-08T20:00:00+00:00", [row(1, "Normal", Some(1.0), None)], []).unwrap();
        write_prices_db(&path, "2026-10-09T20:00:00+00:00", [row(2, "Normal", Some(2.0), None)], []).unwrap();

        let conn = open_prices_db(path.to_str().unwrap()).unwrap().unwrap();
        let got = prices_for_products(&conn, &[1, 2], &PRINTINGS).unwrap();
        assert_eq!(got.keys().collect::<Vec<_>>(), vec![&2]);
        assert_eq!(got[&2].date, NaiveDate::from_ymd_opt(2026, 10, 9));
    }

    #[test]
    fn other_dbs_are_ignored() {
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

    #[test]
    fn ambiguous_card_ids_are_left_out() {
        let game = Game {
            name: "Test",
            category: 0,
            printings: PRINTINGS,
            card_id: Some(|group, product| {
                Some(format!("{}-{}", group.abbreviation.as_deref()?, product.extended("Number")?))
            }),
        };
        let group = |abbreviation: &str| Group {
            group_id: 0,
            abbreviation: Some(abbreviation.to_string()),
        };
        let product = |product_id, number: &str| Product {
            product_id,
            extended_data: vec![ExtendedData {
                name: "Number".to_string(),
                value: number.to_string(),
            }],
        };
        let mut got = match_cards(
            &game,
            &[
                (group("a"), vec![product(1, "1"), product(2, "2"), product(3, "2")]),
                (group("b"), vec![product(4, "2"), Product { product_id: 5, extended_data: vec![] }]),
            ],
        );
        got.sort();
        assert_eq!(got, vec![("a-1".to_string(), 1), ("b-2".to_string(), 4)]);
    }
}
