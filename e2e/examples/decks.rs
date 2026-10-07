//! End-to-end test for decks: a collection with `kind = "deck"` whose cards
//! live in `deck_cards` rather than `cards`, partitioned by `board`.
//!
//! The property everything else rests on is that decks are *additive*: a deck
//! records what is to be played, a collection records what is physically
//! owned, and putting a card in a deck doesn't deduct it from anywhere. The
//! transfer endpoint is the deliberate exception, and says so.
//!
//! Covers:
//!   1. adding to a deck leaves the source collection's quantity untouched
//!   2. the same card on two boards is two independent rows
//!   3. boards are open-ended, not a fixed main/side pair
//!   4. moving between boards conserves copies
//!   5. a deck's cards survive a rename, and vanish with a delete
//!   6. a shared deck serves its cards, one row per board
//!   7. the collection card endpoints refuse a deck (and vice versa), so a
//!      deck can never accumulate orphaned rows in `cards`
//!   8. the collection want endpoint refuses a deck, whose wanted copies are its own rows'
//!      business rather than the cards table's
//!   9. an unbuilt deck commits nothing; building it commits its cards and
//!      says where they physically are, and dismantling it releases them
//!  10. over-commitment is reported, never refused
//!  11. build state and location survive a rename
//!  12. a deck row carries owned and wanted counts at once, and the deck's size
//!      counts the greater of them rather than their sum
//!  13. boards are free-form and user-editable: added, renamed (carrying their
//!      cards), and reordered
//!  14. a card can be transferred between a collection and a deck, and a
//!      collection converted to a deck and back
//!
//! Cross-printing matching (owning the Revised printing satisfying a deck that
//! lists the M13 one) is covered by unit tests in `server::decks` instead: the
//! bundled `data/testPrintings.db` has only one *hydratable* printing of any
//! card name, so there is nothing here to match across.
//!
//! Deploys its own server (see `e2e::harness`), on a copy of
//! `data/testPrintings.db`:
//!   cargo run --example decks

use std::collections::HashSet;
use std::num::NonZeroU32;

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};
use e2e::models::{CollectionKind, DeckCard, Transfer};

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

    // The page carries the board, so a mainboard and a sideboard copy of the same printing stay
    // apart rather than being summed into one row the reader can't take apart again.
    let distinct: HashSet<(&String, &String, &String)> =
        rows.iter().map(|c| (&c.id, &c.finish, &c.board)).collect();
    eq(
        page.total,
        distinct.len(),
        "a shared deck has one row per card, finish and board",
    )?;

    let shared = page
        .cards
        .iter()
        .find(|c| c["id"] == CARD_A && c["board"] == "main")
        .ok_or_else(|| eyre::eyre!("shared deck is missing its mainboard cards"))?;
    eq(
        shared["quantity"].as_i64(),
        Some(i64::from(qty(&rows, CARD_A, "main"))),
        "a shared row reports what its own board holds",
    )?;
    ensure(
        page.cards.iter().all(|c| c["board"].is_string()),
        "every shared deck row names the board it sits on",
    )?;
    ensure(
        shared["name"].is_string(),
        "a shared row is hydrated with its card details",
    )?;
    ok("a shared deck serves its list, boards and all");

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
        "the collection want endpoint writes to cards, so it refuses a deck",
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

    // ── 10. Only a built deck commits its cards ─────────────────────────────
    step("10. Build a deck, then take it apart");

    client.adjust_deck_card(deck2, CARD_A, "", "main", 3).await?;

    let a = availability(client, CARD_A).await?;
    eq(a.owned, 4, "owned counts the collection")?;
    eq(a.committed, 0, "an unbuilt deck commits nothing")?;
    ensure(a.holdings.is_empty(), "an unbuilt deck isn't a holding")?;
    ok("an unbuilt deck is only a list");

    client.set_deck_meta(deck2, true, Some("shelf")).await?;

    let a = availability(client, CARD_A).await?;
    eq(a.committed, 3, "a built deck commits its cards")?;
    eq(a.free, 1, "free is what's left after what's committed")?;
    let holding = a
        .holdings
        .iter()
        .find(|h| h.deck == deck2 && h.board == "main")
        .ok_or_else(|| eyre::eyre!("a built deck should appear in the holdings"))?;
    eq(
        holding.location.as_deref(),
        Some("shelf"),
        "a holding says where the deck physically is",
    )?;
    ok("building a deck commits its cards, and says where they are");

    ensure(client.deck_meta(deck2).await?.built, "build state round-trips")?;

    client.set_deck_meta(deck2, false, None).await?;
    eq(
        availability(client, CARD_A).await?.committed,
        0,
        "dismantling a deck releases its cards",
    )?;
    ok("taking a deck apart frees its cards again");

    // ── 11. Over-commitment is reported, not refused ────────────────────────
    step("11. Commit more copies than are owned");

    client.set_deck_meta(deck2, true, Some("shelf")).await?;
    client.adjust_deck_card(deck2, CARD_A, "", "side", 5).await?;

    let a = availability(client, CARD_A).await?;
    eq(a.owned, 4, "still own exactly what the collection holds")?;
    eq(a.committed, 8, "3 on the mainboard and 5 on the sideboard")?;
    ensure(a.over_committed, "over-commitment is flagged")?;
    ensure(a.free < 0, "free goes negative rather than clamping at zero")?;
    ok("over-commitment is reported rather than blocked");

    // ── 12. Build state survives a rename ───────────────────────────────────
    step("12. Rename a built deck");

    let renamed2 = format!("{deck2}-renamed");
    client.rename_collection(deck2, &renamed2).await?;

    let meta = client.deck_meta(&renamed2).await?;
    ensure(meta.built, "build state follows the rename")?;
    eq(
        meta.location.as_deref(),
        Some("shelf"),
        "so does where the deck is",
    )?;
    eq(
        availability(client, CARD_A).await?.committed,
        8,
        "a renamed deck still commits its cards",
    )?;
    client.remove_collection(&renamed2).await?;
    ok("a renamed deck keeps its build state");

    // The collection is still exactly as it was at step 1, despite everything the decks did to it.
    let owned = client.list_cards(col).await?;
    eq(
        owned.iter().find(|c| c.id == CARD_A).map(|c| c.quantity),
        Some(4),
        "the collection never moved throughout",
    )?;
    ok("ownership was never touched by any deck operation");

    step("13. Owned and wanted copies on one row");

    let wants = format!("{deck}-wants");
    client.add_deck(&wants).await?;

    eq(
        client.deck_boards(&wants).await?.iter().map(|b| b.board.as_str()).collect::<Vec<_>>(),
        vec!["Mainboard", "Sideboard"],
        "a new deck starts with the default boards",
    )?;

    // Four wanted, three to hand: four cards of the deck, not seven
    client.adjust_deck_card_counts(&wants, CARD_A, "", "Mainboard", 3, 4).await?;
    let row = client
        .deck_cards(&wants)
        .await?
        .into_iter()
        .find(|c| c.id == CARD_A)
        .ok_or_else(|| eyre::eyre!("the card wasn't on the deck"))?;
    eq(row.quantity, 3, "owned copies")?;
    eq(row.want_quantity, 4, "wanted copies")?;
    eq(row.slots(), 4, "the greater of the two, not the sum")?;
    eq(
        client.deck_card_count(&wants).await?,
        4,
        "and the deck's size counts it once",
    )?;
    ok("a deck row carries both counts, and the size takes the greater");

    step("14. Free-form, editable boards");

    client.add_deck_board(&wants, "Mainboard:Lands").await?;
    client.adjust_deck_card(&wants, CARD_B, "", "Mainboard:Lands", 2).await?;

    let boards = client.rename_deck_board(&wants, "Mainboard:Lands", "Manabase").await?;
    eq(
        boards.iter().map(|b| b.board.as_str()).collect::<Vec<_>>(),
        vec!["Mainboard", "Sideboard", "Manabase"],
        "the renamed board keeps its place",
    )?;
    eq(
        client
            .deck_cards(&wants)
            .await?
            .iter()
            .find(|c| c.id == CARD_B)
            .map(|c| c.board.as_str()),
        Some("Manabase"),
        "and takes its cards with it",
    )?;

    ensure(
        client.rename_deck_board(&wants, "Manabase", "Sideboard").await.is_err(),
        "renaming onto a name the deck already uses is refused rather than merged",
    )?;

    let boards = client.reorder_deck_boards(&wants, &["Manabase"]).await?;
    eq(
        boards.iter().map(|b| b.board.as_str()).collect::<Vec<_>>(),
        vec!["Manabase", "Mainboard", "Sideboard"],
        "boards left out of the order keep their place after the ones in it",
    )?;
    ok("boards are added, renamed, and reordered freely");

    step("15. Move a card between a collection and a deck");

    let two = NonZeroU32::new(2).expect("2 is not zero");
    let before = card_quantity(client, col, CARD_A).await?;
    let moved = client
        .transfer_deck_card(&wants, col, CARD_A, "", "Mainboard", Transfer::IntoDeck(two))
        .await?;
    eq(moved, 2, "two copies moved")?;
    eq(
        card_quantity(client, col, CARD_A).await?,
        before - 2,
        "a transfer deducts from the collection, unlike an allocation",
    )?;

    let moved = client
        .transfer_deck_card(&wants, col, CARD_A, "", "Mainboard", Transfer::OutOfDeck(two))
        .await?;
    eq(moved, 2, "and they come back")?;
    eq(
        card_quantity(client, col, CARD_A).await?,
        before,
        "leaving the collection where it started",
    )?;
    ok("cards move between a collection and a deck, both ways");

    step("16. Convert a collection to a deck and back");

    let convert = format!("e2e-decks-convert-{}", std::process::id());
    client.add_collection(&convert).await?;
    client.add_cards(&convert, CARD_A, "", 4, None).await?;

    client.set_collection_kind(&convert, CollectionKind::Deck).await?;
    eq(
        client
            .deck_cards(&convert)
            .await?
            .iter()
            .find(|c| c.id == CARD_A)
            .map(|c| (c.quantity, c.board.as_str())),
        Some((4, "Mainboard")),
        "the cards come across onto the first default board",
    )?;

    client.set_collection_kind(&convert, CollectionKind::Collection).await?;
    eq(
        card_quantity(client, &convert, CARD_A).await?,
        4,
        "and back again",
    )?;
    client.remove_collection(&convert).await?;
    client.remove_collection(&wants).await?;
    ok("a collection converts to a deck and back, carrying its cards");

    println!("\n✓ All assertions passed");
    Ok(())
}

/// How many copies of a card a collection holds, or 0 if it holds none.
async fn card_quantity(client: &GathersClient, collection: &str, id: &str) -> eyre::Result<i32> {
    Ok(client
        .list_cards(collection)
        .await?
        .iter()
        .find(|c| c.id == id && c.finish.is_empty())
        .map(|c| c.quantity)
        .unwrap_or(0))
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

/// The availability entry for one card, which is always exactly one row per requested id.
async fn availability(
    client: &GathersClient,
    id: &str,
) -> eyre::Result<e2e::models::CardAvailability> {
    client
        .card_availability(&[id])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| eyre::eyre!("no availability returned for '{id}'"))
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
