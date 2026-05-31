use super::*;

// ── purchase totals ───────────────────────────────────────────────────────────

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

// ── get_all_purchase_history ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_all_purchase_history_empty() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    assert!(p.get_all_purchase_history(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_get_all_purchase_history_multiple_cards() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(3.0)).await;
    record_purchase(&mut p, &col, "card2", 1, 0, Some(7.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(5.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist.len(), 3);
    assert_eq!(hist.iter().filter(|e| e.card_uuid == "card1").count(), 2);
    assert_eq!(hist.iter().filter(|e| e.card_uuid == "card2").count(), 1);
}

#[tokio::test]
async fn test_get_all_purchase_history_isolated_by_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("A".to_string()).await.unwrap();
    let col_b = p.add_collection("B".to_string()).await.unwrap();
    record_purchase(&mut p, &col_a, "card1", 1, 0, Some(1.0)).await;
    record_purchase(&mut p, &col_b, "card1", 1, 0, Some(2.0)).await;

    let hist_a = p.get_all_purchase_history(&col_a).await.unwrap();
    assert_eq!(hist_a.len(), 1);
    assert_eq!(hist_a[0].normal_price_per_unit, Some(1.0));

    let hist_b = p.get_all_purchase_history(&col_b).await.unwrap();
    assert_eq!(hist_b.len(), 1);
    assert_eq!(hist_b[0].normal_price_per_unit, Some(2.0));
}

// ── history trimming on card removal ─────────────────────────────────────────

#[tokio::test]
async fn test_remove_cards_history_not_trimmed_when_sufficient() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(3.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(5.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), -1, 0).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    assert_eq!(hist.len(), 2);
    let total: i32 = hist.iter().map(|e| e.quantity).sum();
    assert_eq!(total, 3);
}

#[tokio::test]
async fn test_remove_cards_trims_lowest_price_entries() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(1.0)).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(3.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), -3, 0).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    let total: i32 = hist.iter().map(|e| e.quantity).sum();
    assert_eq!(total, 2);
    assert!(hist.iter().all(|e| e.normal_price_per_unit == Some(5.0)));
}

#[tokio::test]
async fn test_remove_cards_partial_entry_trim() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 0).await;
    record_purchase(&mut p, &col, "card1", 3, 0, Some(1.0)).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(9.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), -2, 0).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    assert_eq!(hist.len(), 2);
    let cheap = hist.iter().find(|e| e.normal_price_per_unit == Some(1.0)).unwrap();
    assert_eq!(cheap.quantity, 1);
    let exp = hist.iter().find(|e| e.normal_price_per_unit == Some(9.0)).unwrap();
    assert_eq!(exp.quantity, 2);
}

#[tokio::test]
async fn test_remove_all_cards_clears_history() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 3, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(4.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(7.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), -3, 0).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    assert!(hist.is_empty());
}

#[tokio::test]
async fn test_remove_cards_null_price_trimmed_first() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 4, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, None).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), -2, 0).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    let total: i32 = hist.iter().map(|e| e.quantity).sum();
    assert_eq!(total, 2);
    assert!(hist.iter().all(|e| e.normal_price_per_unit == Some(5.0)));
}

#[tokio::test]
async fn test_foil_history_trimmed_independently() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 0, 4).await;
    record_purchase(&mut p, &col, "card1", 0, 2, Some(2.0)).await;
    record_purchase(&mut p, &col, "card1", 0, 2, Some(8.0)).await;

    add_card(&mut p, &col, &"card1".to_string(), 0, -2).await;

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    let foil_total: i32 = hist.iter().map(|e| e.foil_quantity).sum();
    assert_eq!(foil_total, 2);
    assert!(hist.iter().all(|e| e.foil_price_per_unit == Some(8.0)));
}

#[tokio::test]
async fn test_move_same_collection_no_history_corruption() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 3, 0).await;
    record_purchase(&mut p, &col, "card1", 3, 0, Some(5.0)).await;

    p.move_cards_between_collections(
        &[CollectionCard {
            uuid: "card1".to_string(), quantity: 3, foil_quantity: 0,
            time_added: OLD_TIME.to_string(), collection: col.clone(), provider: "".to_string(),
        }],
        col.clone(),
    ).await.unwrap();

    let hist = p.get_purchase_history(&col, &"card1".to_string()).await.unwrap();
    assert_eq!(hist.iter().map(|e| e.quantity).sum::<i32>(), 3);
}

#[tokio::test]
async fn test_move_cards_trims_source_history() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    add_card(&mut p, &col_a, &"card1".to_string(), 4, 0).await;
    record_purchase(&mut p, &col_a, "card1", 2, 0, Some(1.0)).await;
    record_purchase(&mut p, &col_a, "card1", 2, 0, Some(9.0)).await;

    p.move_cards_between_collections(
        &[CollectionCard {
            uuid: "card1".to_string(), quantity: 3, foil_quantity: 0,
            time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string(),
        }],
        col_b.clone(),
    ).await.unwrap();

    let hist = p.get_purchase_history(&col_a, &"card1".to_string()).await.unwrap();
    assert_eq!(hist.iter().map(|e| e.quantity).sum::<i32>(), 1);
    assert!(hist.iter().all(|e| e.normal_price_per_unit == Some(9.0)));
}

#[tokio::test]
async fn test_move_cards_transfers_history_to_destination() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    add_card(&mut p, &col_a, &"card1".to_string(), 4, 0).await;
    record_purchase(&mut p, &col_a, "card1", 2, 0, Some(1.0)).await;
    record_purchase(&mut p, &col_a, "card1", 2, 0, Some(9.0)).await;

    p.move_cards_between_collections(
        &[CollectionCard {
            uuid: "card1".to_string(), quantity: 3, foil_quantity: 0,
            time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string(),
        }],
        col_b.clone(),
    ).await.unwrap();

    let hist_a = p.get_purchase_history(&col_a, &"card1".to_string()).await.unwrap();
    assert_eq!(hist_a.iter().map(|e| e.quantity).sum::<i32>(), 1);
    assert!(hist_a.iter().all(|e| e.normal_price_per_unit == Some(9.0)));

    let hist_b = p.get_purchase_history(&col_b, &"card1".to_string()).await.unwrap();
    assert_eq!(hist_b.iter().map(|e| e.quantity).sum::<i32>(), 3);
    assert_eq!(hist_b.iter().find(|e| e.normal_price_per_unit == Some(1.0)).unwrap().quantity, 2);
    assert_eq!(hist_b.iter().find(|e| e.normal_price_per_unit == Some(9.0)).unwrap().quantity, 1);
}

#[tokio::test]
async fn test_move_cards_foil_history_transferred() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("Collection A".to_string()).await.unwrap();
    let col_b = p.add_collection("Collection B".to_string()).await.unwrap();
    add_card(&mut p, &col_a, &"card1".to_string(), 0, 3).await;
    record_purchase(&mut p, &col_a, "card1", 0, 2, Some(2.0)).await;
    record_purchase(&mut p, &col_a, "card1", 0, 1, Some(8.0)).await;

    p.move_cards_between_collections(
        &[CollectionCard {
            uuid: "card1".to_string(), quantity: 0, foil_quantity: 2,
            time_added: OLD_TIME.to_string(), collection: col_a.clone(), provider: "".to_string(),
        }],
        col_b.clone(),
    ).await.unwrap();

    let hist_a = p.get_purchase_history(&col_a, &"card1".to_string()).await.unwrap();
    assert_eq!(hist_a.iter().map(|e| e.foil_quantity).sum::<i32>(), 1);
    assert!(hist_a.iter().all(|e| e.foil_price_per_unit == Some(8.0)));

    let hist_b = p.get_purchase_history(&col_b, &"card1".to_string()).await.unwrap();
    assert_eq!(hist_b.iter().map(|e| e.foil_quantity).sum::<i32>(), 2);
}

// ── delete_purchase_entry ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_purchase_entry_removes_entry() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(3.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist[0].id;

    let deleted = p.delete_purchase_entry(&col, id).await.unwrap();
    assert!(deleted);
    assert!(p.get_all_purchase_history(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_purchase_entry_returns_false_when_not_found() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    let deleted = p.delete_purchase_entry(&col, 9999).await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_delete_purchase_entry_isolated_by_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("A".to_string()).await.unwrap();
    let col_b = p.add_collection("B".to_string()).await.unwrap();
    record_purchase(&mut p, &col_a, "card1", 1, 0, Some(5.0)).await;

    let hist = p.get_all_purchase_history(&col_a).await.unwrap();
    let id = hist[0].id;

    let deleted = p.delete_purchase_entry(&col_b, id).await.unwrap();
    assert!(!deleted);
    assert_eq!(p.get_all_purchase_history(&col_a).await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_delete_purchase_entry_leaves_other_entries_intact() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 1, 0, Some(1.0)).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(2.0)).await;
    record_purchase(&mut p, &col, "card2", 3, 0, Some(5.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id_to_delete = hist.iter().find(|e| e.normal_price_per_unit == Some(1.0)).unwrap().id;

    p.delete_purchase_entry(&col, id_to_delete).await.unwrap();

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist.len(), 2);
    assert!(!hist.iter().any(|e| e.normal_price_per_unit == Some(1.0)));
    assert!(hist.iter().any(|e| e.normal_price_per_unit == Some(2.0)));
    assert!(hist.iter().any(|e| e.card_uuid == "card2"));
}

#[tokio::test]
async fn test_delete_purchase_entry_updates_totals() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    record_purchase(&mut p, &col, "card1", 2, 0, Some(4.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(8.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist.iter().find(|e| e.normal_price_per_unit == Some(4.0)).unwrap().id;
    p.delete_purchase_entry(&col, id).await.unwrap();

    let totals = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = totals.get("card1").unwrap();
    assert_eq!(s.quantity, 1);
    assert!((s.total_normal_paid - 8.0).abs() < 1e-9);
}

// ── update_purchase_entry ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_update_purchase_entry_changes_quantity() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 5, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(5.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist[0].id;

    let result = p.update_purchase_entry(&col, id, 5, 0, Some(5.0), None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::Updated);

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].quantity, 5);
    assert_eq!(hist[0].normal_price_per_unit, Some(5.0));
}

#[tokio::test]
async fn test_update_purchase_entry_changes_price() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 3, 0).await;
    record_purchase(&mut p, &col, "card1", 3, 0, Some(2.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist[0].id;

    p.update_purchase_entry(&col, id, 3, 0, Some(9.99), None).await.unwrap();

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert!((hist[0].normal_price_per_unit.unwrap() - 9.99).abs() < 1e-9);
}

#[tokio::test]
async fn test_update_purchase_entry_clears_price_to_null() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 2, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(4.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist[0].id;

    p.update_purchase_entry(&col, id, 2, 0, None, None).await.unwrap();

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].normal_price_per_unit, None);
    assert!(p.get_collection_purchase_totals(&col).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_update_purchase_entry_returns_false_when_not_found() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    let result = p.update_purchase_entry(&col, 9999, 1, 0, None, None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::NotFound);
}

#[tokio::test]
async fn test_update_purchase_entry_isolated_by_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col_a = p.add_collection("A".to_string()).await.unwrap();
    let col_b = p.add_collection("B".to_string()).await.unwrap();
    add_card(&mut p, &col_a, &"card1".to_string(), 2, 0).await;
    record_purchase(&mut p, &col_a, "card1", 2, 0, Some(3.0)).await;

    let hist = p.get_all_purchase_history(&col_a).await.unwrap();
    let id = hist[0].id;

    let result = p.update_purchase_entry(&col_b, id, 2, 0, Some(999.0), None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::NotFound);

    let hist = p.get_all_purchase_history(&col_a).await.unwrap();
    assert_eq!(hist[0].quantity, 2);
    assert_eq!(hist[0].normal_price_per_unit, Some(3.0));
}

#[tokio::test]
async fn test_update_purchase_entry_foil_fields() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 0, 3).await;
    record_purchase(&mut p, &col, "card1", 0, 2, Some(6.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id = hist[0].id;

    p.update_purchase_entry(&col, id, 0, 3, None, Some(7.50)).await.unwrap();

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].foil_quantity, 3);
    assert_eq!(hist[0].foil_price_per_unit, Some(7.50));
    assert_eq!(hist[0].normal_price_per_unit, None);
}

#[tokio::test]
async fn test_update_purchase_entry_reflects_in_totals() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 4, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(3.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let result = p.update_purchase_entry(&col, hist[0].id, 4, 0, Some(5.0), None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::Updated);

    let totals = p.get_collection_purchase_totals(&col).await.unwrap();
    let s = totals.get("card1").unwrap();
    assert_eq!(s.quantity, 4);
    assert!((s.total_normal_paid - 20.0).abs() < 1e-9);
}

// ── update_purchase_entry validation ─────────────────────────────────────────

#[tokio::test]
async fn test_update_entry_rejects_qty_exceeding_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 3, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(1.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let result = p.update_purchase_entry(&col, hist[0].id, 5, 0, Some(1.0), None).await.unwrap();
    assert!(matches!(result, UpdateEntryResult::ValidationError(_)));

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].quantity, 2);
}

#[tokio::test]
async fn test_update_entry_rejects_foil_qty_exceeding_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 0, 2).await;
    record_purchase(&mut p, &col, "card1", 0, 1, Some(3.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let result = p.update_purchase_entry(&col, hist[0].id, 0, 10, None, Some(3.0)).await.unwrap();
    assert!(matches!(result, UpdateEntryResult::ValidationError(_)));

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].foil_quantity, 1);
}

#[tokio::test]
async fn test_update_entry_qty_equal_to_collection_is_allowed() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 3, 0).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(2.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let result = p.update_purchase_entry(&col, hist[0].id, 3, 0, Some(2.0), None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::Updated);

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(hist[0].quantity, 3);
}

#[tokio::test]
async fn test_update_entry_validation_counts_other_entries() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 4, 0).await;
    record_purchase(&mut p, &col, "card1", 2, 0, Some(1.0)).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(2.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let id_of_first = hist.iter().find(|e| e.normal_price_per_unit == Some(1.0)).unwrap().id;

    let result = p.update_purchase_entry(&col, id_of_first, 4, 0, Some(1.0), None).await.unwrap();
    assert!(matches!(result, UpdateEntryResult::ValidationError(_)));

    let result = p.update_purchase_entry(&col, id_of_first, 3, 0, Some(1.0), None).await.unwrap();
    assert_eq!(result, UpdateEntryResult::Updated);
}

#[tokio::test]
async fn test_update_entry_error_message_mentions_counts() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = p.add_collection("Col".to_string()).await.unwrap();
    add_card(&mut p, &col, &"card1".to_string(), 2, 0).await;
    record_purchase(&mut p, &col, "card1", 1, 0, Some(5.0)).await;

    let hist = p.get_all_purchase_history(&col).await.unwrap();
    let result = p.update_purchase_entry(&col, hist[0].id, 99, 0, Some(5.0), None).await.unwrap();
    if let UpdateEntryResult::ValidationError(msg) = result {
        assert!(msg.contains("99"), "message should mention requested qty: {msg}");
        assert!(msg.contains("2"), "message should mention collection qty: {msg}");
    } else {
        panic!("expected ValidationError");
    }
}
