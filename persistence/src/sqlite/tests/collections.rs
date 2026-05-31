use super::*;

#[tokio::test]
async fn test_collection_management() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();

    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    assert!(!col.is_empty());

    let cols = p.list_collections(None).await.unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.contains(&"Test Collection".to_string()));
    assert!(cols.contains(&DEFAULT.into()));

    let col2 = p.add_collection("Another Collection".to_string()).await.unwrap();
    assert!(!col2.is_empty());

    let cols = p.list_collections(None).await.unwrap();
    assert_eq!(cols.len(), 3);

    p.remove_collection(&"Test Collection".to_string(), None).await.unwrap();

    let cols = p.list_collections(None).await.unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.contains(&DEFAULT.into()));
    assert!(cols.contains(&"Another Collection".to_string()));
}

#[tokio::test]
async fn test_add_collection_duplicate_name_is_idempotent() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    p.add_collection("My Collection".to_string()).await.unwrap();
    let result = p.add_collection("My Collection".to_string()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "My Collection");
    let result = p.add_collection(DEFAULT.to_string()).await;
    assert!(result.is_ok());
    let cols = p.list_collections(None).await.unwrap();
    assert_eq!(cols.len(), 2);
}

#[tokio::test]
async fn test_list_collections_with_filter() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    p.add_collection("Test Alpha".to_string()).await.unwrap();
    p.add_collection("Test Beta".to_string()).await.unwrap();
    p.add_collection("Gamma".to_string()).await.unwrap();

    let cols = p.list_collections(Some("Test".to_string())).await.unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.contains(&"Test Alpha".to_string()));
    assert!(cols.contains(&"Test Beta".to_string()));

    let cols = p.list_collections(Some("Alpha".to_string())).await.unwrap();
    assert_eq!(cols.len(), 1);

    let cols = p.list_collections(Some("XYZ_NOMATCH".to_string())).await.unwrap();
    assert!(cols.is_empty());

    let cols = p.list_collections(None).await.unwrap();
    assert_eq!(cols.len(), 4);
}

#[tokio::test]
async fn test_remove_collection_can_be_removed() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;
    p.remove_collection(&col, None).await.unwrap();
    assert!(!p.list_collections(None).await.unwrap().contains(&col));
}

#[tokio::test]
async fn test_remove_collection_with_none_move_to() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;
    p.remove_collection(&col, None).await.unwrap();
    assert!(!p.list_collections(None).await.unwrap().contains(&col));
    let cards = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_remove_collection_that_cant_be_removed() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"12345".to_string(), 5, 3).await;
    add_card(&mut p, &DEFAULT.into(), &"12346".to_string(), 2, 8).await;

    assert_eq!(p.list_collections(None).await.unwrap().len(), 2);
    p.remove_collection(&DEFAULT.into(), None).await.unwrap();
    assert_eq!(p.list_collections(None).await.unwrap().len(), 2); // Default not removed
    p.remove_collection(&col, None).await.unwrap();
    assert_eq!(p.list_collections(None).await.unwrap().len(), 1);
    let cards = p
        .get_cards_in_collection_paginated(&DEFAULT.into(), CollectionCardsParams::new(0, 5))
        .await
        .unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_remove_collection_with_move_to() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col1 = p.add_collection("Collection 1".to_string()).await.unwrap();
    let col2 = p.add_collection("Collection 2".to_string()).await.unwrap();

    let cid1 = add_card(&mut p, &col1, &"card1".to_string(), 5, 2).await;
    let cid2 = add_card(&mut p, &col1, &"card2".to_string(), 3, 1).await;

    let result = p.remove_collection(&col1, Some(col2.clone())).await.unwrap();
    assert_eq!(result, col1);
    assert!(!p.list_collections(None).await.unwrap().contains(&col1));

    let cards2 = p
        .get_cards_in_collection_paginated(&col2, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(cards2.len(), 2);
    assert_eq!(cards2.iter().find(|c| c.uuid == cid1).unwrap().quantity, 5);
    assert_eq!(cards2.iter().find(|c| c.uuid == cid2).unwrap().quantity, 3);
}

#[tokio::test]
async fn test_remove_default_collection_with_move_to() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;
    let cid = add_card(&mut p, &DEFAULT.into(), &"default_card".to_string(), 3, 1).await;

    p.remove_collection(&DEFAULT.into(), Some(col.clone())).await.unwrap();

    assert!(p.list_collections(None).await.unwrap().contains(&DEFAULT.into()));
    let cards = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(cards.len(), 2);
    let dc = cards.iter().find(|c| c.uuid == cid).unwrap();
    assert_eq!(dc.quantity, 3);
    assert_eq!(dc.foil_quantity, 1);

    let cards = p
        .get_cards_in_collection_paginated(&DEFAULT.into(), CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_remove_collection_move_to_merges_quantities() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col1 = p.add_collection("Collection 1".to_string()).await.unwrap();
    let col2 = p.add_collection("Collection 2".to_string()).await.unwrap();

    add_card(&mut p, &col1, &"shared_card".to_string(), 3, 1).await;
    add_card(&mut p, &col2, &"shared_card".to_string(), 2, 4).await;
    add_card(&mut p, &col1, &"unique_card".to_string(), 5, 0).await;

    p.remove_collection(&col1, Some(col2.clone())).await.unwrap();
    assert!(!p.list_collections(None).await.unwrap().contains(&col1));

    let cards = p
        .get_cards_in_collection_paginated(&col2, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(cards.len(), 2);
    let shared = cards.iter().find(|c| c.uuid == "shared_card").unwrap();
    assert_eq!(shared.quantity, 5);
    assert_eq!(shared.foil_quantity, 5);
    let unique = cards.iter().find(|c| c.uuid == "unique_card").unwrap();
    assert_eq!(unique.quantity, 5);
    assert_eq!(unique.foil_quantity, 0);
}
