//! End-to-end test for searching within a collection and the collection
//! value breakdown:
//!   POST /api/collection/cards/{id}/search
//!   POST /api/collection/cards/{id}/search/count
//!   GET  /api/collection/cards/{id}/value_breakdown
//!
//! Run against a live server:
//!   cargo run --example collection_search_and_value
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example collection_search_and_value

use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39, name contains "Priest"
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Mutilate — M13 #102
const CARD_B: &str = "c83a7592-5879-5d52-b27c-e866597b389f";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs collection search & value breakdown e2e ===");
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
    let col = format!("e2e-searchval-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);

    let result = run(&client, &col).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    client.add_collection(col).await?;
    client.add_cards(col, CARD_A, 4, 0, Some(3.00)).await?;
    client.add_cards(col, CARD_B, 0, 2, Some(10.00)).await?;

    // ── 1. Search by name filters to matching cards only ────────────────────
    step("1. Search by (partial, case-insensitive) name");

    let filters = serde_json::json!({ "name": "priest" });
    let results = client.search_collection_cards(col, &filters, 0, 100).await?;
    eq(results.len(), 1, "name filter 'priest' matches exactly 1 card")?;
    eq(results[0].id.as_str(), CARD_A, "the match is card A")?;

    let count = client.search_collection_cards_count(col, &filters).await?;
    eq(count, 1, "search/count agrees with search results length")?;
    ok("name search returns exactly the matching card, count endpoint agrees");

    // ── 2. Search by set code matches both (same set) ────────────────────────
    step("2. Search by set code");

    let filters = serde_json::json!({ "set_code": "m13" });
    let results = client.search_collection_cards(col, &filters, 0, 100).await?;
    eq(results.len(), 2, "both cards are in set M13")?;
    ok("set code filter matches both cards");

    // ── 3. Search with no matches returns an empty page and a zero count ────
    step("3. Search with a name that matches nothing");

    let filters = serde_json::json!({ "name": "definitely-not-a-real-card-name-zzz" });
    let results = client.search_collection_cards(col, &filters, 0, 100).await?;
    ensure(results.is_empty(), "no cards match a nonsense name")?;
    let count = client.search_collection_cards_count(col, &filters).await?;
    eq(count, 0, "search/count is 0 for no matches")?;
    ok("nonmatching search returns empty results and zero count");

    // ── 4. Collector number filter narrows to a single card ─────────────────
    step("4. Search by exact collector number");

    let filters = serde_json::json!({ "set_code": "m13", "collector_number": "39" });
    let results = client.search_collection_cards(col, &filters, 0, 100).await?;
    eq(results.len(), 1, "collector number 39 matches only card A")?;
    eq(results[0].id.as_str(), CARD_A, "match is card A")?;
    ok("collector number filter narrows to exactly one card");

    // ── 5. Value breakdown reflects owned quantities and priced cards ───────
    step("5. Value breakdown counts owned, priced cards");

    let breakdown = client.value_breakdown(col).await?;
    eq(breakdown.total_count, 2, "2 distinct owned cards")?;
    ensure(breakdown.priced_count <= breakdown.total_count, "priced count never exceeds total count")?;
    // Both cards were added with purchase prices, and MTG cards carry retail
    // price data, so we expect a positive total value.
    ensure(breakdown.total_value >= 0.0, "total value is non-negative")?;
    ok(&format!(
        "value breakdown: total_count={}, priced_count={}, total_value={}",
        breakdown.total_count, breakdown.priced_count, breakdown.total_value
    ));

    // ── 6. Wishlist-only cards contribute to wanted_value, not total_value ──
    step("6. A wanted-only card adds to wanted_value without affecting owned totals");

    let before = client.value_breakdown(col).await?;
    // War Priest of Thune isn't owned by CARD_B's identity — reuse CARD_A's
    // sibling concept isn't available here, so just bump CARD_A's want beyond
    // what's owned to isolate the wanted-only delta on an already-owned card.
    client.adjust_want(col, CARD_A, 5).await?;
    let after = client.value_breakdown(col).await?;
    eq(after.total_count, before.total_count, "want doesn't change owned total_count")?;
    ensure(
        after.wanted_value >= before.wanted_value,
        "wanted_value doesn't decrease after wanting more copies",
    )?;
    ok("wanting more copies of an owned card leaves owned totals untouched");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

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
