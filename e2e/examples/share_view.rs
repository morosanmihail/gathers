//! End-to-end test for the shareable, read-only collection view:
//!   /api/share/collection/{id} should return full card data merged with
//!   collection metadata, paginated, matching what /cards/{id}/list +
//!   per-provider card lookups would otherwise take multiple requests to get.
//!
//! Run against a live server:
//!   cargo run --example share_view
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example share_view

use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Mutilate — M13 #102
const CARD_B: &str = "c83a7592-5879-5d52-b27c-e866597b389f";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs shareable collection view e2e ===");
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
    let col = format!("e2e-share-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);

    let result = run(&client, &col).await;

    drop(guard);

    result
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    // ── 1. Empty collection ───────────────────────────────────────────────────
    step("1. Shared view of a freshly created (empty) collection");

    client.add_collection(col).await?;

    let page = client.public_cards(col, 0, 1000).await?;
    eq(page.total, 0, "empty collection: total = 0")?;
    ensure(page.cards.is_empty(), "empty collection: no cards")?;
    ok("empty collection returns total=0, no cards");

    // ── 2. Add cards, verify full data is merged in a single request ─────────
    step("2. Add cards, verify merged card data");

    client.add_cards(col, CARD_A, 4, 0, None).await?;
    client.add_cards(col, CARD_B, 0, 2, None).await?;

    let page = client.public_cards(col, 0, 1000).await?;
    eq(page.total, 2, "2 distinct cards")?;
    eq(page.cards.len(), 2, "page returns both cards in one request")?;

    let a = find(&page.cards, CARD_A)?;
    eq(field_i64(a, "quantity"), 4, "card A quantity")?;
    eq(field_i64(a, "foilQuantity"), 0, "card A foil qty")?;
    ensure(
        !field_str(a, "name").is_empty(),
        "card A has a non-empty name (full card data was merged in)",
    )?;
    ensure(
        !field_str(a, "provider").is_empty(),
        "card A has a provider",
    )?;

    let b = find(&page.cards, CARD_B)?;
    eq(field_i64(b, "quantity"), 0, "card B quantity")?;
    eq(field_i64(b, "foilQuantity"), 2, "card B foil qty")?;
    ensure(!field_str(b, "name").is_empty(), "card B has a non-empty name")?;
    ok("both cards present with quantities + full card details merged in");

    // ── 3. Pagination: two pages of size 1 cover both cards, no overlap ──────
    step("3. Pagination");

    let page0 = client.public_cards(col, 0, 1).await?;
    eq(page0.cards.len(), 1, "page 0: 1 card")?;
    eq(page0.total, 2, "page 0: total still 2")?;

    let page1 = client.public_cards(col, 1, 1).await?;
    eq(page1.cards.len(), 1, "page 1: 1 card")?;
    eq(page1.total, 2, "page 1: total still 2")?;

    let id0 = field_str(&page0.cards[0], "id");
    let id1 = field_str(&page1.cards[0], "id");
    ensure(id0 != id1, "the two pages return different cards")?;
    let ids: std::collections::HashSet<&str> = [CARD_A, CARD_B].into_iter().collect();
    ensure(ids.contains(id0.as_str()), "page 0 card is one of the two added")?;
    ensure(ids.contains(id1.as_str()), "page 1 card is one of the two added")?;
    ok("pagination covers exactly the 2 cards with no overlap");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn find<'a>(cards: &'a [serde_json::Value], id: &str) -> eyre::Result<&'a serde_json::Value> {
    cards
        .iter()
        .find(|c| field_str(c, "id") == id)
        .ok_or_else(|| eyre::eyre!("card '{id}' not found in shared view"))
}

fn field_str(card: &serde_json::Value, key: &str) -> String {
    card.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}

fn field_i64(card: &serde_json::Value, key: &str) -> i64 {
    card.get(key).and_then(|v| v.as_i64()).unwrap_or_default()
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
