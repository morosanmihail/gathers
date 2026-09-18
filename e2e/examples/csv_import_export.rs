//! End-to-end test for CSV export/import round-tripping:
//!   GET  /api/collection/export/{id}
//!   POST /api/collection/import
//!
//! Run against a live server:
//!   cargo run --example csv_import_export
//!
//! Override the server URL:
//!   GATHERS_URL=http://localhost:5234 cargo run --example csv_import_export

use e2e::{CollectionGuard, GathersClient};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Mutilate — M13 #102
const CARD_B: &str = "c83a7592-5879-5d52-b27c-e866597b389f";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let url = std::env::var("GATHERS_URL").unwrap_or_else(|_| "http://localhost:5234".to_string());
    let client = GathersClient::new(&url);

    println!("=== GatheRs CSV import/export e2e ===");
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
    let col_src = format!("e2e-csv-src-{tag}");
    let col_imported = format!("e2e-csv-imported-{tag}");

    let mut guard = CollectionGuard::new(&client);
    guard.register(&col_src);
    guard.register(&col_imported);

    let result = run(&client, &col_src, &col_imported).await;

    drop(guard);
    result
}

async fn run(client: &GathersClient, col_src: &str, col_imported: &str) -> eyre::Result<()> {
    // ── 1. Build a source collection, export it ──────────────────────────────
    step("1. Export a collection with owned cards to CSV");

    client.add_collection(col_src).await?;
    client.add_cards(col_src, CARD_A, 3, 1, None).await?;
    client.add_cards(col_src, CARD_B, 0, 2, None).await?;

    let csv = client.export_csv(col_src).await?;
    ensure(csv.contains("Set"), "CSV has a header row")?;
    ensure(csv.contains("M13"), "CSV includes the M13 set code")?;
    let data_lines = csv.lines().skip(1).filter(|l| !l.trim().is_empty()).count();
    eq(data_lines, 2, "CSV has one data row per distinct card")?;
    ok("export produced a CSV with a header and 2 data rows");

    // ── 2. Import that CSV into a fresh collection ───────────────────────────
    step("2. Import the exported CSV into a new collection");

    client.import_csv(col_imported, &csv).await?;

    let collections = client.list_collections().await?;
    ensure(
        collections.iter().any(|c| c.id == col_imported),
        "import created the target collection",
    )?;

    let imported_cards = client.list_cards(col_imported).await?;
    eq(imported_cards.len(), 2, "2 distinct cards after import")?;

    let a = find_card(&imported_cards, CARD_A)?;
    eq(a.quantity, 3, "card A quantity round-tripped")?;
    eq(a.foil_quantity, 1, "card A foil quantity round-tripped")?;

    let b = find_card(&imported_cards, CARD_B)?;
    eq(b.quantity, 0, "card B quantity round-tripped")?;
    eq(b.foil_quantity, 2, "card B foil quantity round-tripped")?;
    ok("imported collection matches the exported quantities exactly");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn find_card<'a>(cards: &'a [e2e::models::CollectionCard], id: &str) -> eyre::Result<&'a e2e::models::CollectionCard> {
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
