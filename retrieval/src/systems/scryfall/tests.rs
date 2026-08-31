use super::parsing::parse_card;
use super::*;
use models::CardColour;

/// Real Scryfall API responses (captured verbatim), used to test parsing
/// without hitting the network.
const BIRGI_MODAL_DFC: &str = include_str!("testdata/birgi_dfc.json");
const LIGHTNING_BOLT: &str = include_str!("testdata/lightning_bolt.json");
const SEARCH_BIRGI: &str = include_str!("testdata/search_birgi.json");
const SPLIT_FIRE_ICE: &str = include_str!("testdata/split_fire_ice.json");
const ADVENTURE_BONECRUSHER: &str = include_str!("testdata/adventure_bonecrusher.json");
const TRANSFORM_DELVER: &str = include_str!("testdata/transform_delver.json");
const SETS_RESPONSE: &str = include_str!("testdata/sets_response.json");
const COLLECTION_RESPONSE: &str = include_str!("testdata/collection_response.json");

fn parse_fixture(json: &str) -> Card {
    let value: Value = serde_json::from_str(json).expect("fixture should be valid JSON");
    parse_card(value.as_object().expect("fixture should be a JSON object"))
        .expect("fixture should parse into a Card")
}

fn as_magic(card: Card) -> models::MagicCard {
    match card {
        Card::Magic(c) => c,
        other => panic!("expected a Magic card, got {other:?}"),
    }
}

#[test]
fn parses_normal_single_faced_card() {
    let card = as_magic(parse_fixture(LIGHTNING_BOLT));
    assert_eq!(card.name, "Lightning Bolt");
    assert_eq!(card.text, "Lightning Bolt deals 3 damage to any target.");
    assert_eq!(card.mana_cost, "{R}");
}

/// Regression test for the reported bug: Scryfall moves face-specific fields
/// (oracle_text, mana_cost, power, toughness, colors) off the top-level card
/// object and into `card_faces` for modal DFCs, so a card like Birgi must not
/// be dropped by `parse_card` for lacking a top-level `oracle_text`.
#[test]
fn parses_modal_double_faced_card() {
    let card = as_magic(parse_fixture(BIRGI_MODAL_DFC));

    assert_eq!(
        card.name,
        "Birgi, God of Storytelling // Harnfel, Horn of Bounty"
    );
    assert_eq!(
        card.type_line,
        "Legendary Creature — God // Legendary Artifact"
    );

    // Combines both faces' oracle text rather than being empty/missing.
    assert!(card.text.contains("Whenever you cast a spell, add"));
    assert!(
        card.text
            .contains("Exile the top two cards of your library")
    );

    // Falls back to the front face for fields Scryfall omits at the top
    // level for multi-faced cards.
    assert_eq!(card.mana_cost, "{2}{R}");
    assert_eq!(card.power, Some("3".to_string()));
    assert_eq!(card.toughness, Some("3".to_string()));
    assert_eq!(card.colors, vec![CardColour::Red]);
}

#[test]
fn search_results_include_double_faced_cards() {
    let value: Value = serde_json::from_str(SEARCH_BIRGI).expect("valid JSON");
    let cards_array = value
        .get("data")
        .and_then(Value::as_array)
        .expect("data array");

    let cards: Vec<Card> = cards_array
        .iter()
        .filter_map(|c| c.as_object().and_then(parse_card))
        .collect();

    assert_eq!(
        cards.len(),
        1,
        "the modal DFC in the search results should not be dropped"
    );
}

#[test]
fn parses_split_card() {
    let card = as_magic(parse_fixture(SPLIT_FIRE_ICE));
    assert_eq!(card.name, "Fire // Ice");
    // Split cards keep combined mana_cost/colors at the top level already.
    assert_eq!(card.mana_cost, "{1}{R} // {1}{U}");
    assert!(card.text.contains("Fire deals 2 damage"));
    assert!(card.text.contains("Tap target permanent"));
}

#[test]
fn parses_adventure_card() {
    let card = as_magic(parse_fixture(ADVENTURE_BONECRUSHER));
    assert_eq!(card.name, "Bonecrusher Giant // Stomp");
    assert_eq!(card.power, Some("4".to_string()));
    assert_eq!(card.toughness, Some("3".to_string()));
    assert!(card.text.contains("becomes the target of a spell"));
    assert!(card.text.contains("Stomp deals 2 damage to any target"));
}

#[test]
fn parses_transform_card_front_face_stats() {
    let card = as_magic(parse_fixture(TRANSFORM_DELVER));
    assert_eq!(card.name, "Delver of Secrets // Insectile Aberration");
    // Front face (Delver of Secrets) stats used as the fallback.
    assert_eq!(card.power, Some("1".to_string()));
    assert_eq!(card.toughness, Some("1".to_string()));
    assert_eq!(card.mana_cost, "{U}");
    assert_eq!(card.colors, vec![CardColour::Blue]);
}

#[test]
fn parses_sets_response() {
    let json: Value = serde_json::from_str(SETS_RESPONSE).expect("valid JSON");
    let sets = parsing::parse_sets_response(&json);

    assert_eq!(sets.len(), 4);
    assert!(sets.iter().any(|s| s.code == "khm" && s.name == "Kaldheim"));
    assert!(
        sets.iter()
            .any(|s| s.code == "lea" && s.name == "Limited Edition Alpha")
    );
}

#[test]
fn parses_collection_response_skips_not_found() {
    let json: Value = serde_json::from_str(COLLECTION_RESPONSE).expect("valid JSON");
    let resolved = parsing::parse_collection_response(&json);

    assert_eq!(resolved.len(), 2);
    assert!(resolved.contains(&(
        "khm".to_string(),
        "123".to_string(),
        "44657ab1-0a6a-4a5f-9688-86f239083821".to_string()
    )));
    assert!(resolved.contains(&(
        "lea".to_string(),
        "1".to_string(),
        "d5c83259-9b90-47c2-b48e-c7d78519e792".to_string()
    )));
    // "zzzz"/"999" was in `not_found` and must not appear.
    assert!(!resolved.iter().any(|(set, _, _)| set == "zzzz"));
}

#[tokio::test]
#[ignore]
async fn get_basic_card() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem {};
    let card = r
        .search_cards(
            CardSearchFilters {
                name: Some("Panharmonicon".to_string()),
                ..Default::default()
            },
            None,
            None,
        )
        .await?;

    assert_eq!(card.len(), 1);
    let card = card.first().expect("No card?");
    let card = if let Card::Magic(card) = card {
        card
    } else {
        panic!("Not a Magic card")
    };
    assert_eq!(card.name, "Panharmonicon");
    assert_eq!(card.color_identity, vec![]);

    Ok(())
}

#[tokio::test]
#[ignore]
async fn get_cards_by_ids() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem {};
    let test_ids = vec![
        "998d0cc8-ca2a-41c3-ab65-d05c26ab8278".to_string(),
        "9a6cd6f6-ae6e-4a77-95ca-64c6882357d5".to_string(),
        "70dd138f-391a-4956-bc2a-fe186429c71a".to_string(),
    ];

    let result = r.get_cards_by_ids(test_ids).await?;

    assert_eq!(result.len(), 3);
    assert!(result.contains_key("998d0cc8-ca2a-41c3-ab65-d05c26ab8278"));
    assert!(result.contains_key("9a6cd6f6-ae6e-4a77-95ca-64c6882357d5"));
    assert!(result.contains_key("70dd138f-391a-4956-bc2a-fe186429c71a"));

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_search_by_types() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let creatures = r
        .search_cards(
            CardSearchFilters {
                types: Some(vec!["Creature".to_string()]),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(!creatures.is_empty(), "Should find at least one creature");

    for card in creatures {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.types.is_empty(),
            "Card {} should have types",
            card.name
        );
    }

    let artifacts = r
        .search_cards(
            CardSearchFilters {
                types: Some(vec!["Artifact".to_string()]),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(!artifacts.is_empty(), "Should find at least one artifact");

    for card in artifacts {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.types.is_empty(),
            "Card {} should have types",
            card.name
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_search_by_subtypes() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let elves = r
        .search_cards(
            CardSearchFilters {
                subtypes: Some(vec!["Elf".to_string()]),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(!elves.is_empty(), "Should find at least one elf");

    for card in elves {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.subtypes.is_empty(),
            "Card {} should have subtypes",
            card.name
        );
    }

    let wizards = r
        .search_cards(
            CardSearchFilters {
                subtypes: Some(vec!["Wizard".to_string()]),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(!wizards.is_empty(), "Should find at least one wizard");

    for card in wizards {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.subtypes.is_empty(),
            "Card {} should have subtypes",
            card.name
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_search_by_supertypes() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let legendary = r
        .search_cards(
            CardSearchFilters {
                supertypes: Some("Legendary".to_string()),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(
        !legendary.is_empty(),
        "Should find at least one legendary card"
    );

    for card in legendary {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.supertypes.is_empty(),
            "Card {} should have supertypes",
            card.name
        );
    }

    let basic = r
        .search_cards(
            CardSearchFilters {
                supertypes: Some("Basic".to_string()),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(!basic.is_empty(), "Should find at least one basic land");

    for card in basic {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.supertypes.is_empty(),
            "Card {} should have supertypes",
            card.name
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_search_by_multiple_types() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let creatures_artifacts = r
        .search_cards(
            CardSearchFilters {
                types: Some(vec!["Creature".to_string(), "Artifact".to_string()]),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(
        !creatures_artifacts.is_empty(),
        "Should find at least one creature or artifact"
    );

    for card in creatures_artifacts {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.types.is_empty(),
            "Card {} should have types",
            card.name
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_search_by_combined_filters() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let legendary_creatures = r
        .search_cards(
            CardSearchFilters {
                types: Some(vec!["Creature".to_string()]),
                subtypes: Some(vec!["Elf".to_string()]),
                supertypes: Some("Legendary".to_string()),
                ..Default::default()
            },
            None,
            Some(5),
        )
        .await?;

    assert!(
        !legendary_creatures.is_empty(),
        "Should find at least one legendary elf creature"
    );

    for card in legendary_creatures {
        let card = if let Card::Magic(card) = card {
            card
        } else {
            panic!("Not a Magic card")
        };

        assert!(
            !card.types.is_empty(),
            "Card {} should have types",
            card.name
        );
        assert!(
            !card.subtypes.is_empty(),
            "Card {} should have subtypes",
            card.name
        );
        assert!(
            !card.supertypes.is_empty(),
            "Card {} should have supertypes",
            card.name
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_get_random_card() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let card = r.get_random_card().await?;
    assert!(
        card.is_some(),
        "Scryfall should always return a random card"
    );
    let card = if let Card::Magic(card) = card.unwrap() {
        card
    } else {
        panic!("Not a Magic card")
    };
    assert!(!card.name.is_empty());
    assert!(!card.id.is_empty());

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_get_random_card_varies() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let mut names = std::collections::HashSet::new();
    for _ in 0..5 {
        let card = r.get_random_card().await?.expect("expected a card");
        if let Card::Magic(card) = card {
            names.insert(card.name);
        }
    }
    assert!(
        names.len() > 1,
        "expected varying random cards, got {names:?}"
    );

    Ok(())
}

/// Live regression test for the reported bug: searching for a modal DFC by
/// name used to come back empty because `parse_card` required a top-level
/// `oracle_text` that Scryfall omits for multi-faced cards.
#[tokio::test]
#[ignore]
async fn search_finds_modal_double_faced_card() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let results = r
        .search_cards(
            CardSearchFilters {
                name: Some("Birgi, God of Storytelling".to_string()),
                ..Default::default()
            },
            None,
            None,
        )
        .await?;

    assert_eq!(results.len(), 1, "expected to find Birgi, got {results:?}");
    let Card::Magic(card) = &results[0] else {
        panic!("Not a Magic card")
    };
    assert!(card.name.starts_with("Birgi, God of Storytelling"));
    assert!(!card.text.is_empty(), "oracle text should not be empty");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_get_sets() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let sets = r.get_sets().await?;

    assert!(!sets.is_empty());
    assert!(sets.iter().any(|s| s.code == "khm"));

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_bulk_search_cards() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;

    let results = r
        .bulk_search_cards(vec![
            ("khm".to_string(), "123".to_string()),
            ("lea".to_string(), "1".to_string()),
            ("zzzz".to_string(), "999".to_string()),
        ])
        .await?;

    assert_eq!(results.len(), 2, "unmatched identifier should be dropped");
    assert!(
        results
            .iter()
            .any(|(set, number, _)| set == "khm" && number == "123")
    );
    assert!(
        results
            .iter()
            .any(|(set, number, _)| set == "lea" && number == "1")
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_bulk_search_cards_empty() -> eyre::Result<()> {
    let r = ScryfallRetrievalSystem::new()?;
    let results = r.bulk_search_cards(vec![]).await?;
    assert!(results.is_empty());
    Ok(())
}
