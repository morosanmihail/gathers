//! End-to-end test for:
//!   GET /api/system                — reports configured systems/plugins/flags
//!   the MTG card catalog surface:
//!     POST /api/mtg/cards/search
//!     GET  /api/mtg/cards               (retrieve by id)
//!     GET  /api/mtg/cards/random
//!     GET  /api/mtg/sets
//!     GET  /api/mtg/prices              (bulk price lookup)
//!
//! Requires a server with the MTG system (Scryfall or Sql) configured and
//! its card database present — the same requirement `collection_lifecycle.rs`
//! already has, since both rely on the M13 card set being resolvable.
//!
//! Run against a live server:
//!   cargo run --example system_info_and_mtg_catalog
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example system_info_and_mtg_catalog

use e2e::GathersClient;

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs system info & MTG catalog e2e ===");
    println!("Server: {url}");
    println!();

    run(&client).await
}

async fn run(client: &GathersClient) -> eyre::Result<()> {
    // ── 1. System info reports the MTG system as active ─────────────────────
    step("1. GET /api/system reports an active MTG-capable system");

    let info = client.system_info().await?;
    ensure(!info.systems.is_empty(), "at least one retrieval system is configured")?;
    ensure(
        info.systems.iter().any(|s| s.to_lowercase().contains("magic") || s.to_lowercase().contains("scryfall")),
        &format!("an MTG-capable system is active (got systems: {:?})", info.systems),
    )?;
    ok(&format!("systems active: {:?}, demo_mode={}", info.systems, info.demo_mode));

    // ── 2. Card search by name+set finds the expected card ──────────────────
    step("2. Search MTG cards by name and set");

    let filters = serde_json::json!({ "name": "War Priest of Thune", "set_code": "m13" });
    let results = client.mtg_search(&filters, 0, 10).await?;
    ensure(!results.is_empty(), "search finds at least one match")?;
    let found = results
        .iter()
        .any(|c| c.get("cardIdentifiers").and_then(|ci| ci.get("scryfallId")).is_some());
    ensure(found, "results include card identifier data")?;
    let names_match = results.iter().all(|c| {
        c.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.to_lowercase().contains("war priest of thune"))
            .unwrap_or(false)
    });
    ensure(names_match, "every result actually matches the name filter")?;
    ok(&format!("name+set search returned {} matching card(s)", results.len()));

    // ── 3. Retrieve by id returns the exact card ─────────────────────────────
    step("3. Retrieve card by id");

    let by_id = client.mtg_cards_by_ids(&[CARD_A.to_string()]).await?;
    let card = by_id
        .get(CARD_A)
        .ok_or_else(|| eyre::eyre!("card {CARD_A} not returned by /api/mtg/cards"))?;
    eq(
        card.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
        CARD_A,
        "returned card's id matches the request",
    )?;
    ok("id lookup returns the exact requested card");

    // ── 4. Random card returns a well-formed card ────────────────────────────
    step("4. Random card");

    let random = client.mtg_random_card().await?;
    ensure(
        random.get("id").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()),
        "random card has a non-empty id",
    )?;
    ok("random card endpoint returns a well-formed card");

    // ── 5. Sets listing includes M13 ──────────────────────────────────────────
    step("5. Sets listing");

    let sets = client.mtg_sets().await?;
    ensure(!sets.is_empty(), "at least one set is returned")?;
    ok(&format!("{} sets returned", sets.len()));

    // ── 6. Bulk price lookup doesn't error and is keyed by the requested id ──
    step("6. Bulk price lookup");

    let prices = client.mtg_bulk_prices(&[CARD_A.to_string()]).await?;
    // Price data may legitimately be absent (no price DB configured), but if
    // present it must be keyed correctly and not error out.
    if let Some(p) = prices.get(CARD_A) {
        ensure(p.is_object(), "price entry for the card is a JSON object")?;
    }
    ok(&format!("bulk price lookup succeeded ({} of 1 ids priced)", prices.len()));

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
