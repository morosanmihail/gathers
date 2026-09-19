//! End-to-end test for decks: a collection with `kind = "deck"` whose cards
//! live in `deck_cards` rather than `cards`, partitioned by `board`.
//!
//! The property everything else rests on is that decks are *additive*: a deck
//! records what is to be played, a collection records what is physically
//! owned, and putting a card in a deck doesn't deduct it from anywhere.
//!
//! Covers:
//!   1. adding to a deck leaves the source collection's quantity untouched
//!   2. the same card on two boards is two independent rows
//!   3. boards are open-ended, not a fixed main/side pair
//!   4. moving between boards conserves copies
//!   5. a deck's cards survive a rename, and vanish with a delete
//!   6. a shared deck serves its cards rather than an empty page
//!   7. the collection card endpoints refuse a deck (and vice versa), so a
//!      deck can never accumulate orphaned rows in `cards`
//!   8. adjusting a want quantity on a deck is an error, since deck rows
//!      have no want quantity at all
//!
//! Deploys its own server (see `e2e::harness`), on a copy of
//! `data/testPrintings.db`:
//!   cargo run --example decks

use std::collections::HashSet;

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};
use e2e::models::{CollectionKind, DeckCard};

// War Priest of Thune, M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Goblin King, 3ED #155
const CARD_B: &str = "0001e0d0-2dcd-5640-aadc-a84765cf5fc9";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("decks")?;
    let setup = ServerSetup::mtg(&harness)?;
    let client = harness.start_server(&setup).await?;

    let col = "e2e-decks-binder";
    let deck = "e2e-decks-burn";
    let deck2 = "e2e-decks-storm";
    let renamed = "e2e-decks-mono-red";

    let result = run(&client, col, deck, deck2, renamed).await;
    harness.conclude(result)
}

async fn run(
    client: &GathersClient,
    col: &str,
    deck: &str,
    deck2: &str,
    renamed: &str,
) -> eyre::Result<()> {
    client.add_collection(col).await?;
    client.add_deck(deck).await?;

    // ── 1. Adding to a deck doesn't touch the collection ────────────────────
    step("1. Own 4 copies, then put 4 in a deck");

    client.add_cards(col, CARD_A, "", 4, None).await?;
    client.adjust_deck_card(deck, CARD_A, "", "main", 4).await?;

    let owned = client.list_cards(col).await?;
    eq(
        owned.iter().find(|c| c.id == CARD_A).map(|c| c.quantity),
        Some(4),
        "collection quantity is unchanged by the deck",
    )?;
    eq(qty(&client.deck_cards(deck).await?, CARD_A, "main"), 4, "deck holds 4 on the mainboard")?;
    ok("a deck records intent, not ownership");

    // A deck is listed *as a deck*, which is how a client knows to send its card edits to /api/deck
    // rather than /api/collection.
    let listed = client.list_collections().await?;
    let entry = listed
        .iter()
        .find(|c| c.id == deck)
        .ok_or_else(|| eyre::eyre!("the deck is missing from the collection list"))?;
    eq(entry.kind, Some(CollectionKind::Deck), "listed with kind = deck")?;
    let plain = listed
        .iter()
        .find(|c| c.id == col)
        .ok_or_else(|| eyre::eyre!("the collection is missing from the list"))?;
    eq(plain.kind, Some(CollectionKind::Collection), "a collection is still a collection")?;
    ok("decks are listed alongside collections, and say which they are");

    // ── 2. Two boards, two independent rows ─────────────────────────────────
    step("2. Add the same card to the sideboard");

    client.adjust_deck_card(deck, CARD_A, "", "side", 2).await?;

    let cards = client.deck_cards(deck).await?;
    eq(cards.iter().filter(|c| c.id == CARD_A).count(), 2, "one card, two board rows")?;
    eq(qty(&cards, CARD_A, "main"), 4, "mainboard untouched")?;
    eq(qty(&cards, CARD_A, "side"), 2, "sideboard tracked separately")?;
    ok("boards are independent rows");

    // ── 3. Boards are open-ended ────────────────────────────────────────────
    step("3. Use a board that isn't main or side");

    // Riftbound decks have a legend, runes, and battlefields; Commander adds a command zone. The
    // server stores whatever the client sends.
    client.adjust_deck_card(deck, CARD_B, "", "command", 1).await?;
    eq(qty(&client.deck_cards(deck).await?, CARD_B, "command"), 1, "arbitrary board accepted")?;
    ok("board is an open string, not a fixed pair");

    // Copies rather than distinct cards, and answered by the server: this is what sizes a deck on
    // the overview, so downloading the card list to add it up would defeat the point.
    eq(client.deck_card_count(deck).await?, 7, "4 main + 2 side + 1 command")?;
    ok("count sums copies across boards");

    // ── 4. Moving between boards conserves copies ───────────────────────────
    step("4. Move 1 copy from mainboard to sideboard");

    client.move_deck_card(deck, CARD_A, "", "main", "side", 1).await?;

    let cards = client.deck_cards(deck).await?;
    eq(qty(&cards, CARD_A, "main"), 3, "mainboard down one")?;
    eq(qty(&cards, CARD_A, "side"), 3, "sideboard up one")?;
    eq(
        qty(&cards, CARD_A, "main") + qty(&cards, CARD_A, "side"),
        6,
        "a move neither creates nor destroys copies",
    )?;
    ok("moving between boards conserves copies");

    // ── 5. Rename carries the deck; delete takes it away ────────────────────
    step("5. Rename the deck");

    client.rename_collection(deck, renamed).await?;

    let cards = client.deck_cards(renamed).await?;
    eq(qty(&cards, CARD_A, "main"), 3, "cards follow the rename")?;
    eq(qty(&cards, CARD_A, "side"), 3, "every board follows the rename")?;
    ok("a renamed deck keeps its cards");

    // ── 6. A shared deck serves its cards ───────────────────────────────────
    step("6. Share the deck");

    // A deck's cards live in `deck_cards`, so the collection query behind /api/share reports an
    // empty page unless the deck path is taken. A shared deck that renders as
    // nothing is worse than one that refuses to be shared at all.
    let rows = client.deck_cards(renamed).await?;
    let link = client.create_share_link(renamed).await?;
    let page = client.public_cards(&link.token, 0, 100).await?;

    // Boards are summed away rather than carried: the public page has no board column, and one
    // row per board would put two entries of the same printing on the same key.
    let distinct: HashSet<(&String, &String)> = rows.iter().map(|c| (&c.id, &c.finish)).collect();
    eq(
        page.total,
        distinct.len(),
        "a shared deck has one row per card and finish",
    )?;

    let shared = page
        .cards
        .iter()
        .find(|c| c["id"] == CARD_A)
        .ok_or_else(|| eyre::eyre!("shared deck is missing its cards"))?;
    eq(
        shared["quantity"].as_i64(),
        Some(i64::from(qty(&rows, CARD_A, "main") + qty(&rows, CARD_A, "side"))),
        "a shared row sums the card across every board it sits on",
    )?;
    ensure(
        shared["name"].is_string(),
        "a shared row is hydrated with its card details",
    )?;
    ok("a shared deck serves its list");

    step("7. Delete the deck");

    client.remove_collection(renamed).await?;
    ensure(
        client.deck_cards(renamed).await.is_err(),
        "a deleted deck is gone, not merely empty",
    )?;
    ok("deleting a deck takes its list with it");

    // ── 8. The two card APIs don't cross ────────────────────────────────────
    step("8. Collection endpoints refuse a deck");

    client.add_deck(deck2).await?;

    // Without this guard a deck name would write a row into `cards` that nothing ever reads back,
    // since a deck's cards come from `deck_cards`.
    ensure(
        client.add_cards(deck2, CARD_B, "", 1, None).await.is_err(),
        "adding through the collection endpoint is refused for a deck",
    )?;
    ensure(
        client.remove_cards(deck2, CARD_B, "", 1).await.is_err(),
        "removing through the collection endpoint is refused for a deck",
    )?;
    // The worst of the bunch: `move_to` writes rows keyed by the destination name, so a deck as
    // destination would take the cards out of the source collection and put them somewhere nothing
    // reads.
    let owned_before = client.list_cards(col).await?;
    ensure(
        client.move_cards(deck2, &owned_before).await.is_err(),
        "moving cards into a deck is refused",
    )?;
    let owned_after = client.list_cards(col).await?;
    eq(
        owned_after.len(),
        owned_before.len(),
        "a refused move leaves the source collection untouched",
    )?;

    ensure(
        client.adjust_want(deck2, CARD_B, 1).await.is_err(),
        "a deck has no want quantity, so adjusting one is an error",
    )?;
    ok("a deck can't accumulate rows in `cards`");

    step("9. Deck endpoints refuse a collection");

    ensure(
        client.adjust_deck_card(col, CARD_B, "", "main", 1).await.is_err(),
        "the deck endpoint is refused for a plain collection",
    )?;
    ensure(
        client.deck_cards(col).await.is_err(),
        "listing deck cards is refused for a plain collection",
    )?;
    ok("the two card APIs don't cross");

    // The collection is still exactly as it was at step 1, despite everything the decks did to it.
    let owned = client.list_cards(col).await?;
    eq(
        owned.iter().find(|c| c.id == CARD_A).map(|c| c.quantity),
        Some(4),
        "the collection never moved throughout",
    )?;
    ok("ownership was never touched by any deck operation");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Quantity of `id` on `board`, or 0 when the deck has no such row. Those are the same thing
/// as far as the deck is concerned, since rows that reach zero are deleted.
fn qty(cards: &[DeckCard], id: &str, board: &str) -> i32 {
    cards
        .iter()
        .find(|c| c.id == id && c.board == board)
        .map(|c| c.quantity)
        .unwrap_or(0)
}

fn ensure(cond: bool, msg: &str) -> eyre::Result<()> {
    if cond {
        Ok(())
    } else {
        Err(eyre::eyre!("assertion failed: {msg}"))
    }
}

fn eq<T: PartialEq + std::fmt::Debug>(got: T, expected: T, label: &str) -> eyre::Result<()> {
    if got == expected {
        Ok(())
    } else {
        Err(eyre::eyre!("{label}: expected {expected:?}, got {got:?}"))
    }
}

fn step(label: &str) {
    println!("\n[{label}]");
}

fn ok(msg: &str) {
    println!("  ✓ {msg}");
}
