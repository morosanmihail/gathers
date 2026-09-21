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
async fn test_finishes_sourced_from_variants() {
    let system = setup_test_db().await;
    let cards = system
        .get_cards_by_ids(vec!["Pokemon-Go-Bulbasaur-001".to_string()])
        .await
        .unwrap();
    let card = cards.get("Pokemon-Go-Bulbasaur-001").unwrap();
    if let Card::Pokemon(p) = card {
        assert_eq!(p.finishes, vec!["Normal".to_string(), "Reverse Holofoil".to_string()]);
    } else {
        panic!("expected Pokemon card");
    }
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
async fn test_search_by_short_set_code() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        set_code: Some("PAR".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    for card in cards {
        if let Card::Pokemon(p) = card {
            assert_eq!(p.set_code, "Paradox Rift");
            assert_eq!(p.set_short_code.as_deref(), Some("PAR"));
        } else {
            panic!("expected Pokemon card");
        }
    }
}

#[tokio::test]
async fn test_search_by_short_set_code_lowercase() {
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        set_code: Some("par".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(10))
        .await
        .unwrap();
    assert_eq!(cards.len(), 10);
    for card in cards {
        if let Card::Pokemon(p) = card {
            assert_eq!(p.set_code, "Paradox Rift");
        } else {
            panic!("expected Pokemon card");
        }
    }
}

#[tokio::test]
async fn test_search_by_short_set_code_excludes_unrelated_name_match() {
    // "PAR" is also a substring of the unrelated "Surging Sparks" set name —
    // an exact short-code match should not pull those cards in.
    let system = setup_test_db().await;
    let filters = CardSearchFilters {
        set_code: Some("PAR".to_string()),
        ..Default::default()
    };
    let cards = system
        .search_cards(filters, Some(0), Some(500))
        .await
        .unwrap();
    assert!(cards.iter().all(|c| {
        if let Card::Pokemon(p) = c {
            p.set_code == "Paradox Rift"
        } else {
            false
        }
    }));
}

#[tokio::test]
async fn test_search_returns_set_short_code() {
    let system = setup_test_db().await;
    let ids = vec!["Paradox-Rift-Iron-Moth-028".to_string()];
    let cards = system.get_cards_by_ids(ids).await.unwrap();
    if let Card::Pokemon(p) = &cards["Paradox-Rift-Iron-Moth-028"] {
        assert_eq!(p.set_short_code.as_deref(), Some("PAR"));
    } else {
        panic!("expected Pokemon card");
    }
}

#[tokio::test]
async fn test_set_short_code_none_when_missing() {
    let system = setup_test_db().await;
    let ids = vec!["Silver-Tempest-Serena-164".to_string()];
    let cards = system.get_cards_by_ids(ids).await.unwrap();
    if let Card::Pokemon(p) = &cards["Silver-Tempest-Serena-164"] {
        assert_eq!(p.set_short_code, None);
    } else {
        panic!("expected Pokemon card");
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
    let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Base Set"));
    assert!(names.contains(&"Jungle"));
}

#[tokio::test]
async fn test_get_sets_returns_short_code() {
    let system = setup_test_db().await;
    let sets = system.get_sets().await.unwrap();
    let paradox_rift = sets.iter().find(|s| s.name == "Paradox Rift").unwrap();
    assert_eq!(paradox_rift.code, "PAR");
    let jungle = sets.iter().find(|s| s.name == "Jungle").unwrap();
    assert_eq!(jungle.code, "JU");
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
async fn test_get_random_card() {
    let system = setup_test_db().await;
    let result = system.get_random_card().await;
    assert!(result.is_ok());
    let card = result.unwrap();
    assert!(card.is_some());
    assert!(matches!(card.unwrap(), Card::Pokemon(_)));
}

#[tokio::test]
async fn test_get_random_card_varies() {
    let system = setup_test_db().await;
    let mut names = std::collections::HashSet::new();
    for _ in 0..20 {
        let card = system.get_random_card().await.unwrap().unwrap();
        if let Card::Pokemon(p) = card {
            names.insert(p.name);
        }
    }
    assert!(names.len() > 1, "expected varying random cards, got {names:?}");
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

// ── unique modes ─────────────────────────────────────────────────────────────

/// A small database for the `unique` modes.
///
/// | id       | name                        | notes                                        |
/// |----------|-----------------------------|----------------------------------------------|
/// | pk-old   | Pikachu                     | Old Set 1999, dex 25                         |
/// | pk-new   | Pikachu                     | New Set 2023, no dex (recent sets have none) |
/// | pk-promo | Pikachu - SWSH039           | Promos 2024, rarity Promo, dex 25            |
/// | pk-full  | Pikachu V (Full Art)        | 2022, dex 25                                 |
/// | raichu   | Raichu                      | dex 26                                       |
/// | eevee-a  | Eevee                       | 2010, dex 133                                |
/// | eevee-b  | Eevee                       | no release date; its expansion says 2021     |
/// | nidoran  | Nidoran F                   | no card type, no dex                         |
/// | nidoran2 | Nidoran♀                    | 1999, dex 29                                 |
/// | tool     | Flying Pikachu              | Tool with dex 25: not a Pikachu              |
/// | switch   | Switch                      | Item, dex placeholder                        |
/// | tag      | Arceus & Dialga & Palkia GX | dex 483 as scraped                           |
/// | pk-ex    | Pikachu ex                  | 2025, no dex: newer, but not the plain Pikachu |
/// | pk-team  | Pikachu & Zekrom GX         | 2025, dex 25                                 |
///
/// so `prints` finds 14 cards and `species` 7: Pikachu, Raichu, Eevee, Nidoran, Dialga, plus
/// the tool and the trainer, which have no species.
fn species_fixture(dir: &TempDir) -> PokemonSQLiteRetrievalSystem {
    let path = dir.path().join("pokemon.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE cards (cardId TEXT UNIQUE, name TEXT, expName TEXT, rarity TEXT, energyType TEXT,
             cardType TEXT NULL, img TEXT, expCardNumber TEXT, pokedex INTEGER NULL, description TEXT NULL,
             releaseDate TEXT NULL, expCodeTCGP TEXT NULL, variants TEXT NULL, expIdTCGP TEXT NULL);
         CREATE TABLE expansions (name TEXT UNIQUE, releaseDate TEXT);
         CREATE TABLE pokedex (id INTEGER, name TEXT, img TEXT);
         INSERT INTO pokedex (id, name) VALUES (25, 'Pikachu'), (26, 'Raichu'), (29, 'Nidoran♀'),
             (133, 'Eevee'), (483, 'Dialga'), (493, 'Arceus');
         INSERT INTO expansions (name, releaseDate) VALUES ('Undated Set', '2021-05-01T00:00:00.000Z');",
    )
    .unwrap();
    // (id, name, set, rarity, card type, dex, release date)
    let cards: [(&str, &str, &str, &str, Option<&str>, Option<i64>, Option<&str>); 14] = [
        ("pk-old", "Pikachu", "Old Set", "Common", Some("Pokemon"), Some(25), Some("1999-01-09T00:00:00Z")),
        ("pk-new", "Pikachu", "New Set", "Common", Some("Pokemon"), None, Some("2023-03-31T00:00:00.000Z")),
        ("pk-promo", "Pikachu - SWSH039", "SWSH Promos", "Promo", Some("Pokemon"), Some(25), Some("2024-06-01T00:00:00Z")),
        ("pk-full", "Pikachu V (Full Art)", "Mid Set", "Ultra Rare", Some("Pokemon"), Some(25), Some("2022-01-01T00:00:00Z")),
        ("raichu", "Raichu", "Old Set", "Rare", Some("Pokemon"), Some(26), Some("2001-01-01T00:00:00Z")),
        ("eevee-a", "Eevee", "Old Set", "Common", Some("Pokemon"), Some(133), Some("2010-01-01T00:00:00Z")),
        ("eevee-b", "Eevee", "Undated Set", "Common", Some("Pokemon"), Some(133), None),
        ("nidoran", "Nidoran F", "New Set", "Common", None, None, Some("2015-01-01T00:00:00Z")),
        ("nidoran2", "Nidoran♀", "Old Set", "Common", Some("Pokemon"), Some(29), Some("1999-01-09T00:00:00Z")),
        ("tool", "Flying Pikachu", "Mid Set", "Common", Some("Tool"), Some(25), Some("2022-01-01T00:00:00Z")),
        ("switch", "Switch", "Mid Set", "Common", Some("Item"), Some(100000), Some("2022-01-01T00:00:00Z")),
        ("tag", "Arceus & Dialga & Palkia GX", "Mid Set", "Ultra Rare", Some("Pokemon"), Some(483), Some("2022-01-01T00:00:00Z")),
        ("pk-ex", "Pikachu ex", "New Set", "Double Rare", Some("Pokemon"), None, Some("2025-01-01T00:00:00Z")),
        ("pk-team", "Pikachu & Zekrom GX", "New Set", "Ultra Rare", Some("Pokemon"), Some(25), Some("2025-01-01T00:00:00Z")),
    ];
    for (id, name, set, rarity, card_type, dex, release) in cards {
        conn.execute(
            "INSERT INTO cards (cardId, name, expName, rarity, energyType, cardType, img, expCardNumber, pokedex, releaseDate)
             VALUES (?1, ?2, ?3, ?4, 'Colorless', ?5, '', '001', ?6, ?7)",
            (id, name, set, rarity, card_type, dex, release),
        )
        .unwrap();
    }
    drop(conn);
    PokemonSQLiteRetrievalSystem::new(Some(path.to_string_lossy().to_string()), None).unwrap()
}

async fn ids(system: &PokemonSQLiteRetrievalSystem, filters: CardSearchFilters, skip: usize, limit: usize) -> Vec<String> {
    system
        .search_cards(filters, Some(skip), Some(limit))
        .await
        .unwrap()
        .into_iter()
        .map(|c| match c {
            Card::Pokemon(p) => p.id,
            _ => unreachable!(),
        })
        .collect()
}

#[tokio::test]
async fn test_unique_modes_offered() {
    let modes = setup_test_db().await.unique_modes();
    let ids: Vec<_> = modes.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["prints", "species"]);
}

#[tokio::test]
async fn test_unique_defaults_to_prints() {
    let dir = TempDir::new().unwrap();
    let system = species_fixture(&dir);
    let all = |f: CardSearchFilters| ids(&system, f, 0, 100);

    let by_default = all(CardSearchFilters::new()).await;
    assert_eq!(by_default.len(), 14);
    assert_eq!(all(CardSearchFilters::new().with_unique("prints")).await, by_default);
    assert_eq!(all(CardSearchFilters::new().with_unique("")).await, by_default);
}

#[tokio::test]
async fn test_species_collapses_a_pokemon_to_its_best_printing() {
    let dir = TempDir::new().unwrap();
    let system = species_fixture(&dir);
    let species = ids(&system, CardSearchFilters::new().with_unique("species"), 0, 100).await;
    // Sorted by name. Pikachu: the newest printing of the plain Pikachu, even though the recent
    // one has no Pokédex number of its own, and not the newer promo, full-art variant or ex and
    // team-up cards. Eevee: the
    // undated printing counts as its expansion's 2021, beating 2010. Nidoran: the untyped
    // "Nidoran F" is found by name and is newer than the ♀ card. The tag team is filed under
    // its scraped number. The tool and trainer aren't Pokémon, so they stay as they are.
    assert_eq!(species, ["tag", "eevee-b", "tool", "nidoran", "pk-new", "raichu", "switch"]);
}

#[tokio::test]
async fn test_species_picks_the_representative_among_the_matching_printings() {
    let dir = TempDir::new().unwrap();
    let system = species_fixture(&dir);
    for (set, expected) in [("Old Set", "pk-old"), ("Mid Set", "pk-full"), ("SWSH Promos", "pk-promo")] {
        let filters = CardSearchFilters::new().with_name("Pikachu").with_set_code(set).with_unique("species");
        let found = ids(&system, filters, 0, 100).await;
        // The tool also matches "Pikachu", in the set it is in.
        let pikachus: Vec<_> = found.iter().filter(|id| id.starts_with("pk-")).collect();
        assert_eq!(pikachus, [expected], "set {set}: {found:?}");
    }
}

#[tokio::test]
async fn test_species_sorts_and_pages_the_collapsed_results() {
    let dir = TempDir::new().unwrap();
    let system = species_fixture(&dir);
    use ::models::filters::{SortField, SortOrder};
    for sort in [SortField::Name, SortField::SetCode, SortField::CollectorNumber] {
        for order in [SortOrder::Asc, SortOrder::Desc] {
            let filters = || CardSearchFilters::new().with_unique("species").with_sort_by(sort.clone()).with_sort_order(order.clone());
            let whole = ids(&system, filters(), 0, 100).await;
            assert_eq!(whole.len(), 7, "{sort:?} {order:?}");
            let mut paged = vec![];
            for page in 0..whole.len() {
                paged.extend(ids(&system, filters(), page * 2, 2).await);
            }
            assert_eq!(paged, whole, "{sort:?} {order:?}");
        }
    }
    let desc = ids(&system, CardSearchFilters::new().with_unique("species").with_sort_order(SortOrder::Desc), 0, 100).await;
    assert_eq!(desc, ["switch", "raichu", "pk-new", "nidoran", "tool", "eevee-b", "tag"]);
}

#[tokio::test]
async fn test_unique_rejects_unknown_mode() {
    let system = setup_test_db().await;
    let err = system
        .search_cards(CardSearchFilters::new().with_unique("art"), None, Some(1))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Unsupported unique mode 'art'"), "{err}");
    assert!(err.to_string().contains("prints, species"), "{err}");
}

#[tokio::test]
async fn test_species_without_a_pokedex_table_uses_scraped_numbers_only() {
    let dir = TempDir::new().unwrap();
    let system = species_fixture(&dir);
    Connection::open(&system._db_path).unwrap().execute_batch("DROP TABLE pokedex; DROP TABLE expansions;").unwrap();
    let species = ids(&system, CardSearchFilters::new().with_unique("species"), 0, 100).await;
    // Without names to match, the numberless cards are results of their own, while the ones
    // with a scraped number still collapse. With no names to say which is the plain Pikachu,
    // dex 25 goes to its newest regular printing.
    assert!(species.contains(&"pk-new".to_string()) && species.contains(&"nidoran".to_string()));
    assert!(species.contains(&"pk-team".to_string()));
    for gone in ["pk-old", "pk-promo", "pk-full"] {
        assert!(!species.contains(&gone.to_string()), "{gone} should have collapsed into pk-team");
    }
}

#[tokio::test]
async fn test_species_on_the_real_data() {
    let system = setup_test_db().await;
    let names = |cards: Vec<Card>| -> Vec<String> {
        cards.into_iter().map(|c| match c { Card::Pokemon(p) => p.name, _ => unreachable!() }).collect()
    };
    let prints = names(system.search_cards(CardSearchFilters::new().with_name("Bulbasaur"), None, Some(1000)).await.unwrap());
    let species = names(
        system
            .search_cards(CardSearchFilters::new().with_name("Bulbasaur").with_unique("species"), None, Some(1000))
            .await
            .unwrap(),
    );
    assert!(prints.len() > 10);
    // One Bulbasaur, plus the Ditto card that is named after it: it is a different species.
    assert_eq!(species.len(), 2, "{species:?}");
    assert_eq!(species.iter().filter(|n| n.starts_with("Bulbasaur")).count(), 1);
    assert!(species.iter().any(|n| n.starts_with("Ditto")));
}

#[tokio::test]
async fn test_cards_with_missing_fields_still_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pokemon.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE cards (cardId TEXT UNIQUE, name TEXT, expName TEXT, rarity TEXT, energyType TEXT,
             cardType TEXT NULL, img TEXT, expCardNumber TEXT, pokedex INTEGER NULL, description TEXT NULL,
             releaseDate TEXT NULL, expCodeTCGP TEXT NULL, variants TEXT NULL, expIdTCGP TEXT NULL);
         -- Only an id and a name: no set, rarity, energy type, card type, image or number.
         INSERT INTO cards (cardId, name) VALUES ('bare', 'Bare Card');",
    )
    .unwrap();
    drop(conn);
    let system = PokemonSQLiteRetrievalSystem::new(Some(path.to_string_lossy().to_string()), None).unwrap();

    let found = system
        .search_cards(CardSearchFilters::new().with_name("Bare"), None, Some(10))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    let by_id = system.get_cards_by_ids(vec!["bare".to_string()]).await.unwrap();
    assert!(by_id.contains_key("bare"));
}
