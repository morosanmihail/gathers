// Re-export everything submodules need via `use super::*`
pub use super::SQLitePersistenceSystem;
pub use models::{CardID, CollectionCard, CollectionID};
pub use models::filters::SortOrder;
pub use crate::{CollectionCardsParams, CollectionSortField, PersistenceSystemTrait, UpdateEntryResult};
pub use rusqlite::params;

pub const DEFAULT: &str = "Default";
pub const OLD_TIME: &str = "2023-01-01T00:00:00Z";

pub async fn get_time_updated(
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

pub async fn add_card(
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

pub async fn record_purchase(
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

mod cards;
mod collections;
mod move_cards;
mod purchase;
mod sorting;
mod timeupdated;

// ── infrastructure tests ──────────────────────────────────────────────────────

#[test]
fn migrations_test() {
    assert!(super::MIGRATIONS.validate().is_ok());
}

#[test]
fn test_new_with_file_path() {
    let dir = std::env::temp_dir();
    let path = dir.join("gathers_test_persistence.db");
    let _ = std::fs::remove_file(&path);
    let p = SQLitePersistenceSystem::new(false, Some(path.to_str().unwrap().to_string()));
    assert!(p.is_ok());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_new_creates_parent_directories() {
    let dir = std::env::temp_dir().join("gathers_test_nested_dir");
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("sub").join("persistence.db");
    let p = SQLitePersistenceSystem::new(false, Some(path.to_str().unwrap().to_string()));
    assert!(p.is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
