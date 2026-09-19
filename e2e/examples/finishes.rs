//! End-to-end test for per-finish tracking of a card in a collection:
//!   `models::CollectionCard::finish` — `""` (the default/primary finish),
//!   MTG's `"foil"`/`"etched"`, a Pokemon variant, or any other label —
//!   makes a card's distinct printings separate `(uuid, finish, collection)`
//!   rows rather than a single row with a separate foil counter.
//!
//! Covers:
//!   1. adding two finishes of the same card as independent rows
//!   2. removing one finish leaves the other untouched
//!   3. removing/un-wanting a finish that was never added is a no-op, not a
//!      negative-quantity "ghost" row (regression test — see
//!      persistence/src/sqlite/cards.rs `add_cards`)
//!   4. the server does not validate `finish` against the card's own catalog
//!      finishes — an arbitrary finish string is accepted and tracked like
//!      any other (that validation, if wanted, belongs to the client — see
//!      webui2/src/lib/types.ts `catalogFinishes`)
//!   5. want_quantity always lives on the default (`""`) finish row,
//!      regardless of which finish(es) are actually owned
//!   6. quantity floors at 0 per finish independently
//!   7. purchase history is tracked and trimmed per finish independently
//!
//! Run against a live server:
//!   cargo run --example finishes
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example finishes

use e2e::{CollectionGuard, GathersClient};
use e2e::models::CollectionCard;

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Mutilate — M13 #102
const CARD_B: &str = "c83a7592-5879-5d52-b27c-e866597b389f";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs finishes e2e ===");
    println!("Server: {url}");
    println!();

    let tag = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let col = format!("e2e-finishes-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);

    let result = run(&client, &col).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    client.add_collection(col).await?;

    // ── 1. Two finishes of the same card are independent rows ───────────────
    step("1. Add default + foil finish of the same card");

    client.add_cards(col, CARD_A, "", 3, None).await?;
    client.add_cards(col, CARD_A, "foil", 2, None).await?;

    let cards = client.list_cards(col).await?;
    eq(cards.iter().filter(|c| c.id == CARD_A).count(), 2, "card A has 2 distinct finish rows")?;
    eq(find_finish(&cards, CARD_A, "")?.quantity, 3, "default finish quantity")?;
    eq(find_finish(&cards, CARD_A, "foil")?.quantity, 2, "foil finish quantity")?;

    let count = client.card_count(col).await?;
    eq(count, 2, "card_count counts (uuid, finish) rows, not distinct cards")?;
    ok("default and foil tracked as independent rows");

    // ── 2. Removing one finish leaves the other untouched ───────────────────
    step("2. Remove 1× foil — default finish unaffected");

    client.remove_cards(col, CARD_A, "foil", 1).await?;

    let cards = client.list_cards(col).await?;
    eq(find_finish(&cards, CARD_A, "")?.quantity, 3, "default finish still 3")?;
    eq(find_finish(&cards, CARD_A, "foil")?.quantity, 1, "foil finish reduced to 1")?;
    ok("removing one finish doesn't touch the other");

    // ── 3. Removing/un-wanting a finish never added is a no-op ──────────────
    step("3. Remove a finish that was never added — no ghost row");

    // Card B has never been added to this collection at all yet.
    client.remove_cards(col, CARD_B, "etched", 5).await?;
    let cards = client.list_cards(col).await?;
    ensure(
        !cards.iter().any(|c| c.id == CARD_B),
        "removing a never-added finish creates no row at all",
    )?;
    ok("remove on a never-added finish is a no-op");

    // Same for want: un-wanting a card that was never wanted.
    let wanted = client.adjust_want(col, CARD_B, -7).await?;
    eq(wanted.want_quantity, 0, "want floors at 0 even starting from nothing")?;
    let cards = client.list_cards(col).await?;
    ensure(
        !cards.iter().any(|c| c.id == CARD_B),
        "adjusting want negative on a never-wanted card creates no row",
    )?;
    ok("un-wanting a never-wanted card is a no-op, no ghost row");

    // ── 4. An arbitrary/unknown finish string is accepted, not rejected ─────
    step("4. A finish not in the card's own catalog is accepted as-is");

    // The server tracks whatever finish label it's given — it has no notion
    // of "valid" finishes for a card (that's a client/UI concern). This
    // isn't a typo-guard: it's deliberately permissive so any game's own
    // vocabulary (a Pokemon variant, a made-up label, etc.) round-trips.
    client.add_cards(col, CARD_B, "glittery-holo-doesnt-really-exist", 1, None).await?;
    let cards = client.list_cards(col).await?;
    let bogus = find_finish(&cards, CARD_B, "glittery-holo-doesnt-really-exist")?;
    eq(bogus.quantity, 1, "unrecognized finish string is stored and returned exactly")?;
    ok("unknown finish label accepted and tracked like any other");

    // Clean up before the next sections so B doesn't interfere with them.
    client.remove_cards(col, CARD_B, "glittery-holo-doesnt-really-exist", 1).await?;

    // ── 5. want_quantity always lives on the default finish row ─────────────
    step("5. Wanting a card owned only in foil creates a separate default-finish row");

    // Card B is currently owned in no finish. Own it only as foil, then want it.
    client.add_cards(col, CARD_B, "foil", 1, None).await?;
    client.adjust_want(col, CARD_B, 5).await?;

    let cards = client.list_cards(col).await?;
    let b_foil = find_finish(&cards, CARD_B, "foil")?;
    eq(b_foil.quantity, 1, "foil quantity unaffected by want")?;
    eq(b_foil.want_quantity, 0, "want isn't tracked on the foil row")?;
    let b_default = find_finish(&cards, CARD_B, "")?;
    eq(b_default.quantity, 0, "default-finish row owns nothing")?;
    eq(b_default.want_quantity, 5, "want lives on the default-finish row")?;
    ok("want_quantity tracked independently of which finish is owned");

    // ── 6. Quantity floors at 0 per finish, independently ────────────────────
    step("6. Over-removing a finish floors at 0 without touching a sibling finish");

    client.remove_cards(col, CARD_B, "foil", 99).await?;
    let cards = client.list_cards(col).await?;
    ensure(
        find_finish(&cards, CARD_B, "foil").is_err(),
        "foil row purged once its quantity (and want) both reach 0",
    )?;
    let b_default = find_finish(&cards, CARD_B, "")?;
    eq(b_default.want_quantity, 5, "default-finish want_quantity untouched by foil over-removal")?;
    ok("over-removing one finish floors at 0 and doesn't affect another finish's row");

    // ── 7. Purchase history is tracked and trimmed per finish ────────────────
    step("7. Purchase history keyed by finish, trimmed independently");

    // Card A currently owns 3 default (unpriced, from step 1) + 1 foil
    // (unpriced net of step 1's 2 minus step 2's removal of 1). Add priced
    // copies of both finishes — only priced additions create purchase
    // history rows, so history should show exactly these quantities.
    client.add_cards(col, CARD_A, "", 2, Some(3.00)).await?; // default finish: 3+2=5 owned
    client.add_cards(col, CARD_A, "foil", 4, Some(20.00)).await?; // foil: 1+4=5 owned

    let hist = client.purchase_history(col, CARD_A).await?;
    let default_qty: i32 = hist.entries.iter().filter(|e| e.finish.is_empty()).map(|e| e.quantity).sum();
    let foil_qty: i32 = hist.entries.iter().filter(|e| e.finish == "foil").map(|e| e.quantity).sum();
    eq(default_qty, 2, "default-finish purchase history qty matches its own priced addition")?;
    eq(foil_qty, 4, "foil purchase history qty matches its own priced addition")?;
    ok("purchase history entries are tagged with the finish they belong to");

    // Removing all 5 foil copies should trim only foil history entries.
    client.remove_cards(col, CARD_A, "foil", 5).await?;
    let hist_after = client.purchase_history(col, CARD_A).await?;
    ensure(
        hist_after.entries.iter().all(|e| e.finish.is_empty()),
        "foil purchase history fully trimmed, default-finish history untouched",
    )?;
    let default_qty_after: i32 = hist_after.entries.iter().map(|e| e.quantity).sum();
    eq(default_qty_after, 2, "default-finish purchase history unaffected by removing all foil")?;
    ok("removing one finish's copies trims only that finish's purchase history");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn find_finish<'a>(cards: &'a [CollectionCard], id: &str, finish: &str) -> eyre::Result<&'a CollectionCard> {
    cards
        .iter()
        .find(|c| c.id == id && c.finish == finish)
        .ok_or_else(|| eyre::eyre!("card '{id}' (finish {finish:?}) not found in collection"))
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
