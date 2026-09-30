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
    let err = p.remove_collection(&DEFAULT.into(), None).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<PersistenceError>(),
        Some(&PersistenceError::CollectionNotRemovable(DEFAULT.into()))
    );
    assert_eq!(p.list_collections(None).await.unwrap().len(), 2); // Default not removed
    let cards = p
        .get_cards_in_collection_paginated(&DEFAULT.into(), CollectionCardsParams::new(0, 5))
        .await
        .unwrap();
    assert_eq!(cards.len(), 2, "a refused removal leaves the default collection's cards alone");

    p.remove_collection(&col, None).await.unwrap();
    assert_eq!(p.list_collections(None).await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_list_collection_info_marks_default_unremovable() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    p.add_collection("Binder".to_string()).await.unwrap();
    let mut info = p.list_collection_info().await.unwrap();
    info.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        info,
        vec![
            CollectionInfo { name: "Binder".into(), removable: true },
            CollectionInfo { name: DEFAULT.into(), removable: false },
        ]
    );
}

#[tokio::test]
async fn test_remove_missing_collection_is_not_found() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let err = p.remove_collection(&"Nope".to_string(), None).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<PersistenceError>(),
        Some(&PersistenceError::CollectionNotFound("Nope".into()))
    );
}

#[tokio::test]
async fn test_remove_collection_move_to_missing_target_changes_nothing() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Binder".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 2, 0).await;

    let err = p.remove_collection(&col, Some("Ghost".to_string())).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<PersistenceError>(),
        Some(&PersistenceError::CollectionNotFound("Ghost".into()))
    );
    assert!(p.list_collections(None).await.unwrap().contains(&col));
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 5)).await.unwrap();
    assert_eq!(cards.len(), 1);
}

#[tokio::test]
async fn test_remove_collection_revokes_share_links_and_history() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Binder".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 2, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(1.0)).await;
    let link = p.create_share_link(&col).await.unwrap();

    p.remove_collection(&col, None).await.unwrap();
    assert_eq!(p.resolve_share_link(&link.token).await.unwrap(), None);

    // A new collection reusing the name starts clean: no old link reaches
    // it, and no stale purchase history is attached to it.
    let col = p.add_collection("Binder".to_string()).await.unwrap();
    assert_eq!(p.resolve_share_link(&link.token).await.unwrap(), None);
    assert!(p.get_all_purchase_history(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_remove_collection_move_to_carries_purchase_history() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let a = p.add_collection("A".to_string()).await.unwrap();
    let b = p.add_collection("B".to_string()).await.unwrap();
    add_card(&mut p, &a, &"card1".to_string(), 2, 0).await;
    record_purchase(&mut p, &a, "card1", 2, 0, Some(3.0)).await;

    p.remove_collection(&a, Some(b.clone())).await.unwrap();
    let history = p.get_all_purchase_history(&b).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].quantity, 2);
}

#[tokio::test]
async fn test_rename_collection_keeps_share_links() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Old".to_string()).await.unwrap();
    let link = p.create_share_link(&col).await.unwrap();

    p.rename_collection(&col, &"New".to_string()).await.unwrap();
    assert_eq!(p.resolve_share_link(&link.token).await.unwrap(), Some("New".to_string()));
    assert_eq!(p.list_share_links(&"New".to_string()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_rename_collection_errors() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let a = p.add_collection("A".to_string()).await.unwrap();
    add_card(&mut p, &a, &"card1".to_string(), 1, 0).await;

    let err = p.rename_collection(&"Missing".to_string(), &"X".to_string()).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<PersistenceError>(),
        Some(&PersistenceError::CollectionNotFound("Missing".into()))
    );
    let err = p.rename_collection(&a, &DEFAULT.to_string()).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<PersistenceError>(),
        Some(&PersistenceError::CollectionExists(DEFAULT.into()))
    );
    // Nothing moved on failure.
    let cards = p.get_cards_in_collection_paginated(&a, CollectionCardsParams::new(0, 5)).await.unwrap();
    assert_eq!(cards.len(), 1);
    // Renaming to the same name is a no-op.
    p.rename_collection(&a, &a).await.unwrap();
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
    assert_eq!(cards2.len(), 4); // 2 finishes each for card1 and card2
    assert_eq!(cards2.iter().find(|c| c.uuid == cid1 && c.finish.is_empty()).unwrap().quantity, 5);
    assert_eq!(cards2.iter().find(|c| c.uuid == cid2 && c.finish.is_empty()).unwrap().quantity, 3);
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
    assert_eq!(cards.len(), 4); // 2 finishes each for card1 and default_card
    let dc = cards.iter().find(|c| c.uuid == cid && c.finish.is_empty()).unwrap();
    assert_eq!(dc.quantity, 3);
    let dc_foil = cards.iter().find(|c| c.uuid == cid && c.finish == "foil").unwrap();
    assert_eq!(dc_foil.quantity, 1);

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
    assert_eq!(cards.len(), 3); // shared_card: 2 finishes merged; unique_card: 1 finish
    let shared = cards.iter().find(|c| c.uuid == "shared_card" && c.finish.is_empty()).unwrap();
    assert_eq!(shared.quantity, 5);
    let shared_foil = cards.iter().find(|c| c.uuid == "shared_card" && c.finish == "foil").unwrap();
    assert_eq!(shared_foil.quantity, 5);
    let unique = cards.iter().find(|c| c.uuid == "unique_card").unwrap();
    assert_eq!(unique.quantity, 5);
    assert!(unique.finish.is_empty());
}

// get_cards_in_collection_count is the pagination total for get_cards_in_collection_paginated, so
// the two must count the same thing: entries (one row per finish), not distinct cards.
#[tokio::test]
async fn test_cards_count_matches_paginated_entry_count() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Binder".to_string()).await.unwrap();

    // One card owned in two finishes, plus a second card in one finish: 2 distinct cards, 3
    // entries
    add_card(&mut p, &col, &"bolt".to_string(), 3, 1).await;
    add_card(&mut p, &col, &"path".to_string(), 2, 0).await;

    let count = p
        .get_cards_in_collection_count(col.clone(), &[], None)
        .await
        .unwrap();
    let rows = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();

    assert_eq!(count, rows.len(), "count must match what pagination returns");
    assert_eq!(count, 3, "three entries across two cards");
}
