use super::*;

#[tokio::test]
async fn test_timeupdated_equals_timeadded_on_create() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 2, 1, OLD_TIME, "").await.unwrap();
    assert_eq!(get_time_updated(&p, &col, "card1").await.unwrap(), OLD_TIME);
}

#[tokio::test]
async fn test_timeupdated_changes_on_quantity_modification() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 2, 1, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 3, 0, OLD_TIME, "").await.unwrap();
    assert_ne!(get_time_updated(&p, &col, "card1").await.unwrap(), OLD_TIME);
}

#[tokio::test]
async fn test_timeupdated_changes_on_foil_quantity_modification() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 2, 1, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 0, -1, OLD_TIME, "").await.unwrap();
    assert_ne!(get_time_updated(&p, &col, "card1").await.unwrap(), OLD_TIME);
}

#[tokio::test]
async fn test_timeupdated_updated_on_move_source() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), 5, 2, OLD_TIME, "").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 3, foil_quantity: 1, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    assert_ne!(get_time_updated(&p, &col_a, "card1").await.unwrap(), OLD_TIME);
}

#[tokio::test]
async fn test_timeupdated_updated_on_move_destination_existing_card() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), 5, 2, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col_b, &"card1".to_string(), 1, 0, OLD_TIME, "").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 2, foil_quantity: 1, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    assert_ne!(get_time_updated(&p, &col_b, "card1").await.unwrap(), OLD_TIME);
}

#[tokio::test]
async fn test_timeupdated_updated_on_remove_collection_merge() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), 3, 1, OLD_TIME, "").await.unwrap();
    p.add_card_to_collection(&col_b, &"card1".to_string(), 2, 0, OLD_TIME, "").await.unwrap();
    p.remove_collection(&col_a, Some(col_b.clone())).await.unwrap();
    assert_ne!(get_time_updated(&p, &col_b, "card1").await.unwrap(), OLD_TIME);
}
