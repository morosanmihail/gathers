use super::*;

#[tokio::test]
async fn test_add_card_to_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    let cid = add_card(&mut p, &col, &"12345".to_string(), 2, 1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].uuid, cid);
    assert_eq!(cards[0].quantity, 2);
    assert_eq!(cards[0].foil_quantity, 1);

    add_card(&mut p, &col, &cid, 3, 2).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards[0].quantity, 5);
    assert_eq!(cards[0].foil_quantity, 3);

    add_card(&mut p, &col, &cid, -3, -1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards[0].quantity, 2);
    assert_eq!(cards[0].foil_quantity, 2);

    add_card(&mut p, &col, &cid, -2, -2).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_add_cards_to_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    let t = OLD_TIME.to_string();

    p.add_cards_to_collection(
        &col,
        &[
            CollectionCard { uuid: "12345".to_string(), quantity: 2, foil_quantity: 1, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12346".to_string(), quantity: 5, foil_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
        ],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards.iter().find(|c| c.uuid == "12345").unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.uuid == "12346").unwrap().quantity, 5);

    p.add_cards_to_collection(
        &col,
        &[CollectionCard { uuid: "12345".to_string(), quantity: 3, foil_quantity: 2, time_added: t.clone(), provider: "".to_string(), collection: col.clone() }],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    let c = cards.iter().find(|c| c.uuid == "12345").unwrap();
    assert_eq!(c.quantity, 5);
    assert_eq!(c.foil_quantity, 3);

    p.add_cards_to_collection(
        &col,
        &[
            CollectionCard { uuid: "12345".to_string(), quantity: -3, foil_quantity: -1, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12346".to_string(), quantity: 5, foil_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
        ],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.uuid == "12345").unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.uuid == "12346").unwrap().quantity, 10);
}

#[tokio::test]
async fn test_add_cards_to_collection_empty_slice() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    let result = p.add_cards_to_collection(&col, &[]).await.unwrap();
    assert!(result.is_empty());
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_quantity_floor_cannot_go_negative() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    add_card(&mut p, &col, &"card1".to_string(), 3, 2).await;
    add_card(&mut p, &col, &"card1".to_string(), -100, -1).await;

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].quantity, 0);
    assert_eq!(cards[0].foil_quantity, 1);

    add_card(&mut p, &col, &"card1".to_string(), 0, -1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

// ── pagination ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_cards_in_collection_paginated() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    for i in 0..10 {
        add_card(&mut p, &col, &(1000 + i).to_string(), 1, 0).await;
    }

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 5)).await.unwrap();
    assert_eq!(cards.len(), 5);
    assert_eq!(cards[0].uuid, "1000");

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(5, 5)).await.unwrap();
    assert_eq!(cards.len(), 5);
    assert_eq!(cards[0].uuid, "1005");

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(8, 5)).await.unwrap();
    assert_eq!(cards.len(), 2);

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(20, 5)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

// ── card count with provider filter ──────────────────────────────────────────

#[tokio::test]
async fn test_get_cards_count_with_providers_filter() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), 2, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"mtg2".to_string(), 1, 0, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), 1, 0, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let count = p.get_cards_in_collection_count(col.clone(), &["MagicSQLite".to_string()]).await.unwrap();
    assert_eq!(count, 2);

    let count = p.get_cards_in_collection_count(col.clone(), &["RiftboundSQLite".to_string()]).await.unwrap();
    assert_eq!(count, 1);

    let count = p.get_cards_in_collection_count(col.clone(), &["MagicSQLite".to_string(), "RiftboundSQLite".to_string()]).await.unwrap();
    assert_eq!(count, 3);

    let count = p.get_cards_in_collection_count(col.clone(), &["Unknown".to_string()]).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_get_cards_count_no_provider_filter() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), 1, 0, OLD_TIME, "A").await.unwrap();
    p.add_card_to_collection(&col, &"card2".to_string(), 1, 0, OLD_TIME, "B").await.unwrap();

    let count = p.get_cards_in_collection_count(col.clone(), &[]).await.unwrap();
    assert_eq!(count, 2);
}
