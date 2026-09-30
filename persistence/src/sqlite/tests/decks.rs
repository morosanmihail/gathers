use super::*;
use models::CollectionKind;

const NOW: &str = "2024-06-01T00:00:00Z";

async fn new_deck(p: &mut SQLitePersistenceSystem, name: &str) -> CollectionID {
    p.add_collection(name.to_string(), CollectionKind::Deck)
        .await
        .unwrap()
}

/// Convenience: put `quantity` copies of `uuid` on `board` of `deck`.
async fn deck_add(
    p: &mut SQLitePersistenceSystem,
    deck: &CollectionID,
    uuid: &str,
    board: &str,
    quantity: i32,
) {
    p.deck_mut(deck).adjust_card(&uuid.to_string(), "", board, quantity, NOW, "mtg")
        .await
        .unwrap();
}

fn qty(cards: &[models::DeckCard], uuid: &str, board: &str) -> i32 {
    cards
        .iter()
        .find(|c| c.uuid == uuid && c.board == board)
        .map(|c| c.quantity)
        .unwrap_or(0)
}

// ── kind ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_kind_round_trips_and_defaults_to_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    let col = new_collection(&mut p, "Binder".to_string()).await;

    assert_eq!(
        p.get_collection_kind(&deck).await.unwrap(),
        Some(CollectionKind::Deck)
    );
    assert_eq!(
        p.get_collection_kind(&col).await.unwrap(),
        Some(CollectionKind::Collection)
    );
    // Pre-existing rows (here: the seeded Default) predate the column and take its default.
    assert_eq!(
        p.get_collection_kind(&DEFAULT.to_string()).await.unwrap(),
        Some(CollectionKind::Collection)
    );
    assert_eq!(
        p.get_collection_kind(&"nope".to_string()).await.unwrap(),
        None
    );

    // A deck is listed alongside collections, and says which it is; that's how a client knows to
    // send its card edits to the deck endpoints.
    let listed = p.list_collection_info().await.unwrap();
    let listed_deck = listed.iter().find(|c| c.name == "Burn").unwrap();
    assert_eq!(listed_deck.kind, CollectionKind::Deck);
    let listed_col = listed.iter().find(|c| c.name == "Binder").unwrap();
    assert_eq!(listed_col.kind, CollectionKind::Collection);
}

/// Every read of the collection list parses this column, and `.collect()` over a `Result` means
/// one unparseable row fails the whole list rather than that entry. The CHECK keeps such a value
/// out of the database.
#[tokio::test]
async fn test_an_unknown_kind_cannot_be_stored() {
    let p = SQLitePersistenceSystem::new(true, None).unwrap();
    let conn = p.connection.lock().await;

    let stored = conn.execute(
        "INSERT INTO collection (name, can_remove, kind) VALUES ('Sideboard', 1, 'sideboard')",
        [],
    );

    assert!(stored.is_err(), "an unrecognised kind must not be storable");
}

// ── the additive semantics ───────────────────────────────────────────────────

/// A deck records what is to be played, a collection records what is owned, and putting a card
/// in a deck changes only the former.
#[tokio::test]
async fn test_adding_to_a_deck_does_not_touch_the_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;

    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    let owned = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(owned.len(), 1, "collection still holds exactly one row");
    assert_eq!(owned[0].quantity, 4, "collection quantity must not move");

    assert_eq!(qty(&p.deck(&deck).cards().await.unwrap(), "bolt", "main"), 4);
}

/// A deck can hold a card the owner doesn't have at all: the over-commitment case is reported,
/// not prevented, so nothing here may fail.
#[tokio::test]
async fn test_deck_can_hold_cards_not_owned() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    let availability = p.card_availability(&["bolt".to_string()]).await.unwrap();
    assert_eq!(availability[0].owned, 0);
}

/// Counts copies, not rows and not distinct cards: a deck is described by how many cards are in
/// it, so four of one card across two boards is four.
#[tokio::test]
async fn test_deck_card_count_sums_copies_across_boards() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    assert_eq!(p.deck(&deck).card_count().await.unwrap(), 0, "an empty deck counts zero");

    deck_add(&mut p, &deck, "bolt", "main", 3).await;
    deck_add(&mut p, &deck, "bolt", "side", 1).await;
    deck_add(&mut p, &deck, "path", "main", 2).await;

    assert_eq!(p.deck(&deck).card_count().await.unwrap(), 6);
}

// ── boards ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_same_card_on_two_boards_is_two_rows() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;

    deck_add(&mut p, &deck, "bolt", "main", 4).await;
    deck_add(&mut p, &deck, "bolt", "side", 2).await;

    let cards = p.deck(&deck).cards().await.unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(qty(&cards, "bolt", "main"), 4);
    assert_eq!(qty(&cards, "bolt", "side"), 2);
}

/// `board` is an open string, not a fixed main/side pair: Riftbound decks have a legend, runes, and
/// battlefields, and MTG Commander has a command zone.
#[tokio::test]
async fn test_board_is_open_ended() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Riftbound Deck").await;

    for board in ["", "main", "legend", "runes", "battlefield"] {
        deck_add(&mut p, &deck, "card", board, 1).await;
    }

    let cards = p.deck(&deck).cards().await.unwrap();
    assert_eq!(cards.len(), 5, "every zone gets a separate row");
    assert_eq!(qty(&cards, "card", "battlefield"), 1);
}

#[tokio::test]
async fn test_removing_down_to_zero_deletes_the_row() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;

    deck_add(&mut p, &deck, "bolt", "main", 2).await;
    deck_add(&mut p, &deck, "bolt", "main", -2).await;

    assert!(p.deck(&deck).cards().await.unwrap().is_empty());
}

/// Removing from a board the card was never on must be a no-op, not a negative-quantity ghost row.
#[tokio::test]
async fn test_removing_from_an_empty_board_is_a_noop() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;

    deck_add(&mut p, &deck, "bolt", "main", -3).await;

    assert!(p.deck(&deck).cards().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_move_between_boards_conserves_copies() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    p.deck_mut(&deck).move_card(&"bolt".to_string(), "", "main", "side", 3)
        .await
        .unwrap();

    let cards = p.deck(&deck).cards().await.unwrap();
    assert_eq!(qty(&cards, "bolt", "main"), 1);
    assert_eq!(qty(&cards, "bolt", "side"), 3);
    let total: i32 = cards.iter().map(|c| c.quantity).sum();
    assert_eq!(total, 4, "a move must not create or destroy copies");
}

#[tokio::test]
async fn test_move_more_than_present_moves_what_is_there() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 2).await;

    p.deck_mut(&deck).move_card(&"bolt".to_string(), "", "main", "side", 99)
        .await
        .unwrap();

    let cards = p.deck(&deck).cards().await.unwrap();
    assert_eq!(qty(&cards, "bolt", "main"), 0);
    assert_eq!(qty(&cards, "bolt", "side"), 2);
}

// ── build state and availability ─────────────────────────────────────────────

#[tokio::test]
async fn test_deck_meta_defaults_to_not_built() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;

    let meta = p.deck(&deck).meta().await.unwrap();
    assert!(!meta.built, "a deck with no stored row is simply not built");
    assert_eq!(meta.location, None);
}

#[tokio::test]
async fn test_deck_meta_round_trips() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Dimir Faeries").await;

    p.deck_mut(&deck).set_meta(true, Some("red deck box"), None)
        .await
        .unwrap();

    let meta = p.deck(&deck).meta().await.unwrap();
    assert!(meta.built);
    assert_eq!(meta.location.as_deref(), Some("red deck box"));
}

/// The driving use case: "Dimir Faeries is built and in my red deck box, so if I need my
/// Spellstutter Sprites for another deck, get them from there."
#[tokio::test]
async fn test_availability_points_at_the_built_deck_holding_the_card() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Dimir Faeries").await;

    add_card(&mut p, &col, &"sprite".to_string(), 4, 0).await;
    deck_add(&mut p, &deck, "sprite", "main", 4).await;
    p.deck_mut(&deck).set_meta(true, Some("red deck box"), None)
        .await
        .unwrap();

    let availability = p.card_availability(&["sprite".to_string()]).await.unwrap();
    let a = &availability[0];

    assert_eq!(a.owned, 4);
    assert_eq!(a.committed, 4);
    assert_eq!(a.free, 0);
    assert!(!a.over_committed);
    assert_eq!(a.holdings.len(), 1);
    assert_eq!(a.holdings[0].deck, "Dimir Faeries");
    assert_eq!(a.holdings[0].location.as_deref(), Some("red deck box"));
    assert_eq!(a.holdings[0].board, "main");
    assert_eq!(a.holdings[0].quantity, 4);
}

/// An unbuilt deck is only a list; it commits nothing, so the cards stay free.
#[tokio::test]
async fn test_unbuilt_decks_do_not_consume_availability() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;

    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.committed, 0);
    assert_eq!(a.free, 4);
    assert!(a.holdings.is_empty());

    // Building it commits the cards; taking it apart frees them again.
    p.deck_mut(&deck).set_meta(true, None, None).await.unwrap();
    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.committed, 4);
    assert_eq!(a.free, 0);

    p.deck_mut(&deck).set_meta(false, None, None).await.unwrap();
    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.committed, 0);
    assert_eq!(a.free, 4);
}

#[tokio::test]
async fn test_over_commitment_is_reported_not_blocked() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;

    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;
    for name in ["Burn", "Storm"] {
        let deck = new_deck(&mut p, name).await;
        deck_add(&mut p, &deck, "bolt", "main", 4).await;
        p.deck_mut(&deck).set_meta(true, Some("shelf"), None)
            .await
            .unwrap();
    }

    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.owned, 4);
    assert_eq!(a.committed, 8);
    assert_eq!(a.free, -4, "free goes negative rather than clamping");
    assert!(a.over_committed);
    assert_eq!(a.holdings.len(), 2, "both built decks are named");
}

/// Availability aggregates per card, not per finish: a foil copy in a built deck still locates
/// a playable one.
#[tokio::test]
async fn test_availability_aggregates_across_finishes() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;

    add_card(&mut p, &col, &"bolt".to_string(), 3, 1).await;
    p.deck_mut(&deck).adjust_card(&"bolt".to_string(), "foil", "main", 1, NOW, "mtg")
        .await
        .unwrap();
    p.deck_mut(&deck).set_meta(true, None, None).await.unwrap();

    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.owned, 4, "3 nonfoil + 1 foil");
    assert_eq!(a.committed, 1);
    assert_eq!(a.holdings[0].finish, "foil", "finish kept as detail");
}

#[tokio::test]
async fn test_availability_for_unknown_card_is_all_zero() {
    let p = SQLitePersistenceSystem::new(true, None).unwrap();
    let a = &p.card_availability(&["nothing".to_string()]).await.unwrap()[0];
    assert_eq!((a.owned, a.committed, a.free), (0, 0, 0));
    assert!(!a.over_committed);
}

// ── decks never disturb ownership records ────────────────────────────────────

/// Purchase history is keyed on `(collection_id, card_uuid)` with no board.
///
/// The prototype put deck membership in `cards`, which made adding a card to a sideboard trim
/// purchase history against the sideboard's quantity and destroy the mainboard purchase record.
/// Deck rows live in `deck_cards`, so that can't happen.
#[tokio::test]
async fn test_deck_activity_never_disturbs_purchase_history() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;

    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;
    record_purchase(&mut p, &col, "bolt", 4, 0, Some(2.50)).await;
    let before = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(before.len(), 1);

    // Exercise every deck path that touches this card.
    deck_add(&mut p, &deck, "bolt", "main", 4).await;
    deck_add(&mut p, &deck, "bolt", "side", 1).await;
    p.deck_mut(&deck).move_card(&"bolt".to_string(), "", "main", "side", 2)
        .await
        .unwrap();
    p.deck_mut(&deck).set_meta(true, Some("red deck box"), None)
        .await
        .unwrap();
    deck_add(&mut p, &deck, "bolt", "side", -3).await;

    let after = p.get_all_purchase_history(&col).await.unwrap();
    assert_eq!(after.len(), before.len(), "no history rows added or removed");
    assert_eq!(after[0].quantity, before[0].quantity);
    assert_eq!(after[0].price_per_unit, before[0].price_per_unit);
}

// ── cascades ─────────────────────────────────────────────────────────────────

/// `deck_cards` / `deck_meta` declare real foreign keys, so a rename cascades rather than orphaning
/// the deck's contents.
#[tokio::test]
async fn test_renaming_a_deck_carries_its_cards_and_metadata() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;
    p.deck_mut(&deck).set_meta(true, Some("red deck box"), None)
        .await
        .unwrap();

    p.rename_collection(&deck, &"Mono Red".to_string())
        .await
        .unwrap();

    let renamed = "Mono Red".to_string();
    assert_eq!(
        qty(&p.deck(&renamed).cards().await.unwrap(), "bolt", "main"),
        4
    );
    let meta = p.deck(&renamed).meta().await.unwrap();
    assert!(meta.built);
    assert_eq!(meta.location.as_deref(), Some("red deck box"));

    assert!(p.deck(&deck).cards().await.unwrap().is_empty());
    assert_eq!(
        p.get_collection_kind(&renamed).await.unwrap(),
        Some(CollectionKind::Deck),
        "a renamed deck is still a deck"
    );
}

/// Deleting a deck must not leave rows behind: an orphaned `deck_cards` row would keep claiming
/// the card was committed to a deck that no longer exists.
#[tokio::test]
async fn test_deleting_a_deck_removes_its_cards_and_metadata() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;

    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;
    p.deck_mut(&deck).set_meta(true, Some("red deck box"), None)
        .await
        .unwrap();

    p.remove_collection(&deck, None).await.unwrap();

    assert!(p.deck(&deck).cards().await.unwrap().is_empty());
    assert!(!p.deck(&deck).meta().await.unwrap().built);

    let a = &p.card_availability(&["bolt".to_string()]).await.unwrap()[0];
    assert_eq!(a.committed, 0, "a deleted deck commits nothing");
    assert_eq!(a.free, 4);
}

/// Merging only makes sense between things that record ownership, so folding a deck into a
/// collection is refused rather than silently dropping the deck's list: those rows are allocations,
/// and moving them would invent copies the owner never had.
#[tokio::test]
async fn test_merging_a_deck_into_a_collection_is_refused() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    assert!(
        p.remove_collection(&deck, Some(col.clone())).await.is_err(),
        "a deck's cards are not copies owned"
    );

    // Refused means refused: the deck is still there, contents intact.
    assert_eq!(qty(&p.deck(&deck).cards().await.unwrap(), "bolt", "main"), 4);

    let owned = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert!(owned.is_empty(), "nothing was invented in the collection");
}

/// Proves the foreign keys the cascade tests above rely on are actually enforced. `PRAGMA
/// foreign_keys` is per-connection and off by default, so a silent regression there would make
/// those tests pass for the wrong reason.
#[tokio::test]
async fn test_deck_cards_require_an_existing_collection() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();

    let result = p
        .deck_mut("no such deck")
        .adjust_card(&"bolt".to_string(), "", "main", 1, NOW, "mtg")
        .await;

    assert!(
        result.is_err(),
        "foreign keys are not being enforced; the cascade tests are vacuous"
    );
}

/// The other direction is refused for a different reason: `cards` rows written under a deck's
/// name are orphaned, since a deck's contents come from `deck_cards`.
#[tokio::test]
async fn test_merging_a_collection_into_a_deck_is_refused() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let col = new_collection(&mut p, "Binder".to_string()).await;
    let deck = new_deck(&mut p, "Burn").await;
    add_card(&mut p, &col, &"bolt".to_string(), 4, 0).await;

    let result = p.remove_collection(&col, Some(deck.clone())).await;
    assert!(result.is_err(), "a deck cannot receive owned copies");

    let owned = p
        .get_cards_in_collection_paginated(&col, CollectionCardsParams::new(0, 100))
        .await
        .unwrap();
    assert_eq!(owned.len(), 1, "the collection survives a refused merge");
}

/// Deleting a deck outright is still fine; it just takes its list with it.
#[tokio::test]
async fn test_deleting_a_deck_without_a_merge_target_succeeds() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Burn").await;
    deck_add(&mut p, &deck, "bolt", "main", 4).await;

    p.remove_collection(&deck, None).await.unwrap();

    assert!(p.deck(&deck).cards().await.unwrap().is_empty());
}

/// Mirrors `test_quantities_saturate_instead_of_breaking_the_collection` for deck rows. SQLite
/// stores the 64-bit sum happily, but every read of this column is an i32, so without the clamp
/// one overflowing row makes the whole deck unreadable, and unrepairable through the API, since
/// removing a card reads the row first.
#[tokio::test]
async fn test_deck_quantities_saturate_instead_of_breaking_the_deck() {
    let mut p = SQLitePersistenceSystem::new(true, None).unwrap();
    let deck = new_deck(&mut p, "Big").await;

    for _ in 0..2 {
        deck_add(&mut p, &deck, "bolt", "main", i32::MAX).await;
    }

    let cards = p.deck(&deck).cards().await.unwrap();
    assert_eq!(qty(&cards, "bolt", "main"), i32::MAX, "saturates rather than overflowing");

    // And the deck is still usable afterwards, which is the point.
    deck_add(&mut p, &deck, "bolt", "main", -1).await;
    assert_eq!(qty(&p.deck(&deck).cards().await.unwrap(), "bolt", "main"), i32::MAX - 1);
}
