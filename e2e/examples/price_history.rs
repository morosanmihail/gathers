//! End-to-end test for collection price history (`price_history_enabled` in
//! `server.toml`, or `GATHERS_PRICE_HISTORY=true`): daily prices of cards
//! tracked in collections, kept in a separate `storage.prices.db`.
//!
//! Covers:
//!   1. the server reports whether price history is on, and the endpoint
//!      answers either way (empty when off)
//!   2. adding a card records its current prices, in the background, dated
//!      by the day the price database says they're as of
//!   3. recorded entries mirror the card's current market prices, using the
//!      collection finish convention (`""` = default finish)
//!   4. adding the same card again the same day doesn't duplicate entries
//!   5. wanting a card (wishlist) records its prices too
//!   6. removing a card from the collection keeps its history
//!   7. unknown cards/providers just have no history
//!
//! Steps 2–6 need the MTG `Sql` system with its price database downloaded;
//! they're skipped (with a note) otherwise.
//!
//! Run against a live server (Tilt starts it with price history on):
//!   cargo run --example price_history
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example price_history

use std::time::Duration;

use e2e::models::PriceHistoryEntry;
use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Mutilate — M13 #102
const CARD_B: &str = "c83a7592-5879-5d52-b27c-e866597b389f";
const MTG_PROVIDER: &str = "MagicSQLite";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs price history e2e ===");
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
    let col = format!("e2e-price-history-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);

    let result = run(&client, &col).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    // ── 1. Reported status and the endpoint itself ──────────────────────────
    step("1. Price history status");

    let info = client.system_info().await?;
    ensure(info.pricing_enabled, "pricing must be enabled on the server")?;
    let history = client.price_history(MTG_PROVIDER, CARD_A).await?;
    eq(history.enabled, info.price_history_enabled, "endpoint and /api/system agree on enabled")?;
    if !info.price_history_enabled {
        ensure(history.entries.is_empty(), "no entries while disabled")?;
        ok("disabled: endpoint answers with no entries");
        skip("price history is off — start the server with GATHERS_PRICE_HISTORY=true to test recording");
        return Ok(());
    }
    ok("price history enabled");

    if !info.systems.iter().any(|s| s == MTG_PROVIDER) {
        skip("MTG Sql system isn't active — can't test recording");
        return Ok(());
    }
    let market = client.mtg_prices(CARD_A).await?;
    if market.is_empty() {
        skip("no MTG prices available (price DB not downloaded?) — can't test recording");
        return Ok(());
    }

    client.add_collection(col).await?;
    let as_of = as_of(&market);

    // ── 2. Adding a card records its current prices ─────────────────────────
    step("2. Add a card — its prices are recorded");

    let added = client.add_cards_with_provider(col, CARD_A, "", 1, None, Some(MTG_PROVIDER)).await?;
    eq(added[0].provider.as_str(), MTG_PROVIDER, "card stored under the MTG provider")?;
    let recorded = wait_for_day(client, CARD_A, &as_of).await?;
    ensure(!recorded.is_empty(), "current prices were recorded after adding the card")?;
    ok(&format!("{} price(s) recorded for {as_of}", recorded.len()));

    // ── 3. Entries mirror current market prices ─────────────────────────────
    step("3. Entries match the card's market prices");

    for entry in &recorded {
        let retailer = market
            .get(&entry.retailer)
            .ok_or_else(|| eyre::eyre!("entry for unknown retailer '{}'", entry.retailer))?;
        let field = match entry.finish.as_str() {
            "" => "normal",
            other => other,
        };
        let expected = retailer.get(field).and_then(|v| v.as_f64());
        eq(expected, Some(entry.price), &format!("{} {:?} price", entry.retailer, entry.finish))?;
        eq(
            retailer.get("currency").and_then(|v| v.as_str()),
            Some(entry.currency.as_str()),
            &format!("{} currency", entry.retailer),
        )?;
        if let Some(date) = retailer.get("date").and_then(|v| v.as_str()) {
            eq(entry.recorded_on.as_str(), date, &format!("{} as-of date", entry.retailer))?;
        }
    }
    ok("every entry matches a retailer's current price, currency and date");

    // ── 4. Same day, same card: no duplicates ───────────────────────────────
    step("4. Add the same card again — no duplicate entries");

    client.add_cards_with_provider(col, CARD_A, "foil", 1, None, Some(MTG_PROVIDER)).await?;
    // Recording is in the background; give it time to (not) add rows.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let again = entries_on(client, CARD_A, &as_of).await?;
    eq(again.len(), recorded.len(), "still one entry per retailer and finish for that day")?;
    ok("re-recording the same day replaces, not appends");

    // ── 5. Wishlist cards are tracked too ───────────────────────────────────
    step("5. Want a card — its prices are recorded");

    if client.mtg_prices(CARD_B).await?.is_empty() {
        skip("card B has no market prices");
    } else {
        client.adjust_want_with_provider(col, CARD_B, 1, Some(MTG_PROVIDER)).await?;
        let day = as_of_for(client, CARD_B).await?;
        let wanted = wait_for_day(client, CARD_B, &day).await?;
        ensure(!wanted.is_empty(), "current prices recorded for a wanted card")?;
        ok(&format!("{} price(s) recorded for the wanted card", wanted.len()));
    }

    // ── 6. Removing a card keeps its history ────────────────────────────────
    step("6. Remove the card — history stays");

    client.remove_cards(col, CARD_A, "", 1).await?;
    client.remove_cards(col, CARD_A, "foil", 1).await?;
    let kept = entries_on(client, CARD_A, &as_of).await?;
    eq(kept.len(), recorded.len(), "history kept after removal")?;
    ok("history outlives the collection entry");

    // ── 7. Unknown cards and providers ──────────────────────────────────────
    step("7. Unknown card / provider — empty history");

    let unknown = client.price_history(MTG_PROVIDER, "no-such-card").await?;
    ensure(unknown.enabled && unknown.entries.is_empty(), "unknown card has no history")?;
    let unknown = client.price_history("NoSuchProvider", CARD_A).await?;
    ensure(unknown.entries.is_empty(), "card has no history under another provider")?;
    ok("no history for unknown card or provider");

    println!("\n=== All price history checks passed ===");
    Ok(())
}

/// The day a card's market prices are as of: the newest retailer date, or
/// today when the source has none.
fn as_of(market: &std::collections::HashMap<String, serde_json::Value>) -> String {
    market
        .values()
        .filter_map(|r| r.get("date").and_then(|d| d.as_str()).map(str::to_string))
        .max()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
}

async fn as_of_for(client: &GathersClient, card: &str) -> eyre::Result<String> {
    Ok(as_of(&client.mtg_prices(card).await?))
}

async fn entries_on(client: &GathersClient, card: &str, day: &str) -> eyre::Result<Vec<PriceHistoryEntry>> {
    Ok(client
        .price_history(MTG_PROVIDER, card)
        .await?
        .entries
        .into_iter()
        .filter(|e| e.recorded_on == day)
        .collect())
}

/// Recording happens in the background after the request returns; polls
/// for up to ~15s until entries for `day` show up.
async fn wait_for_day(client: &GathersClient, card: &str, day: &str) -> eyre::Result<Vec<PriceHistoryEntry>> {
    for _ in 0..30 {
        let entries = entries_on(client, card, day).await?;
        if !entries.is_empty() {
            return Ok(entries);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(vec![])
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

fn skip(msg: &str) {
    println!("  ↷ skipped: {msg}");
}
