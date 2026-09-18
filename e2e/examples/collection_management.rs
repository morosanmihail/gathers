//! End-to-end test for collection lifecycle management that
//! `collection_lifecycle.rs` doesn't cover: creation idempotency, renaming,
//! name validation, and removal of a collection that still holds cards.
//!
//! Run against a live server:
//!   cargo run --example collection_management
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example collection_management

use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs collection management e2e ===");
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
    let col = format!("e2e-mgmt-{tag}");
    let col_renamed = format!("e2e-mgmt-renamed-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);
    guard.register(&col_renamed);

    let result = run(&client, &col, &col_renamed).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col: &str, col_renamed: &str) -> eyre::Result<()> {
    // ── 1. Creating a collection is idempotent ───────────────────────────────
    step("1. Creating a collection twice doesn't error or duplicate it");

    client.add_collection(col).await?;
    client.add_collection(col).await?;

    let collections = client.list_collections().await?;
    let matches = collections.iter().filter(|c| c.id == col).count();
    eq(matches, 1, "collection appears exactly once after adding it twice")?;
    ok("duplicate add is idempotent");

    // ── 2. Name validation rejects bad input ─────────────────────────────────
    step("2. Collection name validation");

    ensure(client.add_collection("").await.is_err(), "empty name rejected")?;
    let too_long = "x".repeat(256);
    ensure(client.add_collection(&too_long).await.is_err(), "256-char name rejected")?;
    ok("empty and overlong names are rejected");

    // ── 3. Rename a collection ────────────────────────────────────────────────
    step("3. Rename a collection, cards follow the new id");

    client.add_cards(col, CARD_A, 2, 0, None).await?;

    let renamed = client.rename_collection(col, col_renamed).await?;
    eq(renamed.id.as_str(), col_renamed, "rename response reports the new id")?;

    let collections = client.list_collections().await?;
    ensure(!collections.iter().any(|c| c.id == col), "old name no longer listed")?;
    ensure(collections.iter().any(|c| c.id == col_renamed), "new name is listed")?;

    let cards = client.list_cards(col_renamed).await?;
    ensure(
        cards.iter().any(|c| c.id == CARD_A && c.quantity == 2),
        "cards survived the rename under the new id",
    )?;
    ok("rename preserves cards and updates the collection listing");

    // Renaming again to a name that's already the guard's registration keeps
    // cleanup correct: the guard removes both `col` and `col_renamed`, and
    // removing an already-gone `col` is a harmless no-op (verified below).

    // ── 4. Removing a collection that still holds cards succeeds ─────────────
    step("4. Removing a non-empty collection removes it (and its cards) entirely");

    let remove = client.remove_collection(col_renamed).await?;
    ensure(!remove.message.is_empty(), "remove reports a non-empty status message")?;

    let collections = client.list_collections().await?;
    ensure(!collections.iter().any(|c| c.id == col_renamed), "collection gone after removal")?;
    ok("non-empty collection removed cleanly");

    // ── 5. Removing an already-removed / never-existing collection ──────────
    step("5. Removing a collection that doesn't exist doesn't error");

    // `col` was renamed away in step 3, so it no longer exists — same shape
    // as removing an id that was never created.
    client.remove_collection(col).await?;
    ok("removing a nonexistent collection is a harmless no-op");

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
