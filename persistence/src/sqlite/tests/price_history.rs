use super::*;
use crate::{PriceHistoryEntry, PricePoint, default_price_history_path};
use chrono::NaiveDate;

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn point(uuid: &str, retailer: &str, finish: &str, price: f64, currency: &str) -> PricePoint {
    point_on(uuid, retailer, finish, price, currency, "2026-01-01")
}

fn point_on(uuid: &str, retailer: &str, finish: &str, price: f64, currency: &str, on: &str) -> PricePoint {
    PricePoint {
        card_uuid: uuid.to_string(),
        retailer: retailer.to_string(),
        finish: finish.to_string(),
        price,
        currency: currency.to_string(),
        recorded_on: day(on),
    }
}

fn entry(retailer: &str, finish: &str, price: f64, currency: &str, on: &str) -> PriceHistoryEntry {
    PriceHistoryEntry {
        retailer: retailer.to_string(),
        finish: finish.to_string(),
        price,
        currency: currency.to_string(),
        recorded_on: day(on),
    }
}

fn with_history() -> SQLitePersistenceSystem {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    p.enable_price_history(true, None).unwrap();
    p
}

#[tokio::test]
async fn disabled_by_default_and_inert() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    assert!(!p.price_history_enabled());
    let written = p
        .record_prices("prov", &[point("c1", "tcg", "", 1.0, "USD")]).await
        .unwrap();
    assert_eq!(written, 0);
    assert!(!p.has_price_history("prov").await.unwrap());
    assert!(p.get_price_history("prov", &"c1".to_string()).await.unwrap().is_empty());
}

#[tokio::test]
async fn records_any_retailer_and_finish() {
    let mut p = with_history();
    assert!(p.price_history_enabled());
    assert!(!p.has_price_history("prov").await.unwrap());

    let written = p
        .record_prices(
            "prov",
            &[
                point("c1", "tcg", "", 1.5, "USD"),
                point("c1", "tcg", "foil", 3.0, "USD"),
                point("c1", "tcg", "etched", 5.0, "USD"),
                point("c1", "cardmarket", "reverse holo", 1.2, "EUR"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(written, 4);
    assert!(p.has_price_history("prov").await.unwrap());
    assert!(!p.has_price_history("other").await.unwrap());

    let history = p.get_price_history("prov", &"c1".to_string()).await.unwrap();
    assert_eq!(
        history,
        vec![
            entry("cardmarket", "reverse holo", 1.2, "EUR", "2026-01-01"),
            entry("tcg", "", 1.5, "USD", "2026-01-01"),
            entry("tcg", "etched", 5.0, "USD", "2026-01-01"),
            entry("tcg", "foil", 3.0, "USD", "2026-01-01"),
        ]
    );
}

#[tokio::test]
async fn same_day_replaces_new_day_appends() {
    let mut p = with_history();
    p.record_prices("prov", &[point("c1", "tcg", "", 1.0, "USD")]).await.unwrap();
    p.record_prices("prov", &[point("c1", "tcg", "", 2.0, "USD")]).await.unwrap();
    p.record_prices("prov", &[point_on("c1", "tcg", "", 4.0, "USD", "2026-01-02")]).await.unwrap();

    let history = p.get_price_history("prov", &"c1".to_string()).await.unwrap();
    let points: Vec<(NaiveDate, f64)> = history.iter().map(|e| (e.recorded_on, e.price)).collect();
    assert_eq!(points, vec![(day("2026-01-01"), 2.0), (day("2026-01-02"), 4.0)]);
}

#[tokio::test]
async fn skips_invalid_prices() {
    let mut p = with_history();
    let written = p
        .record_prices(
            "prov",
            &[
                point("c1", "a", "", 0.0, "USD"),
                point("c1", "a", "foil", -1.0, "USD"),
                point("c1", "b", "", f64::NAN, "USD"),
                point("c1", "b", "foil", f64::INFINITY, "USD"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(written, 0);
    assert!(!p.has_price_history("prov").await.unwrap());
}

#[tokio::test]
async fn history_is_per_provider() {
    let mut p = with_history();
    p.record_prices("a", &[point("c1", "tcg", "", 1.0, "USD")]).await.unwrap();
    assert_eq!(p.get_price_history("a", &"c1".to_string()).await.unwrap().len(), 1);
    assert!(p.get_price_history("b", &"c1".to_string()).await.unwrap().is_empty());
}

#[tokio::test]
async fn retailer_histories_read_one_retailer_per_card() {
    let mut p = with_history();
    p.record_prices(
        "prov",
        &[
            point_on("c1", "tcg", "", 2.0, "USD", "2026-01-02"),
            point_on("c1", "tcg", "foil", 5.0, "USD", "2026-01-01"),
            point_on("c1", "cardmarket", "", 1.0, "EUR", "2026-01-01"),
            point_on("c2", "cardmarket", "", 3.0, "EUR", "2026-01-01"),
            point_on("c2", "tcg", "", 4.0, "USD", "2026-01-01"),
            point_on("c3", "tcg", "", 6.0, "USD", "2026-01-01"),
        ],
    )
    .await
    .unwrap();
    p.record_prices("other", &[point_on("c1", "tcg", "", 9.0, "USD", "2026-01-01")]).await.unwrap();

    let retailers = std::collections::HashMap::from([
        ("c1".to_string(), "tcg".to_string()),
        ("c2".to_string(), "cardmarket".to_string()),
        ("c4".to_string(), "tcg".to_string()),
    ]);
    let histories = p.get_retailer_price_histories("prov", &retailers).await.unwrap();

    let daily = |finish: &str, price: f64, on: &str| crate::DailyPrice { finish: finish.to_string(), price, recorded_on: day(on) };
    assert_eq!(histories.len(), 2, "c3 wasn't asked for, c4 has no history");
    assert_eq!(histories["c1"], vec![daily("foil", 5.0, "2026-01-01"), daily("", 2.0, "2026-01-02")]);
    assert_eq!(histories["c2"], vec![daily("", 3.0, "2026-01-01")]);
}

#[tokio::test]
async fn tracked_uuids_cover_all_collections_and_wants() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let a = p.add_collection("A".to_string()).await.unwrap();
    let b = p.add_collection("B".to_string()).await.unwrap();
    p.add_card_to_collection(&a, &"c1".to_string(), "", 1, OLD_TIME, "prov").await.unwrap();
    p.add_card_to_collection(&a, &"c1".to_string(), "foil", 1, OLD_TIME, "prov").await.unwrap();
    p.add_card_to_collection(&b, &"c1".to_string(), "", 1, OLD_TIME, "prov").await.unwrap();
    p.add_card_to_collection(&b, &"c2".to_string(), "", 1, OLD_TIME, "other").await.unwrap();
    p.adjust_want_quantity(&b, &"c3".to_string(), 2, "prov").await.unwrap();

    assert_eq!(p.tracked_card_uuids("prov").await.unwrap(), vec!["c1".to_string(), "c3".to_string()]);
    assert_eq!(p.tracked_card_uuids("other").await.unwrap(), vec!["c2".to_string()]);
    assert!(p.tracked_card_uuids("none").await.unwrap().is_empty());
}

#[tokio::test]
async fn price_db_is_a_separate_file() {
    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("storage.db").to_string_lossy().into_owned();
    let prices_path = default_price_history_path(&storage);
    assert!(prices_path.ends_with("storage.prices.db"));

    let mut p = SQLitePersistenceSystem::new(false, Some(storage.clone())).unwrap();
    p.enable_price_history(false, Some(prices_path.clone())).unwrap();
    p.record_prices("prov", &[point("c1", "tcg", "", 1.0, "USD")]).await.unwrap();
    drop(p);

    // Reopened, the history is still there.
    let mut p = SQLitePersistenceSystem::new(false, Some(storage)).unwrap();
    p.enable_price_history(false, Some(prices_path)).unwrap();
    assert!(p.has_price_history("prov").await.unwrap());
}

#[tokio::test]
async fn unopenable_price_db_leaves_it_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    // A directory can't be opened as a database.
    assert!(p.enable_price_history(false, Some(dir.path().to_string_lossy().into_owned())).is_err());
    assert!(!p.price_history_enabled());
}

#[tokio::test]
async fn database_only_accepts_real_days() {
    let p = with_history();
    let conn = p.price_connection.as_ref().unwrap().lock().await;
    let insert = |on: &str| {
        conn.execute(
            "INSERT INTO price_history VALUES ('prov', 'c1', 'tcg', '', 1.0, 'USD', ?1)",
            params![on],
        )
    };
    assert!(insert("2026-01-01").is_ok());
    for bad in ["2026-1-1", "2026-02-30", "2026-01-01T00:00:00Z", "yesterday"] {
        assert!(insert(bad).is_err(), "{bad:?} was accepted");
    }
}
