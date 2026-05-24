use super::*;
use crate::{CollectionCard, CollectionCardsParams, CollectionSortField, PersistenceSystemTrait};
use models::filters::SortOrder;
use models::{CardID, CollectionID};
use rusqlite::params;

const DEFAULT: &str = "Default";
const OLD_TIME: &str = "2023-01-01T00:00:00Z";

#[test]
fn migrations_test() {
    assert!(MIGRATIONS.validate().is_ok());
}

async fn get_time_updated(
    persistence: &SQLitePersistenceSystem,
    collection_id: &str,
    card_uuid: &str,
) -> Option<String> {
    let conn = persistence.connection.lock().await;
    conn.query_row(
        "SELECT timeupdated FROM cards WHERE collection = ?1 AND uuid = ?2",
        params![collection_id, card_uuid],
        |row| row.get(0),
    )
    .ok()
}

async fn add_card(
    p: &mut SQLitePersistenceSystem,
    collection_id: &CollectionID,
    card_id: &CardID,
    quantity: i32,
    foil_quantity: i32,
) -> CardID {
    p.add_card_to_collection(collection_id, card_id, quantity, foil_quantity, OLD_TIME, "")
        .await
        .unwrap();
    card_id.clone()
}

async fn record_purchase(
    p: &mut SQLitePersistenceSystem,
    col: &str,
    uuid: &str,
    qty: i32,
    foil: i32,
    price: Option<f64>,
) {
    let normal_price = if qty > 0 { price } else { None };
    let foil_price = if foil > 0 { price } else { None };
    p.record_purchase(
        &col.to_string(),
        &uuid.to_string(),
        qty,
        foil,
        normal_price,
        foil_price,
        "prov",
        OLD_TIME,
    )
    .await
    .unwrap();
}

// ── collection management ──────────────────────────────────────────────────

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

// ── remove collection variants ─────────────────────────────────────────────

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

// ── card operations ────────────────────────────────────────────────────────

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

// ── pagination ─────────────────────────────────────────────────────────────

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

// ── sorting ────────────────────────────────────────────────────────────────

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

// ── provider filtering ─────────────────────────────────────────────────────

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

// ── move cards ─────────────────────────────────────────────────────────────

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

// ── timeupdated ────────────────────────────────────────────────────────────

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
        &[CollectionCard { uuid: "card1".to_string(), quantity: 3, foil_quantity: 1, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
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
        &[CollectionCard { uuid: "card1".to_string(), quantity: 2, foil_quantity: 1, time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string() }],
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

// ── purchase history ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_purchase_totals_no_entries() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    assert!(p.get_collection_purchase_totals(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_purchase_totals_null_price_excluded() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, None).await;
    assert!(p.get_collection_purchase_totals(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_purchase_totals_single_entry() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;
    let s = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = s.get("card1").unwrap();
    assert_eq!(s.quantity, 2);
    assert_eq!(s.foil_quantity, 0);
    assert!((s.total_normal_paid - 10.0).abs() < 1e-9);
    assert_eq!(s.total_foil_paid, 0.0);
}

#[tokio::test]
async fn test_purchase_totals_multiple_entries_same_card() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(7.0)).await;
    let s = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = s.get("card1").unwrap();
    assert_eq!(s.quantity, 3);
    assert!((s.total_normal_paid - 17.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_purchase_totals_mixed_null_and_priced() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, None).await;
    let s = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = s.get("card1").unwrap();
    assert_eq!(s.quantity, 2);
    assert!((s.total_normal_paid - 10.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_purchase_totals_foil_and_normal_separate() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(4.0)).await;
    record_purchase(&mut p, &col, "card1", 0, 1, Some(12.0)).await;
    let s = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = s.get("card1").unwrap();
    assert_eq!(s.quantity, 2);
    assert_eq!(s.foil_quantity, 1);
    assert!((s.total_normal_paid - 8.0).abs() < 1e-9);
    assert!((s.total_foil_paid - 12.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_purchase_totals_partial_history_qty() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(8.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, None).await;
    let s = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = s.get("card1").unwrap();
    assert_eq!(s.quantity, 2);
    assert!((s.total_normal_paid - 16.0).abs() < 1e-9);
}
