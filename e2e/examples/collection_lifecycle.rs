//! End-to-end test for the full collection lifecycle:
//!   add cards → verify → remove cards → verify history trimmed
//!   → move cards → verify history transferred → clean up
//!
//! Run against a live server:
//!   cargo run --example collection_lifecycle
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example collection_lifecycle

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

    println!("=== GatheRs collection lifecycle e2e ===");
    println!("Server: {url}");
    println!();

    // Unique per run: pid keeps parallel runs distinct, timestamp avoids reuse.
    let tag = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let col_src = format!("e2e-src-{tag}");
    let col_dst = format!("e2e-dst-{tag}");

    // Register for cleanup before creating — guard's Drop always runs even if
    // run() returns Err or we panic.
    let mut guard = CollectionGuard::new(&client);
    guard.register(&col_src);
    guard.register(&col_dst);

    let result = run(&client, &col_src, &col_dst).await;

    // Guard drops here (or at end of scope), cleaning up both collections.
    drop(guard);

    result
}

async fn run(client: &GathersClient, col_src: &str, col_dst: &str) -> eyre::Result<()> {
    // ── 1. Create two collections ─────────────────────────────────────────────
    step("1. Create collections");

    client.add_collection(col_src).await?;
    client.add_collection(col_dst).await?;

    let collections = client.list_collections().await?;
    ensure(collections.iter().any(|c| c.id == col_src), "src collection not listed")?;
    ensure(collections.iter().any(|c| c.id == col_dst), "dst collection not listed")?;
    ok("both collections created");

    // ── 2. Add cards with purchase prices ────────────────────────────────────
    step("2. Add cards to source collection");

    // card A: 4 copies in two batches at different prices
    client.add_cards(col_src, CARD_A, 2, 0, Some(3.00)).await?;
    client.add_cards(col_src, CARD_A, 2, 0, Some(5.00)).await?;
    // card B: 2 foil at $10 each
    client.add_cards(col_src, CARD_B, 0, 2, Some(10.00)).await?;

    let cards = client.list_cards(col_src).await?;
    let a = find_card(&cards, CARD_A)?;
    eq(a.quantity, 4, "card A quantity")?;
    eq(a.foil_quantity, 0, "card A foil")?;
    let b = find_card(&cards, CARD_B)?;
    eq(b.quantity, 0, "card B quantity")?;
    eq(b.foil_quantity, 2, "card B foil")?;
    ok("4× card A + 2× foil card B in collection");

    let count = client.card_count(col_src).await?;
    eq(count, 2, "card count = 2 distinct cards")?;
    ok("card_count returns 2");

    // ── 3. Verify purchase history recorded ──────────────────────────────────
    step("3. Verify purchase history");

    let hist_a = client.purchase_history(col_src, CARD_A).await?;
    eq(hist_a.entries.len(), 2, "card A: 2 purchase entries")?;
    let total_qty_a: i32 = hist_a.entries.iter().map(|e| e.quantity).sum();
    eq(total_qty_a, 4, "card A: total purchased qty = 4")?;
    ok("card A: 2 purchase history entries summing to 4");

    let hist_b = client.purchase_history(col_src, CARD_B).await?;
    eq(hist_b.entries.len(), 1, "card B: 1 purchase entry")?;
    eq(hist_b.entries[0].foil_quantity, 2, "card B: foil qty = 2")?;
    ok("card B: 1 foil purchase history entry");

    let all_hist = client.all_purchase_history(col_src).await?;
    eq(all_hist.entries.len(), 3, "all history: 3 entries total")?;
    ok("all_purchase_history returns 3 entries");

    // ── 4. Remove cards → purchase history trimmed ───────────────────────────
    step("4. Remove 2× card A → history trimmed");

    // Remove 2 copies; 2 remain. History: [qty=2 @ $3, qty=2 @ $5].
    // Cheapest ($3) entry should be removed, $5 entry kept.
    client.remove_cards(col_src, CARD_A, 2, 0).await?;

    let cards = client.list_cards(col_src).await?;
    let a = find_card(&cards, CARD_A)?;
    eq(a.quantity, 2, "card A quantity after remove")?;

    let hist_a = client.purchase_history(col_src, CARD_A).await?;
    let total_qty_a: i32 = hist_a.entries.iter().map(|e| e.quantity).sum();
    eq(total_qty_a, 2, "card A: history qty trimmed to 2")?;
    let remaining_price = hist_a
        .entries
        .iter()
        .filter_map(|e| e.normal_price_per_unit)
        .max_by(|a, b| a.partial_cmp(b).unwrap());
    ensure(
        remaining_price == Some(5.00),
        &format!("cheapest entry removed, $5 entry kept (got {remaining_price:?})"),
    )?;
    ok("2× removed, cheapest history entry trimmed");

    // ── 5. Remove all of card A → history fully cleared ──────────────────────
    step("5. Remove remaining 2× card A");

    client.remove_cards(col_src, CARD_A, 2, 0).await?;

    let hist_a = client.purchase_history(col_src, CARD_A).await?;
    ensure(hist_a.entries.is_empty(), "card A history empty after full removal")?;
    ok("card A history fully cleared");

    let cards = client.list_cards(col_src).await?;
    ensure(
        !cards.iter().any(|c| c.id == CARD_A),
        "card A no longer in collection",
    )?;
    ok("card A removed from collection");

    // ── 6. Re-add card A, then move both cards to dst ─────────────────────────
    step("6. Re-add card A with price, then move both cards to dst");

    client.add_cards(col_src, CARD_A, 3, 0, Some(7.00)).await?;

    let cards = client.list_cards(col_src).await?;
    let cards_to_move: Vec<CollectionCard> = cards
        .into_iter()
        .filter(|c| c.id == CARD_A || c.id == CARD_B)
        .collect();
    eq(cards_to_move.len(), 2, "2 distinct cards to move")?;

    client.move_cards(col_dst, &cards_to_move).await?;

    // ── 7. Verify source collection is empty ─────────────────────────────────
    step("7. Verify source collection emptied");

    let src_cards = client.list_cards(col_src).await?;
    ensure(src_cards.is_empty(), "source collection empty after move")?;
    ok("source collection empty");

    let src_hist = client.all_purchase_history(col_src).await?;
    ensure(
        src_hist.entries.is_empty(),
        "source purchase history empty after full move",
    )?;
    ok("source purchase history empty");

    // ── 8. Verify destination received cards + history ────────────────────────
    step("8. Verify destination received cards and purchase history");

    let dst_cards = client.list_cards(col_dst).await?;
    let a_dst = find_card(&dst_cards, CARD_A)?;
    eq(a_dst.quantity, 3, "dst: card A quantity = 3")?;
    let b_dst = find_card(&dst_cards, CARD_B)?;
    eq(b_dst.foil_quantity, 2, "dst: card B foil = 2")?;
    ok("destination has correct quantities");

    let hist_a_dst = client.purchase_history(col_dst, CARD_A).await?;
    let qty_a_dst: i32 = hist_a_dst.entries.iter().map(|e| e.quantity).sum();
    eq(qty_a_dst, 3, "dst: card A history sums to 3")?;
    ok("card A purchase history transferred to destination");

    let hist_b_dst = client.purchase_history(col_dst, CARD_B).await?;
    let foil_b_dst: i32 = hist_b_dst.entries.iter().map(|e| e.foil_quantity).sum();
    eq(foil_b_dst, 2, "dst: card B foil history sums to 2")?;
    ok("card B foil purchase history transferred to destination");

    let dst_all = client.all_purchase_history(col_dst).await?;
    eq(dst_all.entries.len(), 2, "dst: 2 total history entries (A + B)")?;
    ok("all_purchase_history shows 2 entries in destination");

    // ── 9. Partial move from dst back to src ──────────────────────────────────
    step("9. Partial move: 1× card A back to src");

    let dst_cards = client.list_cards(col_dst).await?;
    let a_in_dst = find_card(&dst_cards, CARD_A)?.clone();

    client
        .move_cards(
            col_src,
            &[CollectionCard {
                id: a_in_dst.id.clone(),
                quantity: 1,
                foil_quantity: 0,
                collection_id: col_dst.to_string(),
                time_added: a_in_dst.time_added.clone(),
                provider: a_in_dst.provider.clone(),
            }],
        )
        .await?;

    let dst_cards = client.list_cards(col_dst).await?;
    let a_dst = find_card(&dst_cards, CARD_A)?;
    eq(a_dst.quantity, 2, "dst: card A quantity = 2 after partial move back")?;

    let hist_a_dst = client.purchase_history(col_dst, CARD_A).await?;
    let qty_a_dst: i32 = hist_a_dst.entries.iter().map(|e| e.quantity).sum();
    eq(qty_a_dst, 2, "dst: card A history trimmed to 2")?;
    ok("dst card A history trimmed to 2 after partial move");

    let hist_a_src = client.purchase_history(col_src, CARD_A).await?;
    let qty_a_src: i32 = hist_a_src.entries.iter().map(|e| e.quantity).sum();
    eq(qty_a_src, 1, "src: received 1 history entry from move")?;
    ok("1 history entry transferred back to src");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn find_card<'a>(cards: &'a [CollectionCard], id: &str) -> eyre::Result<&'a CollectionCard> {
    cards
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| eyre::eyre!("card '{id}' not found in collection"))
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
