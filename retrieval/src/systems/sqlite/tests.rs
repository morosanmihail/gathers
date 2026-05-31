use super::*;
use ::models::{CardColour, filters::SortOrder};
use tempfile::TempDir;

#[tokio::test]
async fn test_new_with_none() {
    let system = MagicSQLiteRetrievalSystem::new(None, None);
    assert!(system.is_ok());
    let system = system.unwrap();
    assert!(!system.db_path.is_empty());
}

#[tokio::test]
async fn test_new_with_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let custom_path = temp_dir.path().join("test.db");
    let system =
        MagicSQLiteRetrievalSystem::new(Some(custom_path.to_string_lossy().to_string()), None);
    assert!(system.is_ok());
    let system = system.unwrap();
    assert_eq!(system.db_path, custom_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_search_cards_with_name_filter() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        name: Some("Goblin King".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(!cards.is_empty());
}

#[tokio::test]
async fn test_search_cards_with_color_identity_filter() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        color_identities: Some(vec![CardColour::Black]),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(!cards.is_empty());
}

#[tokio::test]
async fn test_search_cards_with_artist_filter() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        artist: Some("Jason Chan".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(!cards.is_empty());
}

#[tokio::test]
async fn test_search_cards_with_text_filter() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        text: Some("destroy target enchantment".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(!cards.is_empty());
}

#[tokio::test]
async fn test_search_cards_with_set_code_filter() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        set_code: Some("M20".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(!cards.is_empty());
}

#[tokio::test]
async fn test_search_cards_with_skip_and_limit() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        name: Some("Rule of Law".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, Some(6), Some(5)).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(cards.len() <= 5);
}

#[tokio::test]
async fn test_search_cards_empty_result() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        name: Some("NonExistentCardXYZ123".to_string()),
        ..Default::default()
    };
    let result = system.search_cards(filters, None, None).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(cards.is_empty());
}

#[tokio::test]
async fn test_get_cards_by_ids() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let ids = vec![
        "0003caab-9ff5-5d1a-bc06-976dd0457f19".to_string(),
        "0005d268-3fd0-5424-bc6b-573ecd713aa1".to_string(),
    ];
    let result = system.get_cards_by_ids(ids).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert_eq!(cards.len(), 2);
}

#[tokio::test]
async fn test_get_cards_by_empty_ids() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system.get_cards_by_ids(vec![]).await;
    assert!(result.is_ok());
    let cards = result.unwrap();
    assert!(cards.is_empty());
}

#[tokio::test]
async fn test_get_sets() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system.get_sets().await;
    assert!(result.is_ok());
    let sets = result.unwrap();
    assert!(!sets.is_empty());
}

#[tokio::test]
async fn test_bulk_search_cards() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let cards = vec![
        (
            SetCode::from_str("TLE").unwrap(),
            CollectorNumber::from_str("12").unwrap(),
        ),
        (
            SetCode::from_str("ARB").unwrap(),
            CollectorNumber::from_str("52").unwrap(),
        ),
    ];
    let result = system.bulk_search_cards(cards).await;
    assert!(result.is_ok());
    let results = result.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_bulk_search_cards_empty() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system.bulk_search_cards(vec![]).await;
    assert!(result.is_ok());
    let results = result.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_named_retrieval_system_trait() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let name = system.name();
    assert_eq!(name, "MagicSQLite");
}

fn card_name(c: &::models::Card) -> String {
    match c {
        ::models::Card::Magic(m) => m.name.to_lowercase(),
        _ => String::new(),
    }
}

fn card_types(c: &::models::Card) -> Vec<String> {
    match c {
        ::models::Card::Magic(m) => m.types.clone(),
        _ => vec![],
    }
}

fn card_rarity(c: &::models::Card) -> String {
    match c {
        ::models::Card::Magic(m) => format!("{:?}", m.rarity).to_lowercase(),
        _ => String::new(),
    }
}

fn card_set_code(c: &::models::Card) -> String {
    match c {
        ::models::Card::Magic(m) => m.set_code.to_lowercase(),
        _ => String::new(),
    }
}

#[tokio::test]
async fn test_search_cards_sort_by_name_asc() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        sort_by: Some(SortField::Name),
        sort_order: Some(SortOrder::Asc),
        ..Default::default()
    };
    let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
    assert!(!cards.is_empty());
    let names: Vec<_> = cards.iter().map(card_name).collect();
    for w in names.windows(2) {
        assert!(w[0] <= w[1], "name order violated: {:?} > {:?}", w[0], w[1]);
    }
}

#[tokio::test]
async fn test_search_cards_sort_by_name_desc() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        sort_by: Some(SortField::Name),
        sort_order: Some(SortOrder::Desc),
        ..Default::default()
    };
    let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
    assert!(!cards.is_empty());
    let names: Vec<_> = cards.iter().map(card_name).collect();
    for w in names.windows(2) {
        assert!(w[0] >= w[1], "name desc order violated: {:?} < {:?}", w[0], w[1]);
    }
}

#[tokio::test]
async fn test_search_cards_sort_by_rarity_asc() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        sort_by: Some(SortField::Rarity),
        sort_order: Some(SortOrder::Asc),
        ..Default::default()
    };
    let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
    assert!(!cards.is_empty());
    let rarities: Vec<_> = cards.iter().map(card_rarity).collect();
    for w in rarities.windows(2) {
        assert!(w[0] <= w[1], "rarity order violated: {:?} > {:?}", w[0], w[1]);
    }
}

#[tokio::test]
async fn test_search_cards_sort_by_set_code() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        sort_by: Some(SortField::SetCode),
        sort_order: Some(SortOrder::Asc),
        ..Default::default()
    };
    let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
    assert!(!cards.is_empty());
    let set_codes: Vec<_> = cards.iter().map(card_set_code).collect();
    for w in set_codes.windows(2) {
        assert!(w[0] <= w[1], "set_code order violated: {:?} > {:?}", w[0], w[1]);
    }
}

#[tokio::test]
async fn test_search_cards_default_sort_is_name_asc() {
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters_default = CardSearchFilters::default();
    let filters_explicit = CardSearchFilters {
        sort_by: Some(SortField::Name),
        sort_order: Some(SortOrder::Asc),
        ..Default::default()
    };
    let default_cards = system.search_cards(filters_default, None, Some(10)).await.unwrap();
    let explicit_cards = system.search_cards(filters_explicit, None, Some(10)).await.unwrap();
    let default_names: Vec<_> = default_cards.iter().map(card_name).collect();
    let explicit_names: Vec<_> = explicit_cards.iter().map(card_name).collect();
    assert_eq!(default_names, explicit_names);
}

// ── Multi-type filter tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_search_cards_multiple_types_mutually_exclusive_returns_empty() {
    // Regression: the types loop was missing `i += 1`, so all conditions
    // reused the same parameter index and effectively checked only the
    // first type. With two mutually-exclusive types the old code returned
    // the same results as a single-type filter; the fix returns zero.
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let filters = CardSearchFilters {
        // No card can be both a Creature and a Sorcery.
        types: Some(vec!["Creature".to_string(), "Sorcery".to_string()]),
        ..Default::default()
    };
    let cards = system.search_cards(filters, None, Some(200)).await.unwrap();
    // card_types used here so the helper is not flagged dead_code.
    assert!(
        cards.iter().all(|c| {
            let t = card_types(c);
            t.iter().any(|x| x.eq_ignore_ascii_case("Creature"))
                && t.iter().any(|x| x.eq_ignore_ascii_case("Sorcery"))
        }),
        "no card can be both Creature and Sorcery, got {} results",
        cards.len()
    );
    assert!(cards.is_empty(), "expected zero results for impossible type combo");
}

#[tokio::test]
async fn test_search_cards_two_type_filter_is_stricter_than_one() {
    // Regression: before the fix both conditions resolved to the same
    // parameter, making the two-type filter identical to the one-type
    // filter. After the fix, AND semantics hold and the result set shrinks.
    let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
    let single = CardSearchFilters {
        types: Some(vec!["Creature".to_string()]),
        ..Default::default()
    };
    let dual = CardSearchFilters {
        // Creatures-only vs Creature-AND-Sorcery (impossible combo → 0 results).
        types: Some(vec!["Creature".to_string(), "Sorcery".to_string()]),
        ..Default::default()
    };
    let single_count = system.search_cards(single, None, Some(200)).await.unwrap().len();
    let dual_count = system.search_cards(dual, None, Some(200)).await.unwrap().len();
    assert!(
        single_count > dual_count,
        "one-type filter ({single_count}) should return more results than mutually-exclusive two-type AND ({dual_count})"
    );
}

// ── Price tests ───────────────────────────────────────────────────────────

// entries: (uuid, source, provider, priceType, finish, price)
fn create_prices_db(dir: &TempDir, entries: &[(&str, &str, &str, &str, &str, f64)]) -> String {
    let path = dir.path().join("prices.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE prices (uuid TEXT, date TEXT, source TEXT, provider TEXT, priceType TEXT, finish TEXT, price REAL, currency TEXT)",
    ).unwrap();
    let mut stmt = conn
        .prepare(
            "INSERT INTO prices (uuid, date, source, provider, priceType, finish, price, currency) VALUES (?1, '2026-05-22', ?2, ?3, ?4, ?5, ?6, 'USD')",
        )
        .unwrap();
    for (uuid, source, provider, price_type, finish, price) in entries {
        stmt.execute(rusqlite::params![uuid, source, provider, price_type, finish, price])
            .unwrap();
    }
    path.to_string_lossy().into_owned()
}

fn write_dummy_prices(dir: &TempDir) -> String {
    create_prices_db(dir, &[
        ("uuid-alpha", "paper", "cardkingdom", "retail", "normal", 1.50),
        ("uuid-alpha", "paper", "cardkingdom", "retail", "foil",   3.00),
        ("uuid-alpha", "paper", "tcgplayer",   "retail", "normal", 1.25),
        ("uuid-beta",  "paper", "cardkingdom", "retail", "normal", 0.25),
    ])
}

// Snapshot of three real UUIDs from AllPricesToday (2026-05-22).
// uuid-00010d56: foil-only for most retailers, cardmarket has both
// uuid-0001e0d0: normal-only retail entries
// uuid-0003caab: both normal and foil; also has mtgo and buylist rows (must be ignored)
fn write_real_prices(dir: &TempDir) -> String {
    create_prices_db(dir, &[
        // 00010d56
        ("00010d56-fe38-5e35-8aed-518019aa36a5", "paper", "cardmarket",  "retail",  "normal", 3.07),
        ("00010d56-fe38-5e35-8aed-518019aa36a5", "paper", "cardmarket",  "retail",  "foil",   4.44),
        ("00010d56-fe38-5e35-8aed-518019aa36a5", "paper", "manapool",    "retail",  "foil",   11.23),
        ("00010d56-fe38-5e35-8aed-518019aa36a5", "paper", "cardkingdom", "retail",  "foil",   11.99),
        ("00010d56-fe38-5e35-8aed-518019aa36a5", "paper", "tcgplayer",   "retail",  "foil",   12.63),
        // 0001e0d0
        ("0001e0d0-2dcd-5640-aadc-a84765cf5fc9", "paper", "cardkingdom", "retail",  "normal", 7.49),
        ("0001e0d0-2dcd-5640-aadc-a84765cf5fc9", "paper", "cardmarket",  "retail",  "normal", 4.78),
        ("0001e0d0-2dcd-5640-aadc-a84765cf5fc9", "paper", "manapool",    "retail",  "normal", 4.12),
        ("0001e0d0-2dcd-5640-aadc-a84765cf5fc9", "paper", "tcgplayer",   "retail",  "normal", 5.89),
        // 0003caab — paper retail
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "manapool",    "retail",  "normal", 0.15),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "manapool",    "retail",  "foil",   0.48),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "tcgplayer",   "retail",  "foil",   2.04),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "tcgplayer",   "retail",  "normal", 0.16),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "cardkingdom", "retail",  "foil",   2.49),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "cardkingdom", "retail",  "normal", 0.35),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "cardmarket",  "retail",  "normal", 0.19),
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "cardmarket",  "retail",  "foil",   1.02),
        // 0003caab — buylist (must be ignored)
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "paper", "cardkingdom", "buylist", "foil",   0.75),
        // 0003caab — mtgo (must be ignored)
        ("0003caab-9ff5-5d1a-bc06-976dd0457f19", "mtgo",  "cardhoarder", "retail",  "normal", 0.03),
    ])
}

fn system_with_prices(prices_path: Option<String>) -> MagicSQLiteRetrievalSystem {
    MagicSQLiteRetrievalSystem::new(None, prices_path).unwrap()
}

#[tokio::test]
async fn test_get_card_prices_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system.get_card_prices("uuid-alpha").await.unwrap();
    assert!(result.is_some());
    let prices = result.unwrap();
    assert_eq!(prices.uuid, "uuid-alpha");
    assert_eq!(prices.paper.len(), 2);
    let ck = prices.paper.get("cardkingdom").unwrap();
    assert_eq!(ck.normal, Some(1.50));
    assert_eq!(ck.foil, Some(3.00));
    let tcp = prices.paper.get("tcgplayer").unwrap();
    assert_eq!(tcp.normal, Some(1.25));
    assert_eq!(tcp.foil, None);
}

#[tokio::test]
async fn test_get_card_prices_not_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system.get_card_prices("uuid-nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_card_prices_no_prices_path() {
    let system = system_with_prices(None);
    let result = system.get_card_prices("uuid-alpha").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_card_prices_file_missing() {
    let system = system_with_prices(Some("/tmp/does_not_exist_prices.sqlite".to_string()));
    let result = system.get_card_prices("uuid-alpha").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_bulk_card_prices_all_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system
        .get_bulk_card_prices(vec!["uuid-alpha".to_string(), "uuid-beta".to_string()])
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("uuid-alpha"));
    assert!(result.contains_key("uuid-beta"));
}

#[tokio::test]
async fn test_get_bulk_card_prices_partial_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system
        .get_bulk_card_prices(vec![
            "uuid-alpha".to_string(),
            "uuid-missing".to_string(),
            "uuid-also-missing".to_string(),
        ])
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result.contains_key("uuid-alpha"));
    assert!(!result.contains_key("uuid-missing"));
}

#[tokio::test]
async fn test_get_bulk_card_prices_none_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system
        .get_bulk_card_prices(vec!["uuid-x".to_string(), "uuid-y".to_string()])
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_bulk_card_prices_empty_input() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system.get_bulk_card_prices(vec![]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_bulk_card_prices_no_prices_path() {
    let system = system_with_prices(None);
    let result = system
        .get_bulk_card_prices(vec!["uuid-alpha".to_string()])
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_bulk_card_prices_file_missing() {
    let system = system_with_prices(Some("/tmp/does_not_exist_prices.sqlite".to_string()));
    let result = system
        .get_bulk_card_prices(vec!["uuid-alpha".to_string()])
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_prices_cache_reuse() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let first = system.get_card_prices("uuid-alpha").await.unwrap();
    let second = system.get_card_prices("uuid-beta").await.unwrap();
    // Both calls succeed without error, proving cache is reused after first load.
    assert!(first.is_some());
    assert!(second.is_some());
    assert_eq!(first.unwrap().uuid, "uuid-alpha");
    assert_eq!(second.unwrap().uuid, "uuid-beta");
}

#[tokio::test]
async fn test_update_prices_no_path_returns_false() {
    let system = system_with_prices(None);
    let result = system.update_prices().await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_prices_prices_data_correctness() {
    let dir = TempDir::new().unwrap();
    let prices_path = write_dummy_prices(&dir);
    let system = system_with_prices(Some(prices_path));

    let result = system
        .get_bulk_card_prices(vec!["uuid-alpha".to_string(), "uuid-beta".to_string()])
        .await
        .unwrap();

    let beta = result.get("uuid-beta").unwrap();
    assert_eq!(beta.paper.len(), 1);
    let ck = beta.paper.get("cardkingdom").unwrap();
    assert_eq!(ck.normal, Some(0.25));
    assert_eq!(ck.foil, None);
}

// ── Real-snapshot tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_real_snapshot_all_retailers_present() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    let prices = system
        .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
        .await
        .unwrap()
        .unwrap();
    // Four retailers in the paper section.
    assert_eq!(prices.paper.len(), 4);
    for retailer in ["cardmarket", "manapool", "cardkingdom", "tcgplayer"] {
        assert!(prices.paper.contains_key(retailer), "missing {retailer}");
    }
}

#[tokio::test]
async fn test_real_snapshot_foil_only_retailer() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    let prices = system
        .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
        .await
        .unwrap()
        .unwrap();
    // manapool has only foil retail for this card.
    let manapool = prices.paper.get("manapool").unwrap();
    assert_eq!(manapool.foil, Some(11.23));
    assert_eq!(manapool.normal, None);
    // cardkingdom also foil-only retail.
    let ck = prices.paper.get("cardkingdom").unwrap();
    assert_eq!(ck.foil, Some(11.99));
    assert_eq!(ck.normal, None);
}

#[tokio::test]
async fn test_real_snapshot_both_normal_and_foil() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    let prices = system
        .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
        .await
        .unwrap()
        .unwrap();
    // cardmarket has both normal and foil retail for this card.
    let cm = prices.paper.get("cardmarket").unwrap();
    assert_eq!(cm.normal, Some(3.07));
    assert_eq!(cm.foil, Some(4.44));
}

#[tokio::test]
async fn test_real_snapshot_normal_only_card() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    let prices = system
        .get_card_prices("0001e0d0-2dcd-5640-aadc-a84765cf5fc9")
        .await
        .unwrap()
        .unwrap();
    // All four retailers have normal prices but no foil.
    assert_eq!(prices.paper.len(), 4);
    for (_, rp) in &prices.paper {
        assert!(rp.normal.is_some(), "expected normal price");
        assert_eq!(rp.foil, None, "expected no foil price");
    }
    assert_eq!(prices.paper.get("cardkingdom").unwrap().normal, Some(7.49));
    assert_eq!(prices.paper.get("tcgplayer").unwrap().normal, Some(5.89));
}

#[tokio::test]
async fn test_real_snapshot_mtgo_section_not_in_paper() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    // uuid-0003caab has an "mtgo" row; it must NOT appear in paper.
    let prices = system
        .get_card_prices("0003caab-9ff5-5d1a-bc06-976dd0457f19")
        .await
        .unwrap()
        .unwrap();
    assert!(!prices.paper.contains_key("cardhoarder"), "mtgo retailer leaked into paper");
    assert_eq!(prices.paper.len(), 4);
}

#[tokio::test]
async fn test_real_snapshot_bulk_returns_all_three() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    let uuids = vec![
        "00010d56-fe38-5e35-8aed-518019aa36a5".to_string(),
        "0001e0d0-2dcd-5640-aadc-a84765cf5fc9".to_string(),
        "0003caab-9ff5-5d1a-bc06-976dd0457f19".to_string(),
        "not-in-file".to_string(),
    ];
    let result = system.get_bulk_card_prices(uuids).await.unwrap();
    // 3 found, 1 missing — missing card must not cause failure.
    assert_eq!(result.len(), 3);
    assert!(!result.contains_key("not-in-file"));
}

#[tokio::test]
async fn test_real_snapshot_buylist_not_in_retail() {
    let dir = TempDir::new().unwrap();
    let system = system_with_prices(Some(write_real_prices(&dir)));

    // cardkingdom for uuid-0003caab: retail foil=2.49, normal=0.35.
    // buylist foil=0.75 must NOT appear as the retail price.
    let prices = system
        .get_card_prices("0003caab-9ff5-5d1a-bc06-976dd0457f19")
        .await
        .unwrap()
        .unwrap();
    let ck = prices.paper.get("cardkingdom").unwrap();
    assert_eq!(ck.normal, Some(0.35));
    assert_eq!(ck.foil, Some(2.49));
}
