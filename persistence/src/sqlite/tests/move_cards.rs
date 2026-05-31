use super::*;

#[tokio::test]
async fn test_move_cards_between_collections() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    let cid = add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;
    add_card(&mut p, &DEFAULT.into(), &"default_card".to_string(), 3, 1).await;

    p.move_cards_between_collections(
        &[CollectionCard { uuid: cid.clone(), quantity: 4, foil_quantity: 0, time_added: "".to_string(), collection: col.clone(), provider: "".to_string() }],
        DEFAULT.to_string(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&DEFAULT.to_string(), CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 2);
    let c = cards.iter().find(|c| c.uuid == cid).unwrap();
    assert_eq!(c.quantity, 4);

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    let c = cards.iter().find(|c| c.uuid == cid).unwrap();
    assert_eq!(c.quantity, 1);
    assert_eq!(c.foil_quantity, 2);
}

#[tokio::test]
async fn test_move_cards_between_collections_skips_zero_quantity() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 0, foil_quantity: 0, time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string() }],
        DEFAULT.to_string(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards[0].quantity, 5);
    assert_eq!(cards[0].foil_quantity, 2);
    let dc = p.get_cards_in_collection_paginated(&DEFAULT.to_string(), CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dc.len(), 0);
}

#[tokio::test]
async fn test_move_partial_preserves_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), 5, 2, OLD_TIME, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 3, foil_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src[0].quantity, 2);
    assert_eq!(src[0].foil_quantity, 2);

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst[0].quantity, 3);
    assert_eq!(dst[0].foil_quantity, 0);
    assert_eq!(dst[0].provider, "mtg");
}

#[tokio::test]
async fn test_move_all_copies_preserves_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), 4, 1, OLD_TIME, "riftbound").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 4, foil_quantity: 1, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src.len(), 0);

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst[0].quantity, 4);
    assert_eq!(dst[0].foil_quantity, 1);
    assert_eq!(dst[0].provider, "riftbound");
}

#[tokio::test]
async fn test_move_same_collection_is_noop() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("My Collection".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 5, 2, OLD_TIME, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), quantity: 5, foil_quantity: 2, time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string() }],
        col.clone(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].quantity, 5);
    assert_eq!(cards[0].foil_quantity, 2);
    assert_eq!(cards[0].provider, "mtg");
}
