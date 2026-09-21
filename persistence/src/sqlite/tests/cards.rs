use super::*;

#[tokio::test]
async fn test_add_card_to_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    let cid = add_card(&mut p, &col, &"12345".to_string(), 2, 1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 2); // one row per finish
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 1);
    assert!(cards.iter().all(|c| c.uuid == cid));

    add_card(&mut p, &col, &cid, 3, 2).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 5);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 3);

    add_card(&mut p, &col, &cid, -3, -1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 2);

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
            CollectionCard { uuid: "12345".to_string(), finish: String::new(), quantity: 2, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12345".to_string(), finish: "foil".to_string(), quantity: 1, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12346".to_string(), finish: String::new(), quantity: 5, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
        ],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.len(), 3);
    assert_eq!(cards.iter().find(|c| c.uuid == "12345" && c.finish.is_empty()).unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.uuid == "12345" && c.finish == "foil").unwrap().quantity, 1);
    assert_eq!(cards.iter().find(|c| c.uuid == "12346").unwrap().quantity, 5);

    p.add_cards_to_collection(
        &col,
        &[
            CollectionCard { uuid: "12345".to_string(), finish: String::new(), quantity: 3, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12345".to_string(), finish: "foil".to_string(), quantity: 2, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
        ],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.uuid == "12345" && c.finish.is_empty()).unwrap().quantity, 5);
    assert_eq!(cards.iter().find(|c| c.uuid == "12345" && c.finish == "foil").unwrap().quantity, 3);

    p.add_cards_to_collection(
        &col,
        &[
            CollectionCard { uuid: "12345".to_string(), finish: String::new(), quantity: -3, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12345".to_string(), finish: "foil".to_string(), quantity: -1, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
            CollectionCard { uuid: "12346".to_string(), finish: String::new(), quantity: 5, want_quantity: 0, time_added: t.clone(), provider: "".to_string(), collection: col.clone() },
        ],
    ).await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.uuid == "12345" && c.finish.is_empty()).unwrap().quantity, 2);
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
    assert_eq!(cards.len(), 1); // "" row purged (hit 0), only "foil" row remains
    assert_eq!(cards[0].finish, "foil");
    assert_eq!(cards[0].quantity, 1);

    add_card(&mut p, &col, &"card1".to_string(), 0, -1).await;
    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

// Regression test: removing (or un-wanting) a (uuid, finish) that has no
// existing row must be a no-op, not create a negative-quantity "ghost" row.
// The ON CONFLICT clamp (`max(cards.quantity + EXCLUDED.quantity, 0)`) only
// ever fires when a row already exists — a fresh INSERT with no conflict
// bypasses it entirely, so the initial value needs its own floor.
#[tokio::test]
async fn test_remove_never_added_card_creates_no_ghost_row() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    p.add_card_to_collection(&col, &"never-added".to_string(), "foil", -3, OLD_TIME, "")
        .await
        .unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert!(cards.is_empty(), "removing a never-added (uuid, finish) must create no row at all");
}

#[tokio::test]
async fn test_want_negative_on_never_wanted_card_creates_no_ghost_row() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    let card = p.adjust_want_quantity(&col, &"never-wanted".to_string(), -7, "").await.unwrap();
    assert_eq!(card.want_quantity, 0, "want floors at 0 even starting from nothing");

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert!(cards.is_empty(), "un-wanting a never-wanted card must create no row at all");
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
    p.add_card_to_collection(&col, &"mtg1".to_string(), "", 2, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"mtg2".to_string(), "", 1, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"rb1".to_string(), "", 1, OLD_TIME, "RiftboundSQLite").await.unwrap();

    let count = p.get_cards_in_collection_count(col.clone(), &["MagicSQLite".to_string()], None).await.unwrap();
    assert_eq!(count, 2);

    let count = p.get_cards_in_collection_count(col.clone(), &["RiftboundSQLite".to_string()], None).await.unwrap();
    assert_eq!(count, 1);

    let count = p.get_cards_in_collection_count(col.clone(), &["MagicSQLite".to_string(), "RiftboundSQLite".to_string()], None).await.unwrap();
    assert_eq!(count, 3);

    let count = p.get_cards_in_collection_count(col.clone(), &["Unknown".to_string()], None).await.unwrap();
    assert_eq!(count, 0);
}

// ── want quantity ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_adjust_want_quantity_creates_wishlist_only_row() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    let card = p.adjust_want_quantity(&col, &"card1".to_string(), 3, "mtg").await.unwrap();
    assert_eq!(card.quantity, 0);
    assert!(card.finish.is_empty());
    assert_eq!(card.want_quantity, 3);
    assert_eq!(card.provider, "mtg");

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].want_quantity, 3);
}

#[tokio::test]
async fn test_adjust_want_quantity_does_not_touch_owned_quantity() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 2, 1).await;

    p.adjust_want_quantity(&col, &"card1".to_string(), 4, "mtg").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().quantity, 2);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().quantity, 1);
    // want_quantity is only ever tracked on the "" (default) finish row.
    assert_eq!(cards.iter().find(|c| c.finish.is_empty()).unwrap().want_quantity, 4);
    assert_eq!(cards.iter().find(|c| c.finish == "foil").unwrap().want_quantity, 0);
}

#[tokio::test]
async fn test_adjust_want_quantity_accumulates() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    p.adjust_want_quantity(&col, &"card1".to_string(), 1, "mtg").await.unwrap();
    p.adjust_want_quantity(&col, &"card1".to_string(), 1, "mtg").await.unwrap();
    let card = p.adjust_want_quantity(&col, &"card1".to_string(), 1, "mtg").await.unwrap();
    assert_eq!(card.want_quantity, 3);

    let card = p.adjust_want_quantity(&col, &"card1".to_string(), -2, "mtg").await.unwrap();
    assert_eq!(card.want_quantity, 1);
}

#[tokio::test]
async fn test_adjust_want_quantity_clamps_negative_to_zero() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    p.adjust_want_quantity(&col, &"card1".to_string(), 2, "mtg").await.unwrap();
    let card = p.adjust_want_quantity(&col, &"card1".to_string(), -100, "mtg").await.unwrap();
    assert_eq!(card.want_quantity, 0);
}

#[tokio::test]
async fn test_adjust_want_quantity_to_zero_purges_wishlist_only_row() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();

    p.adjust_want_quantity(&col, &"card1".to_string(), 3, "mtg").await.unwrap();
    p.adjust_want_quantity(&col, &"card1".to_string(), -3, "mtg").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 0);
}

#[tokio::test]
async fn test_owned_card_survives_want_quantity_cleared_to_zero() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Test Collection".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 1, 0).await;

    p.adjust_want_quantity(&col, &"card1".to_string(), 3, "mtg").await.unwrap();
    p.adjust_want_quantity(&col, &"card1".to_string(), -3, "mtg").await.unwrap();

    let cards = p.get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 10)).await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].quantity, 1);
    assert_eq!(cards[0].want_quantity, 0);
}

#[tokio::test]
async fn test_get_cards_count_no_provider_filter() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"card1".to_string(), "", 1, OLD_TIME, "A").await.unwrap();
    p.add_card_to_collection(&col, &"card2".to_string(), "", 1, OLD_TIME, "B").await.unwrap();

    let count = p.get_cards_in_collection_count(col.clone(), &[], None).await.unwrap();
    assert_eq!(count, 2);
}

// ── hiding rows from disabled plugins ────────────────────────────────────────

async fn plugin_scope_fixture() -> (SQLitePersistenceSystem, String) {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    p.add_card_to_collection(&col, &"mtg1".to_string(), "", 1, OLD_TIME, "MagicSQLite").await.unwrap();
    p.add_card_to_collection(&col, &"book1".to_string(), "", 1, OLD_TIME, "plugin-books").await.unwrap();
    p.add_card_to_collection(&col, &"comic1".to_string(), "", 1, OLD_TIME, "plugin-comics").await.unwrap();
    // A row stored before providers were lowercased.
    p.add_card_to_collection(&col, &"book2".to_string(), "", 1, OLD_TIME, "plugin-Books").await.unwrap();
    (p, col)
}

async fn uuids_with_scope(p: &SQLitePersistenceSystem, col: &String, scope: Option<Vec<String>>) -> Vec<String> {
    let mut params = CollectionCardsParams::new(0, 100);
    params.enabled_plugin_providers = scope;
    let mut uuids: Vec<String> = p.get_cards_in_collection_paginated(col, params).await.unwrap()
        .into_iter().map(|c| c.uuid).collect();
    uuids.sort();
    uuids
}

#[tokio::test]
async fn test_plugin_scope_none_applies_no_restriction() {
    let (p, col) = plugin_scope_fixture().await;
    assert_eq!(uuids_with_scope(&p, &col, None).await.len(), 4);
    assert_eq!(p.get_cards_in_collection_count(col, &[], None).await.unwrap(), 4);
}

#[tokio::test]
async fn test_plugin_scope_empty_hides_every_plugin_row() {
    let (p, col) = plugin_scope_fixture().await;
    assert_eq!(uuids_with_scope(&p, &col, Some(vec![])).await, vec!["mtg1"]);
    assert_eq!(p.get_cards_in_collection_count(col, &[], Some(&[])).await.unwrap(), 1);
}

#[tokio::test]
async fn test_plugin_scope_keeps_only_enabled_plugins() {
    let (p, col) = plugin_scope_fixture().await;
    let enabled = vec!["plugin-books".to_string()];
    // Matches case-insensitively, so the legacy "plugin-Books" row stays too.
    assert_eq!(uuids_with_scope(&p, &col, Some(enabled.clone())).await, vec!["book1", "book2", "mtg1"]);
    assert_eq!(p.get_cards_in_collection_count(col, &[], Some(&enabled)).await.unwrap(), 3);
}

#[tokio::test]
async fn test_plugin_scope_combines_with_provider_filter() {
    let (p, col) = plugin_scope_fixture().await;
    let mut params = CollectionCardsParams::new(0, 100);
    params.provider = Some("plugin-comics".to_string());
    params.enabled_plugin_providers = Some(vec!["plugin-books".to_string()]);
    assert!(p.get_cards_in_collection_paginated(&col, params).await.unwrap().is_empty());

    let count = p
        .get_cards_in_collection_count(col, &["plugin-comics".to_string()], Some(&["plugin-books".to_string()]))
        .await
        .unwrap();
    assert_eq!(count, 0);
}
