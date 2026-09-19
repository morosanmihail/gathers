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
    finish: &str,
) -> Option<String> {
    let conn = persistence.connection.lock().await;
    conn.query_row(
        "SELECT timeupdated FROM cards WHERE collection = ?1 AND uuid = ?2 AND finish = ?3",
        params![collection_id, card_uuid, finish],
        |row| row.get(0),
    )
    .ok()
}

/// Convenience wrapper over the new per-finish `add_card_to_collection`,
/// keeping the old two-bucket (normal, foil) shape most tests were
/// written against — issues up to two calls under the hood, one per
/// nonzero finish.
pub async fn add_card(
    p: &mut SQLitePersistenceSystem,
    collection_id: &CollectionID,
    card_id: &CardID,
    quantity: i32,
    foil_quantity: i32,
) -> CardID {
    if quantity != 0 {
        p.add_card_to_collection(collection_id, card_id, "", quantity, OLD_TIME, "").await.unwrap();
    }
    if foil_quantity != 0 {
        p.add_card_to_collection(collection_id, card_id, "foil", foil_quantity, OLD_TIME, "").await.unwrap();
    }
    card_id.clone()
}

/// Convenience wrapper over the new per-finish `record_purchase`, keeping
/// the old two-bucket (normal, foil) shape most tests were written
/// against.
pub async fn record_purchase(
    p: &mut SQLitePersistenceSystem,
    col: &str,
    uuid: &str,
    qty: i32,
    foil: i32,
    price: Option<f64>,
) {
    if qty > 0 {
        p.record_purchase(&col.to_string(), &uuid.to_string(), "", qty, price, "prov", OLD_TIME).await.unwrap();
    }
    if foil > 0 {
        p.record_purchase(&col.to_string(), &uuid.to_string(), "foil", foil, price, "prov", OLD_TIME).await.unwrap();
    }
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

/// Verifies migration 08 (the finish-generalization migration) actually
/// splits pre-existing (quantity, foilquantity) rows into up to two
/// finish rows apiece, using the real embedded migration files (up to
/// version 7, i.e. the old schema) rather than a hand-copied schema that
/// could drift from the real one.
#[tokio::test]
async fn test_migration_08_splits_existing_rows_by_finish() {
    let mut conn = rusqlite::Connection::open(":memory:").unwrap();
    super::MIGRATIONS.to_version(&mut conn, 7).unwrap();

    // card_both: both owned and foil, plus a want (should end up on the
    //            "" row alongside quantity, not duplicated onto "foil").
    // card_foil_only: nothing owned, only foil copies.
    // card_want_only: fully wishlist, nothing owned at all (0/0/want>0).
    conn.execute_batch(
        "INSERT INTO cards (uuid, collection, quantity, foilquantity, want_quantity, timeadded, timeupdated, provider) VALUES
            ('card_both', 'Default', 3, 2, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'mtg'),
            ('card_foil_only', 'Default', 0, 5, 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'mtg'),
            ('card_want_only', 'Default', 0, 0, 4, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'mtg');
         INSERT INTO purchase_history (collection_id, card_uuid, quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit, provider, recorded_at) VALUES
            ('Default', 'card_both', 3, 2, 4.0, 9.0, 'mtg', '2024-01-01T00:00:00Z');",
    ).unwrap();

    super::MIGRATIONS.to_latest(&mut conn).unwrap();

    let mut stmt = conn.prepare("SELECT uuid, finish, quantity, want_quantity FROM cards ORDER BY uuid, finish").unwrap();
    let rows: Vec<(String, String, i32, i32)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            ("card_both".to_string(), "".to_string(), 3, 1),
            ("card_both".to_string(), "foil".to_string(), 2, 0),
            ("card_foil_only".to_string(), "foil".to_string(), 5, 0),
            ("card_want_only".to_string(), "".to_string(), 0, 4),
        ]
    );

    let mut stmt = conn.prepare("SELECT card_uuid, finish, quantity, price_per_unit FROM purchase_history ORDER BY finish").unwrap();
    let hist: Vec<(String, String, i32, Option<f64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        hist,
        vec![
            ("card_both".to_string(), "".to_string(), 3, Some(4.0)),
            ("card_both".to_string(), "foil".to_string(), 2, Some(9.0)),
        ]
    );
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
