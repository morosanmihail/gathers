//! End-to-end simulation of several weeks of price history, against a server
//! and mirror this test deploys itself — no live server, network or
//! downloaded databases needed.
//!
//! Setup:
//!   - publishes the repo's small test card databases (`data/testPrintings.db`,
//!     `data/pokemon.db`) into a temp dir and serves it with the real `mirror`
//!     binary (its own upstream refresh is suppressed with fresh
//!     `.last_update` markers)
//!   - starts the real `server` in a temp HOME with price history on, MTG
//!     (`sql`) and Pokémon (`pokemon-sql`) enabled and `GATHERS_MIRRORS_PATH`
//!     pointing at that mirror, so it bootstraps its card databases from it
//!
//! Then, for each simulated week (a few milliseconds each), the test rewrites
//! the MTG and Pokémon price databases on the mirror as dated a week later,
//! triggers both `/prices/update` endpoints and waits for the server to
//! download them and record history. Along the way:
//!   - a card is added to the collection mid-run (history starts that week)
//!   - a card is removed (history stops, but what was recorded stays)
//!   - a card with prices is never in a collection (never recorded)
//!   - one retailer doesn't list one finish for a week (no entry that week)
//!   - one week the MTG mirror isn't updated (no new entries that week — the
//!     history is dated by the price data, not by when it was fetched)
//!   - one week a Pokémon PSA 10 price isn't re-quoted (its last quote keeps
//!     its own date instead of being copied forward)
//!
//! Finally every card's full history is compared exactly with what the
//! simulation expects, the server is restarted and the history checked again.
//!
//! Run:
//!   cargo run --example price_history_weeks

use std::time::Duration;

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup, wait_until};

const WEEKS: u32 = 6;
/// Week whose MTG prices are never published: the mirror keeps serving the
/// previous week's file.
const STALE_MTG_WEEK: u32 = 5;
/// Week a Pokémon PSA 10 price isn't re-quoted.
const UNQUOTED_PSA10_WEEK: u32 = 4;
/// Week cardmarket has no foil listing for `CARD_A`.
const NO_CARDMARKET_FOIL_WEEK: u32 = 2;
/// `CARD_D` is added to the collection right after this week's update.
const ADD_D_WEEK: u32 = 3;
/// `CARD_B` is removed from the collection right after this week's update.
const REMOVE_B_WEEK: u32 = 4;

const MTG: &str = "MagicSQLite";
const POKEMON: &str = "PokemonSQLite";

// All in data/testPrintings.db.
/// War Priest of Thune: two retailers, both finishes.
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
/// Goblin King: removed after `REMOVE_B_WEEK`.
const CARD_B: &str = "0001e0d0-2dcd-5640-aadc-a84765cf5fc9";
/// Grave Titan: priced, but never in a collection.
const CARD_C: &str = "0005283f-d113-5937-ba52-a30570bfb334";
/// Sphinx of the Final Word: added after `ADD_D_WEEK`.
const CARD_D: &str = "00010d56-fe38-5e35-8aed-518019aa36a5";
// In data/pokemon.db.
const CARD_P: &str = "Scarlet-&-Violet-Miraidon-ex-081";

/// `(date, retailer, finish, price, currency)` — one price history entry.
type Entry = (String, String, String, f64, String);

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("multi-week price history")?;
    let result = run(&mut harness).await;
    harness.conclude(result)
}

async fn run(harness: &mut Harness) -> eyre::Result<()> {
    // ── Deploy mirror + server ──────────────────────────────────────────────
    step("Deploy mirror and server");

    harness.publish_to_mirror(&harness.data_file("testPrintings.db"), "AllPrintings.sqlite")?;
    harness.publish_to_mirror(&harness.data_file("pokemon.db"), "pokemon.sqlite")?;
    let mirrors_toml = harness.start_mirror().await?;
    ok("mirror serving the test card DBs");

    // Card databases start missing: the server fetches them from the mirror.
    let db = harness.root().join("db");
    let db_path = |file: &str| db.join(file).to_string_lossy().into_owned();
    let setup = ServerSetup::new()?
        .auto_update()
        .env("GATHERS_MIRRORS_PATH", mirrors_toml.to_string_lossy())
        .env("GATHERS_SYSTEMS", "sql,pokemon-sql")
        .env("GATHERS_PRICE_HISTORY", "true")
        .env("MTG_DB_PATH", db_path("AllPrintings.db"))
        .env("MTG_PRICES_PATH", db_path("AllPricesToday.db"))
        .env("POKEMON_DB_PATH", db_path("pokemon.db"))
        .env("POKEMON_PRICES_PATH", db_path("pokemon_prices.sqlite"))
        .expect_system(MTG)
        .expect_system(POKEMON);
    let client = harness.start_server(&setup).await?;
    let info = client.system_info().await?;
    ensure(info.price_history_enabled, "server reports price history enabled")?;
    ensure(db.join("storage.prices.db").exists(), "storage.prices.db created next to storage.db")?;
    ok("server up, card DBs downloaded from mirror, price history DB created");

    // ── Collection ──────────────────────────────────────────────────────────
    step("Create collection with cards A, B and Pokemon P");

    let col = "Weeks";
    client.add_collection(col).await?;
    client.add_cards_with_provider(col, CARD_A, "", 1, None, Some(MTG)).await?;
    client.add_cards_with_provider(col, CARD_A, "foil", 1, None, Some(MTG)).await?;
    client.add_cards_with_provider(col, CARD_B, "", 2, None, Some(MTG)).await?;
    client.add_cards_with_provider(col, CARD_P, "", 1, None, Some(POKEMON)).await?;
    // No price DB has been published yet: adding cards records nothing.
    tokio::time::sleep(Duration::from_millis(300)).await;
    for (provider, card) in [(MTG, CARD_A), (MTG, CARD_B), (POKEMON, CARD_P)] {
        ensure(history(&client, provider, card).await?.is_empty(), "no history before any prices exist")?;
    }
    ok("cards added; no history yet without price data");

    // ── Weeks ───────────────────────────────────────────────────────────────
    let mut pokemon_rows = vec![];
    let start = std::time::Instant::now();
    for week in 1..=WEEKS {
        step(&format!("Week {week} ({})", date(week)));

        if week != STALE_MTG_WEEK {
            publish_mtg_prices(harness, week)?;
        }
        publish_pokemon_prices(harness, &mut pokemon_rows, week)?;

        client.update_prices("mtg").await?;
        client.update_prices("pokemon").await?;
        wait_until("price downloads to finish", || async {
            let info = client.system_info().await?;
            Ok(!info.downloading.contains_key("Sql-prices") && !info.downloading.contains_key("PokemonSql-prices"))
        })
        .await?;

        let mtg_day = date(mtg_data_week(week));
        if week == STALE_MTG_WEEK {
            // Nothing new to wait for; let the snapshot (which re-records the
            // previous week's prices under their own date) finish.
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else {
            wait_for_day(&client, MTG, CARD_A, &mtg_day).await?;
        }
        wait_for_day(&client, POKEMON, CARD_P, &date(week)).await?;

        // The server now serves this week's prices…
        let market = client.mtg_prices(CARD_A).await?;
        eq(
            market.get("cardkingdom").and_then(|r| r.get("date")).and_then(|d| d.as_str()),
            Some(mtg_day.as_str()),
            "MTG prices served are as of the latest published week",
        )?;
        // …and A's history for that day is exactly that week's prices.
        let latest: Vec<Entry> = history(&client, MTG, CARD_A).await?.into_iter().filter(|e| e.0 == mtg_day).collect();
        eq(latest, expected_mtg(CARD_A, mtg_data_week(week)), "card A's entries for the week")?;

        if week == STALE_MTG_WEEK {
            ok("MTG mirror unchanged: no new MTG entries");
        } else {
            ok("MTG prices recorded");
        }
        ok("Pokemon prices recorded");

        if week == ADD_D_WEEK {
            client.add_cards_with_provider(col, CARD_D, "", 1, None, Some(MTG)).await?;
            let d = wait_for_day(&client, MTG, CARD_D, &date(week)).await?;
            eq(d, expected_mtg(CARD_D, week), "card D's prices recorded when added")?;
            ok("card D added: its current prices recorded straight away");
        }
        if week == REMOVE_B_WEEK {
            client.remove_cards(col, CARD_B, "", 2).await?;
            ok("card B removed from the collection");
        }
    }
    ok(&format!("{WEEKS} weeks simulated in {:?}", start.elapsed()));

    // ── Final histories ─────────────────────────────────────────────────────
    step("Validate final price histories");
    let expected = expected_histories();
    check_histories(&client, &expected).await?;

    // ── Restart ─────────────────────────────────────────────────────────────
    step("Restart server — history persists");
    harness.stop("server");
    let client = harness.start_server(&setup).await?;
    check_histories(&client, &expected).await?;

    println!("\n=== All multi-week price history checks passed ===");
    Ok(())
}

// ── Simulated market ────────────────────────────────────────────────────────

/// Monday of simulated week `week` (1-based), seven days apart.
fn date(week: u32) -> String {
    (chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap() + chrono::Days::new(7 * (week as u64 - 1)))
        .format("%Y-%m-%d")
        .to_string()
}

/// The week whose MTG price file the mirror serves during `week`.
fn mtg_data_week(week: u32) -> u32 {
    if week == STALE_MTG_WEEK { week - 1 } else { week }
}

/// `(retailer, mtgjson finish, currency)` listings of each MTG card in
/// `week`. Prices are multiples of 0.25 so they survive storage exactly.
fn mtg_listings(card: &str, week: u32) -> Vec<(&'static str, &'static str, &'static str)> {
    match card {
        CARD_A => {
            let mut l = vec![("cardkingdom", "normal", "USD"), ("cardkingdom", "foil", "USD"), ("cardmarket", "normal", "EUR")];
            if week != NO_CARDMARKET_FOIL_WEEK {
                l.push(("cardmarket", "foil", "EUR"));
            }
            l
        }
        CARD_B | CARD_C => vec![("tcgplayer", "normal", "USD")],
        CARD_D => vec![("cardkingdom", "normal", "USD"), ("cardkingdom", "foil", "USD")],
        _ => vec![],
    }
}

fn mtg_price(card: &str, listing: usize, week: u32) -> f64 {
    let base = [CARD_A, CARD_B, CARD_C, CARD_D].iter().position(|c| *c == card).unwrap() as f64 * 8.0;
    0.25 * (4.0 + base + listing as f64 * 2.0 + week as f64)
}

fn pokemon_raw(week: u32) -> f64 {
    5.0 + 0.25 * week as f64
}

fn pokemon_psa10(week: u32) -> f64 {
    if week == UNQUOTED_PSA10_WEEK { 0.0 } else { 50.0 + week as f64 }
}

/// What history should hold for an MTG card's prices from `week`, sorted
/// like the server sorts (retailer, then finish).
fn expected_mtg(card: &str, week: u32) -> Vec<Entry> {
    let mut entries: Vec<Entry> = mtg_listings(card, week)
        .into_iter()
        .enumerate()
        .map(|(i, (retailer, finish, currency))| {
            let finish = if finish == "normal" { "" } else { finish };
            (date(week), retailer.to_string(), finish.to_string(), mtg_price(card, i, week), currency.to_string())
        })
        .collect();
    entries.sort_by(|a, b| (&a.1, &a.2).cmp(&(&b.1, &b.2)));
    entries
}

/// Every card's complete expected history, oldest first.
fn expected_histories() -> Vec<(&'static str, &'static str, Vec<Entry>)> {
    let mtg_weeks: Vec<u32> = (1..=WEEKS).filter(|w| *w != STALE_MTG_WEEK).collect();
    let mtg_history = |card: &str, weeks: &[u32]| weeks.iter().flat_map(|w| expected_mtg(card, *w)).collect::<Vec<_>>();

    let a_weeks = mtg_weeks.clone();
    let b_weeks: Vec<u32> = mtg_weeks.iter().copied().filter(|w| *w <= REMOVE_B_WEEK).collect();
    let d_weeks: Vec<u32> = mtg_weeks.iter().copied().filter(|w| *w >= ADD_D_WEEK).collect();

    let mut pokemon = vec![];
    for week in 1..=WEEKS {
        if pokemon_psa10(week) > 0.0 {
            pokemon.push((date(week), "graded_psa10".to_string(), String::new(), pokemon_psa10(week), "USD".to_string()));
        }
        pokemon.push((date(week), "raw".to_string(), String::new(), pokemon_raw(week), "USD".to_string()));
    }

    vec![
        (MTG, CARD_A, mtg_history(CARD_A, &a_weeks)),
        (MTG, CARD_B, mtg_history(CARD_B, &b_weeks)),
        (MTG, CARD_C, vec![]),
        (MTG, CARD_D, mtg_history(CARD_D, &d_weeks)),
        (POKEMON, CARD_P, pokemon),
    ]
}

async fn check_histories(client: &GathersClient, expected: &[(&str, &str, Vec<Entry>)]) -> eyre::Result<()> {
    for (provider, card, entries) in expected {
        let got = history(client, provider, card).await?;
        eq(&got, entries, &format!("{provider} {card} full history"))?;
        let days = entries.iter().map(|e| e.0.as_str()).collect::<std::collections::BTreeSet<_>>();
        ok(&format!("{card}: {} entries over {} day(s)", entries.len(), days.len()));
    }
    Ok(())
}

// ── Server queries ──────────────────────────────────────────────────────────

async fn history(client: &GathersClient, provider: &str, card: &str) -> eyre::Result<Vec<Entry>> {
    Ok(client
        .price_history(provider, card)
        .await?
        .entries
        .into_iter()
        .map(|e| (e.recorded_on, e.retailer, e.finish, e.price, e.currency))
        .collect())
}

/// Waits for `card` to have entries dated `day` (recording happens in the
/// background), returning them.
async fn wait_for_day(client: &GathersClient, provider: &str, card: &str, day: &str) -> eyre::Result<Vec<Entry>> {
    wait_until(&format!("{card} history for {day}"), || async {
        Ok(history(client, provider, card).await?.iter().any(|e| e.0 == day))
    })
    .await?;
    Ok(history(client, provider, card).await?.into_iter().filter(|e| e.0 == day).collect())
}

// ── Mirror contents ─────────────────────────────────────────────────────────

/// Publishes an `AllPricesToday` database holding `week`'s prices.
fn publish_mtg_prices(harness: &Harness, week: u32) -> eyre::Result<()> {
    let path = harness.root().join(format!("mtg-prices-{week}.sqlite"));
    let conn = rusqlite::Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE prices (uuid TEXT, date TEXT, source TEXT, provider TEXT, priceType TEXT, finish TEXT, price REAL, currency TEXT);",
    )?;
    let mut insert = conn.prepare("INSERT INTO prices VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
    for card in [CARD_A, CARD_B, CARD_C, CARD_D] {
        for (i, (retailer, finish, currency)) in mtg_listings(card, week).into_iter().enumerate() {
            let price = mtg_price(card, i, week);
            insert.execute(rusqlite::params![card, date(week), "paper", retailer, "retail", finish, price, currency])?;
            // Buylist and online prices aren't market prices; never recorded.
            insert.execute(rusqlite::params![card, date(week), "paper", retailer, "buylist", finish, price / 2.0, currency])?;
            insert.execute(rusqlite::params![card, date(week), "mtgo", retailer, "retail", finish, price / 4.0, currency])?;
        }
    }
    drop(insert);
    drop(conn);
    harness.publish_to_mirror(&path, "AllPricesToday.sqlite")
}

/// Adds `week`'s row for the Pokémon card to `rows` and publishes them all
/// — that database keeps its whole history, newest rows winning.
fn publish_pokemon_prices(harness: &Harness, rows: &mut Vec<(String, f64, f64)>, week: u32) -> eyre::Result<()> {
    rows.push((format!("{}T00:00:00.000Z", date(week)), pokemon_raw(week), pokemon_psa10(week)));
    let path = harness.root().join(format!("pokemon-prices-{week}.sqlite"));
    let conn = rusqlite::Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE prices (date TEXT, cardId TEXT, variant TEXT, rawPrice REAL, gradedPriceTen REAL, gradedPriceNine REAL);",
    )?;
    for (day, raw, psa10) in rows.iter() {
        conn.execute("INSERT INTO prices VALUES (?1, ?2, '', ?3, ?4, 0.0)", rusqlite::params![day, CARD_P, raw, psa10])?;
    }
    drop(conn);
    harness.publish_to_mirror(&path, "pokemon_prices.sqlite")
}

// ── Assertions ──────────────────────────────────────────────────────────────

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
        Err(eyre::eyre!("{label}:\n  expected {expected:?}\n  got      {got:?}"))
    }
}

fn step(label: &str) {
    println!("\n[{label}]");
}

fn ok(msg: &str) {
    println!("  ✓ {msg}");
}
