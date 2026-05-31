use super::*;

#[tokio::test]
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
