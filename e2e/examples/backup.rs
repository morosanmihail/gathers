//! End-to-end test for full backups (`/api/settings/backup`): downloading a
//! zip of the storage and price history databases, and restoring one to
//! replace everything.
//!
//! Covers:
//!   1. the backup is a zip holding a manifest and both databases
//!   2. restoring it brings back the collections and cards as they were,
//!      dropping anything made since
//!   3. invalid uploads (not a zip, a zip that isn't a backup, a backup
//!      from a newer server) are refused and change nothing
//!
//! Deploys its own server (see `e2e::harness`), on a copy of
//! `data/testPrintings.db`, with price history on:
//!   cargo run --example backup

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};

// War Priest of Thune — M13 #39
const CARD_A: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Goblin King — 3ED #155
const CARD_B: &str = "0001e0d0-2dcd-5640-aadc-a84765cf5fc9";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("full backup and restore")?;
    let setup = ServerSetup::mtg(&harness)?.env("GATHERS_PRICE_HISTORY", "true");
    let client = harness.start_server(&setup).await?;
    let result = run(&client).await;
    harness.conclude(result)
}

async fn run(client: &GathersClient) -> eyre::Result<()> {
    step("1. Download a backup");
    client.add_collection("e2e-kept").await?;
    client.add_cards("e2e-kept", CARD_A, "", 3, None).await?;
    client.add_cards("e2e-kept", CARD_B, "foil", 1, None).await?;
    let backup = client.download_backup().await?;
    ensure(backup.starts_with(b"PK"), "backup is a zip archive")?;
    for name in ["manifest.json", "storage.db", "price_history.db"] {
        ensure(contains(&backup, name.as_bytes()), &format!("backup holds {name}"))?;
    }
    ok("backup zip holds manifest.json, storage.db and price_history.db");

    step("2. Change things, then restore the backup");
    client.add_collection("e2e-later").await?;
    client.add_cards("e2e-kept", CARD_A, "", 5, None).await?;
    let response = client.restore_backup(backup.clone()).await?;
    eq(response["price_history_restored"].as_bool(), Some(true), "price history restored")?;
    let names = collection_names(client).await?;
    ensure(names.contains(&"e2e-kept".to_string()), "backed up collection is back")?;
    ensure(!names.contains(&"e2e-later".to_string()), "collection made after the backup is gone")?;
    let mut cards: Vec<_> = client
        .list_cards("e2e-kept")
        .await?
        .into_iter()
        .map(|c| (c.id, c.finish, c.quantity))
        .collect();
    cards.sort();
    let mut expected = vec![
        (CARD_A.to_string(), String::new(), 3),
        (CARD_B.to_string(), "foil".to_string(), 1),
    ];
    expected.sort();
    eq(cards, expected, "cards as they were at backup time")?;
    client.add_cards("e2e-kept", CARD_A, "", 1, None).await?;
    ok("restore replaced the data, and the server keeps working on it");

    step("3. Invalid uploads are refused and change nothing");
    let before = collection_names(client).await?;
    ensure(client.restore_backup(b"not a zip".to_vec()).await.is_err(), "non-zip refused")?;
    let newer = patch_manifest_version(&backup)?;
    let err = client.restore_backup(newer).await.err().map(|e| e.to_string()).unwrap_or_default();
    ensure(err.contains("newer than this server supports"), &format!("newer backup refused, got: {err}"))?;
    eq(collection_names(client).await?, before, "collections unchanged after refused uploads")?;
    ok("garbage and too-new backups are refused with nothing changed");
    Ok(())
}

async fn collection_names(client: &GathersClient) -> eyre::Result<Vec<String>> {
    let mut names: Vec<_> = client.list_collections().await?.into_iter().map(|c| c.id).collect();
    names.sort();
    Ok(names)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Rebuilds the backup with its manifest claiming a far newer storage schema.
fn patch_manifest_version(backup: &[u8]) -> eyre::Result<Vec<u8>> {
    use std::io::{Read as _, Write as _};
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(backup))?;
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut entry = src.by_index(i)?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        if entry.name() == "manifest.json" {
            let mut manifest: serde_json::Value = serde_json::from_slice(&data)?;
            manifest["storage_schema_version"] = 9999.into();
            data = serde_json::to_vec(&manifest)?;
        }
        out.start_file(entry.name(), zip::write::SimpleFileOptions::default())?;
        out.write_all(&data)?;
    }
    Ok(out.finish()?.into_inner())
}

fn ensure(cond: bool, msg: &str) -> eyre::Result<()> {
    if cond { Ok(()) } else { Err(eyre::eyre!("assertion failed: {msg}")) }
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
