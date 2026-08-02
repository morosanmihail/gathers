use super::*;
use ::models::pokemon::EnergyType;
use rusqlite::Connection;
use tempfile::TempDir;

async fn setup_test_db() -> PokemonSQLiteRetrievalSystem {
    PokemonSQLiteRetrievalSystem::new(None, None).unwrap()
}

#[tokio::test]
async fn test_new_with_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let system =
        PokemonSQLiteRetrievalSystem::new(Some(db_path.to_string_lossy().to_string()), None);
    assert!(system.is_ok());
    let system = system.unwrap();
    assert_eq!(system._db_path, db_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_search_by_name() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        name: Some("Bulbasaur".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(2))
        .await
        .unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().all(|c| {
        if let Card::Pokemon(p) = c {
            p.name.contains("Bulbasaur")
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_search_by_name_partial() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        name: Some("charme".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    for card in cards {
        if let Card::Pokemon(p) = card {
            assert!(p.name.contains("Charmeleon"))
        }
    }
}

#[tokio::test]
async fn test_search_by_set_code() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        set_code: Some("Jungle".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    for card in cards {
        if let Card::Pokemon(p) = card {
            assert_eq!(p.set_code, "Jungle");
        } else {
            panic!("expected Pokemon card");
        }
    }
}

#[tokio::test]
async fn test_search_by_collector_number() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        collector_number: Some("63".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    if let Card::Pokemon(p) = &cards[0] {
        assert_eq!(p.collector_number, "063");
    } else {
        panic!("expected Pokemon card");
    }
}

#[tokio::test]
async fn test_search_by_energy_type() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        energy_types: Some(vec![EnergyType::Fire]),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    assert!(cards.iter().all(|c| {
        if let Card::Pokemon(p) = c {
            p.energy_types.contains(&EnergyType::Fire)
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_search_by_card_type() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        types: Some(vec!["Trainer".to_string()]),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    for card in cards {
        if let Card::Pokemon(p) = card {
            assert_eq!(p.card_type, "Trainer");
            assert!(p.pokedex.is_none());
        } else {
            panic!("expected Pokemon card");
        }
    }
}

#[tokio::test]
async fn test_search_with_limit() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters::default();
    let cards = system
        .search_cards(filters, Some(0), Some(3))
        .await
        .unwrap();
    assert_eq!(cards.len(), 3);
}

#[tokio::test]
async fn test_search_with_skip_and_limit() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters::default();
    let all = system
        .search_cards(filters.clone(), Some(0), Some(10))
        .await
        .unwrap();
    let page2 = system
        .search_cards(filters, Some(3), Some(3))
        .await
        .unwrap();
    assert_eq!(all.len(), 10);
    assert_eq!(page2.len(), 3);
}

#[tokio::test]
async fn test_search_empty_result() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        name: Some("Cucuriguuuuu".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert!(cards.is_empty());
}

#[tokio::test]
async fn test_get_cards_by_ids() {
    let system = setup_test_db().await;
    let ids = vec![
        "Pokemon-Go-Bulbasaur-001".to_string(),
        "Supreme-Victors-Bulbasaur-93".to_string(),
    ];
    let cards = system.get_cards_by_ids(ids).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.contains_key("Pokemon-Go-Bulbasaur-001"));
    assert!(cards.contains_key("Supreme-Victors-Bulbasaur-93"));
    if let Card::Pokemon(p) = &cards["Pokemon-Go-Bulbasaur-001"] {
        assert_eq!(p.name, "Bulbasaur");
    } else {
        panic!("expected Pokemon card");
    }
    if let Card::Pokemon(p) = &cards["Supreme-Victors-Bulbasaur-93"] {
        assert_eq!(p.name, "Bulbasaur");
    } else {
        panic!("expected Pokemon card");
    }
}

#[tokio::test]
async fn test_get_cards_by_ids_empty() {
    let system = setup_test_db().await;
    let cards = system.get_cards_by_ids(vec![]).await.unwrap();
    assert!(cards.is_empty());
}

#[tokio::test]
async fn test_get_sets() {
    let system = setup_test_db().await;
    let sets = system.get_sets().await.unwrap();
    assert!(sets.len() >= 157);
    let codes: Vec<&str> = sets.iter().map(|s| s.code.as_str()).collect();
    assert!(codes.contains(&"Base Set"));
    assert!(codes.contains(&"Jungle"));
}

#[tokio::test]
async fn test_bulk_search_cards() {
    let system = setup_test_db().await;
    let query = vec![
        ("Base Set".to_string(), "044".to_string()),
        ("Base Set".to_string(), "004".to_string()),
    ];
    let results = system.bulk_search_cards(query).await.unwrap();
    assert_eq!(results.len(), 4);

    println!("{results:?}");
    let bulbasaur = results
        .iter()
        .find(|r| r.2 == "Base-Set-Bulbasaur-044")
        .unwrap();
    assert_eq!(bulbasaur.0, "Base Set");
    assert_eq!(bulbasaur.1, "044");
    let charizard = results
        .iter()
        .find(|r| r.2 == "Base-Set-Shadowless-Charizard-004")
        .unwrap();
    assert_eq!(charizard.0, "Base Set");
    assert_eq!(charizard.1, "004");
}

#[tokio::test]
async fn test_bulk_search_cards_empty() {
    let system = setup_test_db().await;
    let results = system.bulk_search_cards(vec![]).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_named_retrieval_system_trait() {
    let system = setup_test_db().await;
    assert_eq!(system.name(), "PokemonSQLite");
}

#[tokio::test]
async fn test_pokedex_is_none_for_trainers() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        name: Some("Professor Oak".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(1))
        .await
        .unwrap();
    assert_eq!(cards.len(), 1);
    if let Card::Pokemon(p) = &cards[0] {
        assert!(p.pokedex.is_none());
    } else {
        panic!("expected Pokemon card");
    }
}

#[tokio::test]
async fn test_search_returns_description_and_release_date() {
    let system = setup_test_db().await;
    let ids = vec!["Pokemon-Go-Bulbasaur-001".to_string()];
    let cards = system.get_cards_by_ids(ids).await.unwrap();
    if let Card::Pokemon(p) = &cards["Pokemon-Go-Bulbasaur-001"] {
        assert_eq!(p.release_date.as_deref(), Some("2022-07-01T00:00:00Z"));
        assert_eq!(p.pokedex, Some(1));
    } else {
        panic!("expected Pokemon card");
    }
}

#[tokio::test]
async fn test_search_by_text_matches_description() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        text: Some("Last Gift".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert!(!cards.is_empty());
    assert!(cards.iter().all(|c| {
        if let Card::Pokemon(p) = c {
            p.description
                .as_deref()
                .is_some_and(|d| d.contains("Last Gift"))
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_search_by_pokedex() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        pokedex: Some(1),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(30))
        .await
        .unwrap();
    assert_eq!(cards.len(), 23);
    assert!(cards.iter().all(|c| {
        if let Card::Pokemon(p) = c {
            p.pokedex == Some(1) && p.name.contains("Bulbasaur")
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_search_sort_by_release_date() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        name: Some("Bulbasaur".to_string()),
        sort_by: Some(::models::filters::SortField::ReleaseDate),
        sort_order: Some(::models::filters::SortOrder::Asc),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(50))
        .await
        .unwrap();
    let dates: Vec<Option<String>> = cards
        .iter()
        .map(|c| match c {
            Card::Pokemon(p) => p.release_date.clone(),
            _ => None,
        })
        .collect();
    let mut sorted = dates.clone();
    sorted.sort();
    assert_eq!(dates, sorted);
}

// ── Price tests ───────────────────────────────────────────────────────────

fn make_prices_db(dir: &TempDir) -> String {
    let path = dir.path().join("prices.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE prices (date TEXT, cardId TEXT, variant TEXT, rawPrice REAL, gradedPriceTen REAL, gradedPriceNine REAL);
         INSERT INTO prices VALUES ('2024-01-01', 'card-alpha', '', 1.50, 10.0, 8.0);
         INSERT INTO prices VALUES ('2024-01-10', 'card-alpha', '', 2.00, 12.0, 9.0);
         INSERT INTO prices VALUES ('2024-01-01', 'card-beta',  '', 0.25, 0.0,  0.0);
         INSERT INTO prices VALUES ('2024-01-01', 'card-zero',  '', 0.0,  0.0,  0.0);",
    ).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn test_get_card_prices_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let result = system.get_card_prices("card-alpha").await.unwrap();
    assert!(result.is_some());
    let prices = result.unwrap();
    assert_eq!(prices.uuid, "card-alpha");
    let raw = prices.paper.get("raw").unwrap();
    assert_eq!(raw.normal, Some(2.00));
    assert_eq!(raw.foil, None);
    let psa10 = prices.paper.get("graded_psa10").unwrap();
    assert_eq!(psa10.normal, Some(12.0));
    let psa9 = prices.paper.get("graded_psa9").unwrap();
    assert_eq!(psa9.normal, Some(9.0));
}

#[tokio::test]
async fn test_get_card_prices_latest_row_used() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    // card-alpha has two rows; latest (2024-01-10) must win
    let prices = system.get_card_prices("card-alpha").await.unwrap().unwrap();
    assert_eq!(prices.paper.get("raw").unwrap().normal, Some(2.00));
}

#[tokio::test]
async fn test_get_card_prices_not_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let result = system.get_card_prices("card-nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_card_prices_all_zero_returns_none() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    // card-zero has all prices = 0.0 → paper map is empty → None
    let result = system.get_card_prices("card-zero").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_card_prices_no_prices_path() {
    let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system.get_card_prices("card-alpha").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_card_prices_file_missing() {
    let system = PokemonSQLiteRetrievalSystem::new(
        None,
        Some("/tmp/does_not_exist_pokemon_prices.sqlite".to_string()),
    )
    .unwrap();
    let result = system.get_card_prices("card-alpha").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_bulk_card_prices_all_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let result = system
        .get_bulk_card_prices(vec!["card-alpha".to_string(), "card-beta".to_string()])
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("card-alpha"));
    assert!(result.contains_key("card-beta"));
}

#[tokio::test]
async fn test_get_bulk_card_prices_partial_found() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let result = system
        .get_bulk_card_prices(vec!["card-alpha".to_string(), "card-missing".to_string()])
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result.contains_key("card-alpha"));
}

#[tokio::test]
async fn test_get_bulk_card_prices_empty_input() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let result = system.get_bulk_card_prices(vec![]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_bulk_card_prices_no_prices_path() {
    let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system
        .get_bulk_card_prices(vec!["card-alpha".to_string()])
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_update_prices_no_path_returns_false() {
    let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
    let result = system.update_prices().await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_prices_beta_raw_only() {
    let dir = TempDir::new().unwrap();
    let prices_path = make_prices_db(&dir);
    let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

    let prices = system.get_card_prices("card-beta").await.unwrap().unwrap();
    assert_eq!(prices.paper.len(), 1);
    assert_eq!(prices.paper.get("raw").unwrap().normal, Some(0.25));
    assert!(!prices.paper.contains_key("graded_psa10"));
    assert!(!prices.paper.contains_key("graded_psa9"));
}
