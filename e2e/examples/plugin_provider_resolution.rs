//! End-to-end test for provider resolution when adding a card (or adjusting
//! its want quantity) to a collection:
//!
//!   - an explicit `provider` is trusted only after being verified against
//!     that specific provider, not accepted blindly
//!   - omitting `provider` falls back to probing every configured system
//!     and plugin in turn — correct only when ids happen to be unique
//!     across all of them
//!   - two plugins that mint the same id no longer let the wrong one "win"
//!     a card, as long as the caller names which one it means
//!
//! Deploys its own server (see `e2e::harness`) configured with two plugins
//! pointing at the *same* `dummy-plugin` instance under different names, so
//! both report an identical catalog (including the id `book-1` / "Dune") —
//! the simplest way to reproduce a real id collision across providers
//! without needing two different plugin implementations:
//!   cargo run --example plugin_provider_resolution

use e2e::models::CollectionCard;
use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};

const BOOK_ID: &str = "book-1"; // Dune — identical under both registered plugin names
const PLUGIN_A: &str = "books-a";
const PLUGIN_B: &str = "books-b";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("plugin provider resolution")?;
    let plugin = harness.start_dummy_plugin().await?;
    // No card systems, only the two plugins; pricing off as plugins have none.
    let setup = ServerSetup::new()?.config(format!(
        r#"system = []
port = 0
pricing_enabled = false
collections_enabled = true

[[plugins]]
name = "{PLUGIN_A}"
base_url = "{plugin}"
enabled = true

[[plugins]]
name = "{PLUGIN_B}"
base_url = "{plugin}"
enabled = true
"#
    ));
    let client = harness.start_server(&setup).await?;

    let result = run(&client, "e2e-prov-a", "e2e-prov-b", "e2e-prov-fallback", "e2e-prov-bogus", "e2e-prov-want").await;
    harness.conclude(result)
}

async fn run(
    client: &GathersClient,
    col_a: &str,
    col_b: &str,
    col_fallback: &str,
    col_bogus: &str,
    col_want: &str,
) -> eyre::Result<()> {
    for c in [col_a, col_b, col_fallback, col_bogus, col_want] {
        client.add_collection(c).await?;
    }

    let provider_a = format!("plugin-{PLUGIN_A}");
    let provider_b = format!("plugin-{PLUGIN_B}");

    // ── 1 & 2. Explicit provider is honored exactly, both directions ────────
    // If the server were still probing every plugin and taking the first
    // (HashMap-iteration-order) match, these two would either both land on
    // the same plugin regardless of what was asked, or be non-deterministic
    // across runs. Getting *both* explicit choices exactly right proves the
    // request is actually driving the outcome.
    step("1. Explicit provider A wins exactly, despite an identical id existing under B");
    client
        .add_cards_with_provider(col_a, BOOK_ID, "", 1, None, Some(&provider_a))
        .await?;
    let cards = client.list_cards(col_a).await?;
    let card = find_card(&cards, BOOK_ID)?;
    eq(card.provider.clone(), provider_a.clone(), "book-1 stored under plugin-books-a")?;
    ok("explicit provider A recorded exactly");

    step("2. Explicit provider B wins exactly, despite an identical id existing under A");
    client
        .add_cards_with_provider(col_b, BOOK_ID, "", 1, None, Some(&provider_b))
        .await?;
    let cards = client.list_cards(col_b).await?;
    let card = find_card(&cards, BOOK_ID)?;
    eq(card.provider.clone(), provider_b.clone(), "book-1 stored under plugin-books-b")?;
    ok("explicit provider B recorded exactly");

    // ── 3. Omitted provider still resolves to a real, valid one ─────────────
    step("3. Omitted provider falls back to probing every configured plugin");
    client.add_cards(col_fallback, BOOK_ID, "", 1, None).await?;
    let cards = client.list_cards(col_fallback).await?;
    let card = find_card(&cards, BOOK_ID)?;
    ensure(
        card.provider == provider_a || card.provider == provider_b,
        &format!("fallback resolved to an unexpected provider: {}", card.provider),
    )?;
    ok(&format!(
        "fallback resolved to {} (which one is ambiguous by construction, but it's a real one)",
        card.provider
    ));

    // ── 4. A bogus/wrong claimed provider isn't trusted blindly ─────────────
    step("4. A provider claim that doesn't actually have the card is rejected, not trusted");
    client
        .add_cards_with_provider(col_bogus, BOOK_ID, "", 1, None, Some("plugin-does-not-exist"))
        .await?;
    let cards = client.list_cards(col_bogus).await?;
    let card = find_card(&cards, BOOK_ID)?;
    ensure(
        card.provider != "plugin-does-not-exist",
        "an unverifiable provider claim must not be stored as-is",
    )?;
    ensure(
        card.provider == provider_a || card.provider == provider_b,
        &format!("bogus-provider fallback resolved to an unexpected provider: {}", card.provider),
    )?;
    ok(&format!("bogus claim rejected, fell back to {}", card.provider));

    // ── 5. adjust_want (want-only entries) follows the same rules ───────────
    step("5. adjust_want honors an explicit provider the same way add_cards does");
    client
        .adjust_want_with_provider(col_want, BOOK_ID, 2, Some(&provider_b))
        .await?;
    let cards = client.list_cards(col_want).await?;
    let card = find_card(&cards, BOOK_ID)?;
    eq(card.provider.clone(), provider_b, "want-only entry stored under plugin-books-b")?;
    eq(card.want_quantity, 2, "want quantity recorded")?;
    ok("adjust_want explicit provider recorded exactly");

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
