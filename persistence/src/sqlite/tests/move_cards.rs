use super::*;

#[tokio::test]
async fn test_move_cards_between_collections() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    let cid = add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;
    add_card(&mut p, &DEFAULT.into(), &"default_card".to_string(), 3, 1).await;

    // Move only the default (non-foil) finish's copies.
    p.move_cards_between_collections(
        &[CollectionCard { uuid: cid.clone(), finish: String::new(), quantity: 4, want_quantity: 0, time_added: "".to_string(), collection: col.clone(), provider: "".to_string() }],
        DEFAULT.to_string(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&DEFAULT.to_string(), CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 3); // default_card's 2 finishes + card1's moved-in "" row
    let c = cards.iter().find(|c| c.uuid == cid && c.finish.is_empty()).unwrap();
    assert_eq!(c.quantity, 4);

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    let c = cards.iter().find(|c| c.uuid == cid && c.finish.is_empty()).unwrap();
    assert_eq!(c.quantity, 1);
    let c_foil = cards.iter().find(|c| c.uuid == cid && c.finish == "foil").unwrap();
    assert_eq!(c_foil.quantity, 2); // untouched — only the "" finish was moved
}

#[tokio::test]
async fn test_move_cards_between_collections_skips_zero_quantity() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 2).await;

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 0, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string() }],
        DEFAULT.to_string(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 5);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 2);
    let dc = p.get_cards_in_collection_paginated(&DEFAULT.to_string(), CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dc.len(), 0);
}

#[tokio::test]
async fn test_move_partial_preserves_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), "", 5, OLD_TIME, "mtg").await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), "foil", 2, OLD_TIME, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 3, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 2);
    assert_eq!(src.iter().find(|c| c.finish == "foil").unwrap().quantity, 2);

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst.len(), 1); // only the "" finish was moved
    assert_eq!(dst[0].quantity, 3);
    assert!(dst[0].finish.is_empty());
    assert_eq!(dst[0].provider, "mtg");
}

#[tokio::test]
async fn test_move_all_copies_preserves_provider() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), "", 4, OLD_TIME, "riftbound").await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), "foil", 1, OLD_TIME, "riftbound").await.unwrap();

    p.move_cards_between_collections(
        &[
            CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 4, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() },
            CollectionCard { uuid: "card1".to_string(), finish: "foil".to_string(), quantity: 1, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() },
        ],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src.len(), 0);

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst.len(), 2);
    assert_eq!(dst.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 4);
    let foil = dst.iter().find(|c| c.finish == "foil").unwrap();
    assert_eq!(foil.quantity, 1);
    assert!(dst.iter().all(|c| c.provider == "riftbound"));
}

#[tokio::test]
async fn test_move_wanted_only_card() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.adjust_want_quantity(&col_a, &"card1".to_string(), 4, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 0, want_quantity: 4, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src.len(), 0, "wanted-only row purged from source once moved out");

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst[0].want_quantity, 4);
    assert_eq!(dst[0].quantity, 0);
}

#[tokio::test]
async fn test_move_card_with_owned_and_wanted_quantities() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    p.add_card_to_collection(&col_a, &"card1".to_string(), "", 3, OLD_TIME, "mtg").await.unwrap();
    p.adjust_want_quantity(&col_a, &"card1".to_string(), 2, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 3, want_quantity: 2, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
        col_b.clone(),
    ).await.unwrap();

    let src = p.get_cards_in_collection_paginated(&col_a, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(src.len(), 0, "source row fully purged: owned and wanted both moved out");

    let dst = p.get_cards_in_collection_paginated(&col_b, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(dst[0].quantity, 3);
    assert_eq!(dst[0].want_quantity, 2);
}

#[tokio::test]
async fn test_move_same_collection_is_noop() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("My Collection".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), "", 5, OLD_TIME, "mtg").await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), "foil", 2, OLD_TIME, "mtg").await.unwrap();

    p.move_cards_between_collections(
        &[
            CollectionCard { uuid: "card1".to_string(), finish: String::new(), quantity: 5, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string() },
            CollectionCard { uuid: "card1".to_string(), finish: "foil".to_string(), quantity: 2, want_quantity: 0, time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string() },
        ],
        col.clone(),
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 5);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 2);
    assert!(cards.iter().all(|c| c.provider == "mtg"));
}
