use super::*;

#[tokio::test]
async fn test_collection_sort_by_quantity_asc() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card_a".to_string(), 5, 0, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_b".to_string(), 1, 0, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_c".to_string(), 3, 0, OLD_TIME, "").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: Some(CollectionSortField::Quantity), sort_order: Some(SortOrder::Asc), provider: None, providers: vec![],
    }).await.unwrap();
    assert_eq!(cards[0].quantity, 1);
    assert_eq!(cards[1].quantity, 3);
    assert_eq!(cards[2].quantity, 5);
}

#[tokio::test]
async fn test_collection_sort_by_quantity_desc() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card_a".to_string(), 5, 0, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_b".to_string(), 1, 0, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_c".to_string(), 3, 0, OLD_TIME, "").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: Some(CollectionSortField::Quantity), sort_order: Some(SortOrder::Desc), provider: None, providers: vec![],
    }).await.unwrap();
    assert_eq!(cards[0].quantity, 5);
    assert_eq!(cards[1].quantity, 3);
    assert_eq!(cards[2].quantity, 1);
}

#[tokio::test]
async fn test_collection_sort_by_foil_quantity_desc() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card_a".to_string(), 1, 10, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_b".to_string(), 1, 2, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card_c".to_string(), 1, 7, OLD_TIME, "").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: Some(CollectionSortField::FoilQuantity), sort_order: Some(SortOrder::Desc), provider: None, providers: vec![],
    }).await.unwrap();
    assert_eq!(cards[0].foil_quantity, 10);
    assert_eq!(cards[1].foil_quantity, 7);
    assert_eq!(cards[2].foil_quantity, 2);
}

#[tokio::test]
async fn test_collection_sort_by_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"z_card".to_string(), 1, 0, OLD_TIME, "ZProvider").await.unwrap();
    p.add_card_to_collection(&col, &"a_card".to_string(), 1, 0, OLD_TIME, "AProvider").await.unwrap();
    p.add_card_to_collection(&col, &"m_card".to_string(), 1, 0, OLD_TIME, "MProvider").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: Some(CollectionSortField::Provider), sort_order: Some(SortOrder::Asc), provider: None, providers: vec![],
    }).await.unwrap();
    assert_eq!(cards[0].provider, "AProvider");
    assert_eq!(cards[1].provider, "MProvider");
    assert_eq!(cards[2].provider, "ZProvider");
}

// ── provider filtering ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_collection_filter_by_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"mtg2".to_string(), 2, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 1, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: None, sort_order: None,
        provider: Some("MagicSQLite".to_string()), providers: vec![],
    }).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().all(|c| c.provider == "MagicSQLite"));
}

#[tokio::test]
async fn test_collection_filter_by_providers_multi() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 1, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"pk1".to_string(), 1, 0, OLD_TIME, "PokemonSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: None, sort_order: None,
        provider: None,
        providers: vec!["MagicSQLite".to_string(), "RiftboundSQLite".to_string()],
    }).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().any(|c| c.uuid == "mtg1"));
    assert!(cards.iter().any(|c| c.uuid == "rb1"));
    assert!(!cards.iter().any(|c| c.uuid == "pk1"));
}

#[tokio::test]
async fn test_collection_filter_by_providers_single_entry() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 1, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: None, sort_order: None,
        provider: None,
        providers: vec!["RiftboundSQLite".to_string()],
    }).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].uuid, "rb1");
}

#[tokio::test]
async fn test_collection_filter_provider_takes_precedence_over_providers() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 1, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: None, sort_order: None,
        provider: Some("MagicSQLite".to_string()),
        providers: vec!["RiftboundSQLite".to_string()],
    }).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].uuid, "mtg1");
}

#[tokio::test]
async fn test_collection_filter_by_provider_no_match() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10, sort_by: None, sort_order: None,
        provider: Some("PokemonSQLite".to_string()), providers: vec![],
    }).await.unwrap();
    assert!(cards.is_empty());
}

#[tokio::test]
async fn test_collection_filter_and_sort_combined() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg_high".to_string(), 5, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"mtg_low".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 99, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams {
        offset: 0, limit: 10,
        sort_by: Some(CollectionSortField::Quantity), sort_order: Some(SortOrder::Asc),
        provider: Some("MagicSQLite".to_string()), providers: vec![],
    }).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].uuid, "mtg_low");
    assert_eq!(cards[1].uuid, "mtg_high");
}
