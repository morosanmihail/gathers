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
//! Run (builds `server` and `mirror` first):
//!   cargo run --example price_history_weeks
//!
//! Use prebuilt binaries instead:
//!   GATHERS_BIN_DIR=target/release cargo run --example price_history_weeks

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use e2e::GathersClient;
use retrieval::mirror;

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
    println!("=== GatheRs multi-week price history e2e ===");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let bin_dir = match std::env::var("GATHERS_BIN_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            step("Building server and mirror");
            let status = Command::new(env!("CARGO"))
                .args(["build", "--bin", "server", "--bin", "mirror"])
                .current_dir(&workspace)
                .status()?;
            eyre::ensure!(status.success(), "building server and mirror failed");
            workspace.join("target/debug")
        }
    };

    let tmp = tempfile::tempdir()?;
    let mut env = TestEnv::new(tmp.path(), &workspace, &bin_dir)?;
    let result = run(&mut env).await;
    if result.is_err() {
        env.dump_logs();
        let kept = tmp.keep();
        println!("\nTest files kept in {}", kept.display());
    }
    result
}

async fn run(env: &mut TestEnv) -> eyre::Result<()> {
    // ── Deploy mirror + server ──────────────────────────────────────────────
    step("Deploy mirror and server");

    env.publish_card_dbs()?;
    env.start_mirror().await?;
    ok(&format!("mirror serving test card DBs on :{}", env.mirror_port));

    let client = env.start_server().await?;
    let info = client.system_info().await?;
    ensure(info.price_history_enabled, "server reports price history enabled")?;
    ensure(
        info.systems.iter().any(|s| s == MTG) && info.systems.iter().any(|s| s == POKEMON),
        "MTG and Pokemon systems bootstrapped from the mirror",
    )?;
    ensure(env.db_dir.join("storage.prices.db").exists(), "storage.prices.db created next to storage.db")?;
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
    let start = Instant::now();
    for week in 1..=WEEKS {
        step(&format!("Week {week} ({})", date(week)));

        if week != STALE_MTG_WEEK {
            env.publish_mtg_prices(week)?;
        }
        env.publish_pokemon_prices(week)?;

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
    env.stop_server();
    let client = env.start_server().await?;
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

async fn wait_until<F, Fut>(what: &str, mut check: F) -> eyre::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = eyre::Result<bool>>,
{
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if check().await.unwrap_or(false) {
            return Ok(());
        }
        eyre::ensure!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

// ── Deployed mirror + server ────────────────────────────────────────────────

struct TestEnv {
    root: PathBuf,
    workspace: PathBuf,
    bin_dir: PathBuf,
    mirror_dir: PathBuf,
    db_dir: PathBuf,
    mirror_port: u16,
    server_port: u16,
    mirror: Option<Child>,
    server: Option<Child>,
    /// Every Pokémon price row published so far — that database keeps its
    /// whole history, newest rows winning.
    pokemon_rows: Vec<(String, f64, f64)>,
}

impl TestEnv {
    fn new(root: &Path, workspace: &Path, bin_dir: &Path) -> eyre::Result<Self> {
        let env = Self {
            root: root.to_path_buf(),
            workspace: workspace.to_path_buf(),
            bin_dir: bin_dir.to_path_buf(),
            mirror_dir: root.join("mirror"),
            db_dir: root.join("db"),
            mirror_port: free_port()?,
            server_port: free_port()?,
            mirror: None,
            server: None,
            pokemon_rows: vec![],
        };
        std::fs::create_dir_all(&env.mirror_dir)?;
        std::fs::create_dir_all(&env.db_dir)?;
        std::fs::create_dir_all(root.join("home"))?;
        Ok(env)
    }

    /// Compresses `src` into the mirror as `{stem}.bz2` + `.sha256`, exactly
    /// as the real mirror publishes.
    fn publish(&self, src: &Path, stem: &str) -> eyre::Result<()> {
        let staging = tempfile::tempdir_in(&self.root)?;
        let bz2 = staging.path().join(format!("{stem}.bz2"));
        mirror::compress_bz2(src, &bz2)?;
        mirror::write_with_sha256(&bz2, &self.mirror_dir, stem)
    }

    fn publish_card_dbs(&self) -> eyre::Result<()> {
        let data = self.workspace.join("data");
        self.publish(&data.join("testPrintings.db"), "AllPrintings.sqlite")?;
        self.publish(&data.join("pokemon.db"), "pokemon.sqlite")?;
        // Fresh markers keep the mirror from refreshing anything upstream.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
        for stem in ["AllPrintings.sqlite", "AllPricesToday.sqlite", "pokemon_prices.sqlite", "riftbound.sqlite", "pokemon.sqlite"] {
            std::fs::write(self.mirror_dir.join(format!("{stem}.last_update")), now.to_string())?;
        }
        Ok(())
    }

    /// Publishes an `AllPricesToday` database holding `week`'s prices.
    fn publish_mtg_prices(&self, week: u32) -> eyre::Result<()> {
        let path = self.root.join(format!("mtg-prices-{week}.sqlite"));
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
        self.publish(&path, "AllPricesToday.sqlite")
    }

    /// Adds `week`'s row for the Pokémon card and publishes everything so far.
    fn publish_pokemon_prices(&mut self, week: u32) -> eyre::Result<()> {
        self.pokemon_rows.push((format!("{}T00:00:00.000Z", date(week)), pokemon_raw(week), pokemon_psa10(week)));
        let path = self.root.join(format!("pokemon-prices-{week}.sqlite"));
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE prices (date TEXT, cardId TEXT, variant TEXT, rawPrice REAL, gradedPriceTen REAL, gradedPriceNine REAL);",
        )?;
        for (day, raw, psa10) in &self.pokemon_rows {
            conn.execute(
                "INSERT INTO prices VALUES (?1, ?2, '', ?3, ?4, 0.0)",
                rusqlite::params![day, CARD_P, raw, psa10],
            )?;
        }
        drop(conn);
        self.publish(&path, "pokemon_prices.sqlite")
    }

    async fn start_mirror(&mut self) -> eyre::Result<()> {
        let log = std::fs::File::create(self.root.join("mirror.log"))?;
        self.mirror = Some(
            Command::new(self.bin_dir.join("mirror"))
                .env("MIRROR_DATA_DIR", &self.mirror_dir)
                .env("MIRROR_PORT", self.mirror_port.to_string())
                .env("MIRROR_INTERVAL_HOURS", "8760")
                .env("HOME", self.root.join("home"))
                .stdout(Stdio::from(log.try_clone()?))
                .stderr(Stdio::from(log))
                .spawn()?,
        );
        let url = format!("http://127.0.0.1:{}/AllPrintings.sqlite.bz2.sha256", self.mirror_port);
        wait_until("mirror to serve files", || async {
            Ok(reqwest::get(&url).await.is_ok_and(|r| r.status().is_success()))
        })
        .await
    }

    async fn start_server(&mut self) -> eyre::Result<GathersClient> {
        let mirrors_toml = self.root.join("mirrors.toml");
        std::fs::write(&mirrors_toml, format!("mirrors = [\"http://127.0.0.1:{}\"]\n", self.mirror_port))?;
        let log = std::fs::OpenOptions::new().create(true).append(true).open(self.root.join("server.log"))?;
        let db = |file: &str| self.db_dir.join(file);
        let mut cmd = Command::new(self.bin_dir.join("server"));
        cmd.args(["--port", &self.server_port.to_string()])
            .env("HOME", self.root.join("home"))
            .env("GATHERS_MIRRORS_PATH", &mirrors_toml)
            .env("GATHERS_SYSTEMS", "sql,pokemon-sql")
            .env("GATHERS_PRICE_HISTORY", "true")
            .env("MTG_DB_PATH", db("AllPrintings.db"))
            .env("MTG_PRICES_PATH", db("AllPricesToday.db"))
            .env("POKEMON_DB_PATH", db("pokemon.db"))
            .env("POKEMON_PRICES_PATH", db("pokemon_prices.sqlite"))
            .env("STORAGE_DB_PATH", db("storage.db"))
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log));
        for var in ["PRICE_HISTORY_DB_PATH", "RIFTBOUND_DB_PATH", "GATHERS_NO_AUTO_UPDATE", "DEMO_MODE", "GATHERS_CORS_ORIGINS"] {
            cmd.env_remove(var);
        }
        self.server = Some(cmd.spawn()?);

        let client = GathersClient::new(format!("http://127.0.0.1:{}", self.server_port));
        wait_until("server to bootstrap MTG and Pokemon from the mirror", || async {
            let info = client.system_info().await?;
            Ok(info.downloading.is_empty() && info.systems.iter().any(|s| s == MTG) && info.systems.iter().any(|s| s == POKEMON))
        })
        .await?;
        Ok(client)
    }

    fn stop_server(&mut self) {
        if let Some(mut child) = self.server.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn dump_logs(&self) {
        for name in ["server.log", "mirror.log"] {
            let content = std::fs::read_to_string(self.root.join(name)).unwrap_or_default();
            let lines: Vec<&str> = content.lines().collect();
            println!("\n--- last lines of {name} ---");
            for line in &lines[lines.len().saturating_sub(40)..] {
                println!("{line}");
            }
        }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.stop_server();
        if let Some(mut child) = self.mirror.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn free_port() -> eyre::Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
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
