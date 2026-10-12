use super::*;
use crate::{PricePoint, latest_price_history_schema_version, latest_storage_schema_version};
use std::path::{Path, PathBuf};

/// A file-backed store (as the server uses) with price history on.
fn open(dir: &Path, name: &str) -> SQLitePersistenceSystem {
    let path = |ext: &str| dir.join(format!("{name}.{ext}")).to_string_lossy().into_owned();
    let mut p = SQLitePersistenceSystem::new(false, Some(path("db"))).unwrap();
    p.enable_price_history(false, Some(path("prices.db"))).unwrap();
    p
}

fn price(uuid: &str, value: f64) -> PricePoint {
    PricePoint {
        card_uuid: uuid.to_string(),
        retailer: "shop".to_string(),
        finish: String::new(),
        price: value,
        currency: "EUR".to_string(),
        recorded_on: "2026-01-01".parse().unwrap(),
    }
}

/// Exports `p` into `dir`, returning the storage and price history copies.
async fn export(p: &SQLitePersistenceSystem, dir: &Path) -> (PathBuf, PathBuf) {
    let storage = dir.join("backup-storage.db");
    let prices = dir.join("backup-prices.db");
    assert_eq!(p.export_storage(&storage).await.unwrap(), latest_storage_schema_version());
    assert_eq!(p.export_price_history(&prices).await.unwrap(), Some(latest_price_history_schema_version()));
    (storage, prices)
}

async fn collections(p: &SQLitePersistenceSystem) -> Vec<String> {
    p.list_collections(None).await.unwrap()
}

fn invalid_input(e: eyre::Report) -> String {
    match e.downcast_ref::<PersistenceError>() {
        Some(PersistenceError::InvalidInput(msg)) => msg.clone(),
        other => panic!("expected InvalidInput, got {other:?}: {e}"),
    }
}

#[tokio::test]
async fn restore_replaces_both_databases() {
    let dir = tempfile::tempdir().unwrap();
    let mut source = open(dir.path(), "source");
    source.add_collection("Binder".to_string()).await.unwrap();
    add_card(&mut source, &"Binder".to_string(), &"card-1".to_string(), 3, 1).await;
    source.record_prices("", &[price("card-1", 2.5)]).await.unwrap();
    let (storage, prices) = export(&source, dir.path()).await;

    let mut target = open(dir.path(), "target");
    target.add_collection("Doomed".to_string()).await.unwrap();
    target.record_prices("", &[price("card-2", 9.0)]).await.unwrap();
    target.restore(&storage, Some(&prices)).await.unwrap();

    assert_eq!(collections(&target).await, collections(&source).await);
    let cards = target
        .get_cards_in_collection_paginated(&"Binder".to_string(), CollectionCardsParams::new(0, 10))
        .await
        .unwrap();
    assert_eq!(cards.iter().map(|c| c.quantity).sum::<i32>(), 4);
    assert_eq!(target.get_price_history("", &"card-1".to_string()).await.unwrap().len(), 1);
    assert!(target.get_price_history("", &"card-2".to_string()).await.unwrap().is_empty());

    // Still usable and still in WAL mode afterwards.
    target.add_collection("After".to_string()).await.unwrap();
    let mode: String = target.connection.lock().await.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");

    // And it's the files that changed, not just the open connections.
    drop(target);
    let reopened = open(dir.path(), "target");
    let names = collections(&reopened).await;
    assert!(names.contains(&"Binder".to_string()) && names.contains(&"After".to_string()), "{names:?}");
    assert_eq!(reopened.get_price_history("", &"card-1".to_string()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn restore_without_price_history_keeps_existing_prices() {
    let dir = tempfile::tempdir().unwrap();
    let source = open(dir.path(), "source");
    let (storage, _) = export(&source, dir.path()).await;

    let mut target = open(dir.path(), "target");
    target.record_prices("", &[price("card-2", 9.0)]).await.unwrap();
    target.restore(&storage, None).await.unwrap();
    assert_eq!(target.get_price_history("", &"card-2".to_string()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn restore_migrates_older_backups() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.db");
    {
        let mut conn = rusqlite::Connection::open(&old).unwrap();
        super::super::MIGRATIONS.to_version(&mut conn, 1).unwrap();
        conn.execute("INSERT INTO collection (name) VALUES ('Old')", []).unwrap();
    }
    let target = open(dir.path(), "target");
    target.restore(&old, None).await.unwrap();
    assert!(collections(&target).await.contains(&"Old".to_string()));
    let version: i64 = target.connection.lock().await.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version as usize, latest_storage_schema_version());
}

#[tokio::test]
async fn restore_rejects_newer_schema() {
    let dir = tempfile::tempdir().unwrap();
    let source = open(dir.path(), "source");
    let (storage, _) = export(&source, dir.path()).await;
    rusqlite::Connection::open(&storage)
        .unwrap()
        .pragma_update(None, "user_version", latest_storage_schema_version() as i64 + 1)
        .unwrap();

    let mut target = open(dir.path(), "target");
    target.add_collection("Kept".to_string()).await.unwrap();
    let msg = invalid_input(target.restore(&storage, None).await.unwrap_err());
    assert!(msg.contains("newer"), "{msg}");
    assert!(collections(&target).await.contains(&"Kept".to_string()));
}

#[tokio::test]
async fn restore_rejects_bad_files_without_changing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let source = open(dir.path(), "source");
    let (storage, prices) = export(&source, dir.path()).await;
    let garbage = dir.path().join("garbage.db");
    std::fs::write(&garbage, b"definitely not a database, just some text padding it out").unwrap();

    let mut target = open(dir.path(), "target");
    target.add_collection("Kept".to_string()).await.unwrap();

    invalid_input(target.restore(&garbage, None).await.unwrap_err());
    // The wrong kind of database in either slot.
    invalid_input(target.restore(&prices, None).await.unwrap_err());
    invalid_input(target.restore(&storage, Some(&storage)).await.unwrap_err());
    assert!(collections(&target).await.contains(&"Kept".to_string()));
}
