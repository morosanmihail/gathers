//! End-to-end test for the generic third-party plugin catalog routes:
//!   GET  /api/plugins                        (list configured plugins)
//!   POST /api/plugins/{name}/search
//!   POST /api/plugins/{name}/cards/by-ids
//!   GET  /api/plugins/{name}/update
//!
//! `plugin_provider_resolution.rs` covers how *collections* resolve which
//! plugin owns a card when ids collide across two plugins; this test
//! exercises the plugin catalog endpoints themselves against a single
//! plugin, independent of collections.
//!
//! Requires a server configured with the `dummy-plugin` example under the
//! name `books-a` (the same plugin already required by
//! `plugin_provider_resolution.rs`):
//!
//! ```toml
//! [[plugins]]
//! name = "books-a"
//! base_url = "http://localhost:5236"
//! enabled = true
//! ```
//!
//! Run:
//!   cargo run -p dummy-plugin &
//!   cargo run --bin server   # with the [[plugins]] block above in server.toml
//!   cargo run --example plugin_catalog
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example plugin_catalog

use e2e::GathersClient;

const PLUGIN: &str = "books-a";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs plugin catalog e2e ===");
    println!("Server: {url}");
    println!();

    run(&client).await
}

async fn run(client: &GathersClient) -> eyre::Result<()> {
    // ── 1. The plugin is listed with its configured base url ────────────────
    step("1. List configured plugins");

    let plugins = client.list_plugins().await?;
    let plugin = plugins
        .iter()
        .find(|p| p.name == PLUGIN)
        .ok_or_else(|| eyre::eyre!("plugin '{PLUGIN}' not found in /api/plugins listing — is it configured?"))?;
    ensure(!plugin.base_url.is_empty(), "listed plugin has a non-empty base_url")?;
    ok(&format!("plugin '{PLUGIN}' listed at {}", plugin.base_url));

    // ── 2. Search with no filters returns the whole catalog ─────────────────
    step("2. Search with no filters returns cards");

    let all = client.plugin_search(PLUGIN, None, None, None, None).await?;
    ensure(!all.is_empty(), "unfiltered search returns at least one card")?;
    ok(&format!("unfiltered search returned {} cards", all.len()));

    // ── 3. Text filter narrows to matching cards (by name or description) ───
    step("3. Text filter narrows results");

    let dune = client.plugin_search(PLUGIN, Some("dune"), None, None, None).await?;
    ensure(
        dune.iter().any(|c| c.name.eq_ignore_ascii_case("dune")),
        "text filter 'dune' finds the book named Dune",
    )?;
    ensure(dune.len() < all.len(), "text filter narrows the result set")?;
    ok("text filter finds the expected card and narrows results");

    // ── 4. set_code filter narrows to a genre, skip/limit page through it ───
    step("4. set_code filter + skip/limit pagination");

    let scifi = client.plugin_search(PLUGIN, None, Some("SCIFI"), None, None).await?;
    ensure(!scifi.is_empty(), "SCIFI set_code filter matches at least one book")?;
    ensure(
        scifi.iter().all(|c| c.set_code.eq_ignore_ascii_case("scifi")),
        "every result under the SCIFI filter is actually tagged SCIFI",
    )?;

    if scifi.len() > 1 {
        let first_page = client
            .plugin_search(PLUGIN, None, Some("SCIFI"), Some(0), Some(1))
            .await?;
        let second_page = client
            .plugin_search(PLUGIN, None, Some("SCIFI"), Some(1), Some(1))
            .await?;
        eq(first_page.len(), 1, "page 0 (limit 1) returns exactly 1 card")?;
        eq(second_page.len(), 1, "page 1 (limit 1) returns exactly 1 card")?;
        ensure(first_page[0].id != second_page[0].id, "consecutive pages don't repeat a card")?;
        ok("set_code filter + skip/limit page through the SCIFI catalog without overlap");
    } else {
        ok("set_code filter matches (only 1 SCIFI book available — pagination check skipped)");
    }

    // ── 5. cards_by_ids resolves exactly the requested, known ids ───────────
    step("5. cards_by_ids resolves known ids and ignores unknown ones");

    let want_id = all[0].id.clone();
    let resolved = client
        .plugin_cards_by_ids(PLUGIN, vec![want_id.clone(), "definitely-not-a-real-id".to_string()])
        .await?;
    eq(resolved.len(), 1, "only the real id resolves, the bogus one is dropped")?;
    ensure(resolved.contains_key(&want_id), "resolved map contains the requested id")?;
    eq(resolved[&want_id].id.as_str(), want_id.as_str(), "resolved card's own id matches the key")?;
    ok("cards_by_ids resolves known ids exactly, silently drops unknown ones");

    // ── 6. update() acknowledges immediately (dummy plugin has nothing to do) ─
    step("6. Trigger a plugin update");

    let msg = client.plugin_update(PLUGIN).await?;
    ensure(!msg.is_empty(), "update endpoint returns a non-empty status message")?;
    ok(&format!("update acknowledged: {msg:?}"));

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
