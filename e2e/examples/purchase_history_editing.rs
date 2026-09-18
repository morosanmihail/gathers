//! End-to-end test for direct purchase history entry editing:
//!   PATCH /api/collection/cards/{id}/purchase_history_entry/{entry_id}
//!   DELETE /api/collection/cards/{id}/purchase_history_entry/{entry_id}
//!
//! `collection_lifecycle.rs` exercises history as a side effect of
//! adding/removing/moving cards; this test drives entry edits directly,
//! including the validation path that rejects recording more copies in
//! history than the collection actually owns.
//!
//! Run against a live server:
//!   cargo run --example purchase_history_editing
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example purchase_history_editing

use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs purchase history editing e2e ===");
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
    let col = format!("e2e-purchedit-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col);

    let result = run(&client, &col).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col: &str) -> eyre::Result<()> {
    client.add_collection(col).await?;

    // ── 1. Create a purchase entry, then edit its price in place ────────────
    step("1. Edit an existing entry's price");

    client.add_cards(col, CARD_A, 3, 0, Some(4.00)).await?;
    let hist = client.purchase_history(col, CARD_A).await?;
    eq(hist.entries.len(), 1, "one purchase entry recorded")?;
    let entry_id = hist.entries[0].id;
    eq(hist.entries[0].normal_price_per_unit, Some(4.00), "initial price")?;

    let (status, _) = client
        .update_purchase_entry(col, entry_id, 3, 0, Some(6.50), None)
        .await?;
    ensure(status == 204, &format!("update returns 204, got {status}"))?;

    let hist = client.purchase_history(col, CARD_A).await?;
    eq(hist.entries.len(), 1, "still one entry after edit")?;
    eq(hist.entries[0].normal_price_per_unit, Some(6.50), "price updated in place")?;
    eq(hist.entries[0].quantity, 3, "quantity unchanged by the price-only edit")?;
    ok("entry price updated in place, quantity untouched");

    // ── 2. Editing quantity within owned bounds succeeds ─────────────────────
    step("2. Editing an entry's recorded quantity down (still within owned copies)");

    let (status, _) = client
        .update_purchase_entry(col, entry_id, 2, 0, Some(6.50), None)
        .await?;
    ensure(status == 204, &format!("update returns 204, got {status}"))?;
    let hist = client.purchase_history(col, CARD_A).await?;
    eq(hist.entries[0].quantity, 2, "entry quantity reduced to 2")?;
    ok("entry quantity edited within owned bounds");

    // ── 3. Editing quantity beyond what's owned is rejected ─────────────────
    step("3. Recording more history copies than the collection owns is rejected");

    // Collection owns 3 copies of CARD_A total; claiming 10 in this single
    // entry (with no other entries) exceeds that.
    let (status, body) = client
        .update_purchase_entry(col, entry_id, 10, 0, Some(6.50), None)
        .await?;
    eq(status.as_u16(), 400, "over-claiming quantity is rejected with 400")?;
    ensure(!body.is_empty(), "validation error includes a message body")?;

    let hist = client.purchase_history(col, CARD_A).await?;
    eq(hist.entries[0].quantity, 2, "rejected edit did not change the stored entry")?;
    ok("over-claiming history quantity is rejected and leaves the entry untouched");

    // ── 4. Updating a nonexistent entry returns 404 ──────────────────────────
    step("4. Updating a nonexistent entry id returns 404");

    let (status, _) = client
        .update_purchase_entry(col, 999_999_999, 1, 0, Some(1.0), None)
        .await?;
    eq(status.as_u16(), 404, "nonexistent entry update returns 404")?;
    ok("404 for a nonexistent entry id");

    // ── 5. Delete the entry ───────────────────────────────────────────────────
    step("5. Delete the entry");

    let status = client.delete_purchase_entry(col, entry_id).await?;
    eq(status.as_u16(), 204, "delete returns 204")?;

    let hist = client.purchase_history(col, CARD_A).await?;
    ensure(hist.entries.is_empty(), "entry gone after delete")?;
    ok("entry deleted");

    // ── 6. Deleting an already-deleted entry returns 404 ────────────────────
    step("6. Deleting the same entry again returns 404");

    let status = client.delete_purchase_entry(col, entry_id).await?;
    eq(status.as_u16(), 404, "second delete of the same entry returns 404")?;
    ok("second delete returns 404, not a silent success");

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
