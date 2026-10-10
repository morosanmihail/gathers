//! End-to-end test for collection price history (`price_history_enabled` in
//! `server.toml`, or `GATHERS_PRICE_HISTORY=true`): daily prices of cards
//! tracked in collections, kept in a separate `storage.prices.db`.
//!
//! Covers:
//!   1. with price history off the endpoint answers, empty; with it on the
//!      server reports it
//!   2. adding a card records its current prices, in the background, dated
//!      by the day the price database says they're as of
//!   3. recorded entries mirror the card's current market prices, using the
//!      collection finish convention (`""` = default finish)
//!   4. adding the same card again the same day doesn't duplicate entries
//!   5. wanting a card (wishlist) records its prices too
//!   6. removing a card from the collection keeps its history
//!   7. unknown cards/providers just have no history
//!
//! Deploys its own server (see `e2e::harness`) on a copy of
//! `data/testPrintings.db`, with a small MTG price database this test
//! writes, first with price history off and then on:
//!   cargo run --example price_history

use std::time::Duration;

use e2e::models::PriceHistoryEntry;
use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Goblin King — 3ED #155
const CARD_B: &str = "0001e0d0-2dcd-5640-aadc-a84765cf5fc9";
const MTG_PROVIDER: &str = "MagicSQLite";
/// The day the test price database says its prices are from.
const PRICES_DATE: &str = "2026-03-02";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("price history")?;
    write_prices(&harness.root().join("db/AllPricesToday.db"))?;

    let result = async {
        // ── 1. Off: the endpoint answers, with nothing ─────────────────────────
        step("1. Price history off, then on");
        let off = ServerSetup::mtg(&harness)?.env("GATHERS_PRICE_HISTORY", "false");
        let client = harness.start_server(&off).await?;
        let info = client.system_info().await?;
        ensure(!info.price_history_enabled, "server reports price history off")?;
        let history = client.price_history(MTG_PROVIDER, CARD_A).await?;
        ensure(!history.enabled && history.entries.is_empty(), "endpoint answers disabled, with no entries")?;
        ok("off: endpoint answers with no entries");
        harness.stop("server");

        let on = ServerSetup::mtg(&harness)?.env("GATHERS_PRICE_HISTORY", "true");
        let client = harness.start_server(&on).await?;
        run(&client, "e2e-price-history").await
    }
    .await;
    harness.conclude(result)
}

/// An `AllPricesToday` database pricing card A at two retailers in both
/// finishes and card B at one, dated `PRICES_DATE`. Buylist rows aren't
/// market prices and must never be recorded.
fn write_prices(path: &std::path::Path) -> eyre::Result<()> {
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE prices (uuid TEXT, date TEXT, source TEXT, provider TEXT, priceType TEXT, finish TEXT, price REAL, currency TEXT);",
    )?;
    let rows: &[(&str, &str, &str, &str, f64, &str)] = &[
        (CARD_A, "cardkingdom", "retail", "normal", 0.5, "USD"),
        (CARD_A, "cardkingdom", "retail", "foil", 2.25, "USD"),
        (CARD_A, "cardmarket", "retail", "normal", 0.25, "EUR"),
        (CARD_A, "cardmarket", "retail", "foil", 1.75, "EUR"),
        (CARD_A, "cardkingdom", "buylist", "normal", 0.1, "USD"),
        (CARD_B, "tcgplayer", "retail", "normal", 4.5, "USD"),
    ];
    for (uuid, retailer, kind, finish, price, currency) in rows {
        conn.execute(
            "INSERT INTO prices VALUES (?1, ?2, 'paper', ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![uuid, PRICES_DATE, retailer, kind, finish, price, currency],
        )?;
    }
    Ok(())
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    let info = client.system_info().await?;
    ensure(info.pricing_enabled && info.price_history_enabled, "server reports pricing and price history on")?;
    ensure(client.price_history(MTG_PROVIDER, CARD_A).await?.enabled, "endpoint reports price history on")?;
    ok("on: server and endpoint report price history enabled");

    let market = client.mtg_prices(CARD_A).await?;
    eq(market.len(), 2, "card A is priced at two retailers")?;

    client.add_collection(col).await?;
    let as_of = as_of(&market);
    eq(as_of.as_str(), PRICES_DATE, "prices are as of the price database's date")?;

    // ── 2. Adding a card records its current prices ─────────────────────────
    step("2. Add a card — its prices are recorded");

    let added = client.add_cards_with_provider(col, CARD_A, "", 1, None, Some(MTG_PROVIDER)).await?;
    eq(added[0].provider.as_str(), MTG_PROVIDER, "card stored under the MTG provider")?;
    let recorded = wait_for_day(client, CARD_A, &as_of).await?;
    // Two retailers × two finishes; the buylist row is not a market price.
    eq(recorded.len(), 4, "one entry per retailer and finish")?;
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

    client.adjust_want_with_provider(col, CARD_B, 1, Some(MTG_PROVIDER)).await?;
    let wanted = wait_for_day(client, CARD_B, PRICES_DATE).await?;
    let wanted: Vec<(&str, &str, f64, &str)> =
        wanted.iter().map(|e| (e.retailer.as_str(), e.finish.as_str(), e.price, e.currency.as_str())).collect();
    eq(wanted, vec![("tcgplayer", "", 4.5, "USD")], "wanted card's prices recorded")?;
    ok("the wanted card's price is recorded");

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
