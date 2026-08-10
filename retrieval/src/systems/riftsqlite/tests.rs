use super::*;
use tempfile::TempDir;

fn card(id: &str, set: &str, name: &str) -> SimplifiedCard {
    SimplifiedCard {
        id: Some(id.to_string()),
        set: Some(set.to_string()),
        name: Some(name.to_string()),
        ..Default::default()
    }
}

fn card_names(db_path: &str) -> Vec<(String, String)> {
    let conn = Connection::open(db_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT id, name FROM cards ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .flatten()
        .collect()
}

#[test]
fn test_upsert_inserts_into_empty_db() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap();

    let cards = vec![card("c1", "OGN", "Card One"), card("c2", "OGN", "Card Two")];
    upsert_riftbound_cards(db_path, &cards).unwrap();

    assert_eq!(
        card_names(db_path),
        vec![
            ("c1".to_string(), "Card One".to_string()),
            ("c2".to_string(), "Card Two".to_string())
        ]
    );
}

#[test]
fn test_upsert_skips_set_already_present() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap();

    // First pass: OGN set saved.
    upsert_riftbound_cards(db_path, &[card("c1", "OGN", "Original Name")]).unwrap();

    // Second pass: upstream "changed" c1's name and added a brand-new set.
    // OGN must be left untouched; the new set must still be inserted.
    let second_pass = vec![
        card("c1", "OGN", "Renamed By Upstream"),
        card("c2", "NEW", "New Set Card"),
    ];
    upsert_riftbound_cards(db_path, &second_pass).unwrap();

    let names = card_names(db_path);
    assert_eq!(
        names,
        vec![
            ("c1".to_string(), "Original Name".to_string()),
            ("c2".to_string(), "New Set Card".to_string())
        ],
        "card from already-known set must not be re-written, new set must still be added"
    );
}

#[test]
fn test_upsert_card_without_set_always_processed() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap();

    upsert_riftbound_cards(db_path, &[card("c1", "OGN", "Card One")]).unwrap();

    let mut no_set = card("c2", "OGN", "Should Be Skipped Set");
    no_set.set = None;
    upsert_riftbound_cards(db_path, &[no_set]).unwrap();

    let names = card_names(db_path);
    assert!(
        names.iter().any(|(id, _)| id == "c2"),
        "card with no set must not be skipped by the set-skip logic"
    );
}

// `card()` above leaves most fields `None`, which round-trip as NULL — fine
// for the upsert tests but `SqlCard::from_row` requires non-null text/
// artists/domains/image, so random-card tests need fully-populated rows.
fn full_card(id: &str, set: &str, name: &str) -> SimplifiedCard {
    SimplifiedCard {
        id: Some(id.to_string()),
        set: Some(set.to_string()),
        name: Some(name.to_string()),
        rarity: Some("Common".to_string()),
        artists: Some(vec!["Some Artist".to_string()]),
        domain_ids: Some(vec!["fury".to_string()]),
        ability_html: Some("Deal 1 damage.".to_string()),
        image_url: Some("https://example.com/card.png".to_string()),
        // The `code`/collector-number column is populated from
        // `collector_number`, not `code` — see `upsert_riftbound_cards`.
        collector_number: Some(serde_json::Value::String("001".to_string())),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_get_random_card() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap().to_string();

    upsert_riftbound_cards(&db_path, &[full_card("c1", "OGN", "Card One")]).unwrap();

    let system = RiftboundSQLiteRetrievalSystem::new(Some(db_path)).unwrap();
    let result = system.get_random_card().await.unwrap();
    assert!(result.is_some());
    match result.unwrap() {
        ::models::Card::Riftbound(c) => assert_eq!(c.name, "Card One"),
        _ => panic!("expected a Riftbound card"),
    }
}

#[tokio::test]
async fn test_get_random_card_varies() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap().to_string();

    let cards: Vec<SimplifiedCard> = (0..10)
        .map(|i| full_card(&format!("c{i}"), "OGN", &format!("Card {i}")))
        .collect();
    upsert_riftbound_cards(&db_path, &cards).unwrap();

    let system = RiftboundSQLiteRetrievalSystem::new(Some(db_path)).unwrap();
    let mut names = std::collections::HashSet::new();
    for _ in 0..20 {
        let card = system.get_random_card().await.unwrap().unwrap();
        if let ::models::Card::Riftbound(c) = card {
            names.insert(c.name);
        }
    }
    assert!(names.len() > 1, "expected varying random cards, got {names:?}");
}
