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

// `card()` above leaves most fields `None`, which round-trip as NULL. Cards still load with
// those missing (see `test_cards_with_missing_fields_still_load`), but the random-card tests
// use fully-populated rows.
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

// ── missing fields ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cards_with_missing_fields_still_load() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("riftbound.db");
    let db_path = db_path.to_str().unwrap().to_string();
    // Only an id, a set and a name: no text, artists, domains, image or rarity, as for a rune.
    upsert_riftbound_cards(&db_path, &[card("ogn-126-298", "OGN", "Body Rune")]).unwrap();
    let system = RiftboundSQLiteRetrievalSystem::new(Some(db_path)).unwrap();

    let found = system
        .search_cards(CardSearchFilters::new().with_name("Body Rune"), None, Some(10))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    let by_id = system.get_cards_by_ids(vec!["ogn-126-298".to_string()]).await.unwrap();
    match &by_id["ogn-126-298"] {
        Card::Riftbound(c) => {
            assert_eq!(c.text, "");
            assert!(c.artists.is_empty());
        }
        _ => panic!("expected a Riftbound card"),
    }
}

// ── unique modes ─────────────────────────────────────────────────────────────

fn printing(id: &str, set: &str, name: &str, text: Option<&str>, rarity: &str) -> SimplifiedCard {
    SimplifiedCard {
        rarity: Some(rarity.to_string()),
        ability_html: text.map(str::to_string),
        collector_number: Some(serde_json::Value::String("1".to_string())),
        ..card(id, set, name)
    }
}

/// | id               | card                                       |
/// |------------------|--------------------------------------------|
/// | ogn-164-298      | Sett (text A), regular                     |
/// | ogn-164a-298     | Sett (text A), alternate art               |
/// | sfd-232-star-221 | Sett (text A), star variant                |
/// | sfd-232-221      | Sett (text A), regular, in a later set     |
/// | ven-sp4-006      | Sett (text A), special                     |
/// | ogn-240-298      | Sett (text B): a different card            |
/// | ogn-126-298      | Body Rune, no text                         |
/// | ven-r04          | Body Rune, no text                         |
/// | ogn-001-298      | Annie, text with markup                    |
/// | ogs-001-024      | Annie, same text plain and re-spaced       |
///
/// so `prints` finds 10 cards and `cards` 4.
fn unique_fixture(dir: &TempDir) -> RiftboundSQLiteRetrievalSystem {
    let db_path = dir.path().join("riftbound.db").to_str().unwrap().to_string();
    let text_a = Some("<p>When I'm played, draw.</p>");
    upsert_riftbound_cards(
        &db_path,
        &[
            printing("ogn-164-298", "OGN", "Sett", text_a, "Epic"),
            printing("ogn-164a-298", "OGN", "Sett", text_a, "Showcase"),
            printing("sfd-232-star-221", "SFD", "Sett", text_a, "Showcase"),
            printing("sfd-232-221", "SFD", "Sett", text_a, "Showcase"),
            printing("ven-sp4-006", "VEN", "Sett", text_a, "Epic"),
            printing("ogn-240-298", "OGN", "Sett", Some("[Tank]"), "Rare"),
            printing("ogn-126-298", "OGN", "Body Rune", None, "Common"),
            printing("ven-r04", "VEN", "Body Rune", None, "Common"),
            printing("ogn-001-298", "OGN", "Annie", Some("<p>Hello  <b>world</b></p>"), "Epic"),
            printing("ogs-001-024", "OGS", "Annie", Some("hello world"), "Epic"),
        ],
    )
    .unwrap();
    RiftboundSQLiteRetrievalSystem::new(Some(db_path)).unwrap()
}

async fn ids(system: &RiftboundSQLiteRetrievalSystem, filters: CardSearchFilters, skip: usize, limit: usize) -> Vec<String> {
    system
        .search_cards(filters, Some(skip), Some(limit))
        .await
        .unwrap()
        .into_iter()
        .map(|c| match c {
            Card::Riftbound(r) => r.id,
            _ => unreachable!(),
        })
        .collect()
}

#[tokio::test]
async fn test_unique_modes_offered() {
    let modes = RiftboundSQLiteRetrievalSystem::new(None).unwrap().unique_modes();
    let ids: Vec<_> = modes.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["prints", "cards"]);
}

#[tokio::test]
async fn test_unique_defaults_to_prints() {
    let dir = TempDir::new().unwrap();
    let system = unique_fixture(&dir);
    let all = |f: CardSearchFilters| ids(&system, f, 0, 100);

    let by_default = all(CardSearchFilters::new()).await;
    assert_eq!(by_default.len(), 10);
    assert_eq!(all(CardSearchFilters::new().with_unique("prints")).await, by_default);
    assert_eq!(all(CardSearchFilters::new().with_unique("")).await, by_default);
}

#[tokio::test]
async fn test_cards_collapses_printings_to_the_regular_one() {
    let dir = TempDir::new().unwrap();
    let system = unique_fixture(&dir);
    let cards = ids(&system, CardSearchFilters::new().with_unique("cards"), 0, 100).await;
    // Sorted by name. Sett's text-A printings all collapse, across sets and art variants, to
    // the regular one in the earliest set; the other Sett has different text and stays. The
    // runes have no text and collapse on name. Annie's text differs only in markup.
    assert_eq!(cards, ["ogn-001-298", "ogn-126-298", "ogn-164-298", "ogn-240-298"]);
}

#[tokio::test]
async fn test_cards_picks_the_representative_among_the_matching_printings() {
    let dir = TempDir::new().unwrap();
    let system = unique_fixture(&dir);
    for (set, expected) in [("SFD", "sfd-232-221"), ("VEN", "ven-sp4-006")] {
        let filters = CardSearchFilters::new().with_name("Sett").with_set_code(set).with_unique("cards");
        assert_eq!(ids(&system, filters, 0, 10).await, [expected], "set {set}");
    }
}

#[tokio::test]
async fn test_cards_sorts_and_pages_the_collapsed_results() {
    let dir = TempDir::new().unwrap();
    let system = unique_fixture(&dir);
    use ::models::filters::{SortField, SortOrder};
    for sort in [SortField::Name, SortField::SetCode, SortField::Rarity] {
        for order in [SortOrder::Asc, SortOrder::Desc] {
            let filters = || CardSearchFilters::new().with_unique("cards").with_sort_by(sort.clone()).with_sort_order(order.clone());
            let whole = ids(&system, filters(), 0, 100).await;
            assert_eq!(whole.len(), 4, "{sort:?} {order:?}");
            let mut paged = vec![];
            for page in 0..whole.len() {
                paged.extend(ids(&system, filters(), page, 1).await);
            }
            assert_eq!(paged, whole, "{sort:?} {order:?}");
        }
    }
}

#[tokio::test]
async fn test_unique_rejects_unknown_mode() {
    let dir = TempDir::new().unwrap();
    let system = unique_fixture(&dir);
    let err = system
        .search_cards(CardSearchFilters::new().with_unique("art"), None, Some(1))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Unsupported unique mode 'art'"), "{err}");
    assert!(err.to_string().contains("prints, cards"), "{err}");
}

#[test]
fn test_variant_rank() {
    for (id, rank) in [
        ("ogn-164-298", 0),
        ("ven-r04", 0),
        ("unl-t01", 0),
        ("sfd-232-star-221", 1),
        ("ogn-164a-298", 2),
        ("ven-sp4-006", 3),
    ] {
        assert_eq!(unique::variant_rank(id), rank, "{id}");
    }
}
