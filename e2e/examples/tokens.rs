//! End-to-end test for tokens, which both MTG systems keep apart from cards:
//!   - MTGJSON (`sql`) stores them in a `tokens` table of their own, so
//!     anything not found in `cards` is looked up there before it counts as
//!     not found.
//!   - Scryfall leaves them out of a search unless it asks for them, so every
//!     search does, and a set/number lookup that finds nothing is retried
//!     with `t:token`.
//!
//! Covers, for each system:
//!   1. looking a token up by id
//!   2. a search finds tokens alongside cards, sorted in among them
//!   3. importing a CSV of cards and tokens resolves both, whatever the
//!      case of their set codes
//!   4. (sql) adding a token without naming its provider resolves the
//!      provider, as for a card
//!
//! Deploys its own servers (see `e2e::harness`): MagicSQLite on a copy of
//! `data/testPrintings.db`, then Scryfall, which needs network access to
//! api.scryfall.com:
//!   cargo run --example tokens

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};
use e2e::models::CollectionCard;
use serde_json::json;

// MTGJSON uuids, from `data/testPrintings.db`.
// War Priest of Thune — M13 #39, a card
const SQL_WAR_PRIEST: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Goblin — TM13 #6, a vanilla token
const SQL_GOBLIN: &str = "a8d5380a-d799-5464-ae84-8cce56f4b336";
// Goblin King — 3ED #155, a card
const SQL_GOBLIN_KING: &str = "0001e0d0-2dcd-5640-aadc-a84765cf5fc9";
// Drake — TM13 #4
const SQL_DRAKE: &str = "67f59d92-2f7e-51ea-9fb9-298bdca35cf3";
// Treasure — TM20 #10
const SQL_TREASURE: &str = "0060ce13-67e2-5607-a29b-721c743e6770";
// Punchcard — TAKH #26, a double-faced token (front face)
const SQL_PUNCHCARD: &str = "1608f092-2e09-5f2d-99bf-421ef3415d73";

// Scryfall ids of the same printings.
const SCRYFALL_WAR_PRIEST: &str = "d28eb320-aea7-466e-8718-de8652a2b191";
const SCRYFALL_GOBLIN: &str = "0e67efea-8a80-42ec-8e77-07d387d933d4";
const SCRYFALL_DRAKE: &str = "93679bb9-ee1c-4eea-bcdd-72785d5788af";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("tokens")?;
    let result = run(&mut harness).await;
    harness.conclude(result)
}

async fn run(harness: &mut Harness) -> eyre::Result<()> {
    let setup = ServerSetup::mtg(harness)?;
    let client = harness.start_server(&setup).await?;
    sql(&client).await?;
    harness.stop("server");

    let setup = ServerSetup::new()?
        .env("GATHERS_SYSTEMS", "scryfall")
        .expect_system("Scryfall");
    let client = harness.start_server(&setup).await?;
    scryfall(&client).await?;

    println!("\n✓ All assertions passed");
    Ok(())
}

async fn sql(client: &GathersClient) -> eyre::Result<()> {
    // ── 1. Tokens by id ─────────────────────────────────────────────────────
    step("sql 1. Look up a card and a token by id");

    let found = client.mtg_cards(&[SQL_WAR_PRIEST, SQL_GOBLIN, "not-a-card"]).await?;
    eq(found.len(), 2, "the card and the token are found, the bogus id isn't")?;
    eq(found[SQL_WAR_PRIEST].name.as_str(), "War Priest of Thune", "card name")?;
    let goblin = &found[SQL_GOBLIN];
    eq(goblin.name.as_str(), "Goblin", "token name")?;
    eq(goblin.set_code.as_str(), "TM13", "token set")?;
    eq(goblin.set_name.as_str(), "Magic 2013 Tokens", "token set name")?;
    eq(goblin.type_line.as_str(), "Token Creature — Goblin", "token type line")?;
    ok("tokens resolve by id alongside cards");

    // ── 2. Search ───────────────────────────────────────────────────────────
    step("sql 2. Search finds tokens alongside cards");

    let drakes = client.search_mtg(&json!({ "name": "Drake" }), 50).await?;
    eq(ids(&drakes), vec![SQL_DRAKE], "a name only a token has finds the token")?;

    let tm13 = client.search_mtg(&json!({ "setCode": "tm13" }), 50).await?;
    eq(ids(&tm13), vec![SQL_DRAKE, SQL_GOBLIN], "a token set's tokens, by name")?;

    let goblins = client.search_mtg(&json!({ "name": "Goblin" }), 50).await?;
    eq(ids(&goblins), vec![SQL_GOBLIN, SQL_GOBLIN_KING], "the Goblin token, then Goblin King")?;
    ok("search finds tokens and cards together, in sort order");

    // ── 3. CSV import ───────────────────────────────────────────────────────
    step("sql 3. Import a CSV of cards and tokens");

    let col = "e2e-tokens-sql";
    client
        .import_csv(
            col,
            "Set,CollectorNumber,Quantity,FoilQuantity\n\
             M13,39,2,0\n\
             TM13,6,3,1\n\
             TAKH,26,1,0\n\
             TM13,999,1,0\n",
        )
        .await?;
    let cards = client.list_cards(col).await?;
    eq(cards.len(), 4, "war priest + goblin (two finishes) + punchcard rows")?;
    eq(find(&cards, SQL_WAR_PRIEST, "")?.quantity, 2, "card imported")?;
    eq(find(&cards, SQL_GOBLIN, "")?.quantity, 3, "token imported")?;
    eq(find(&cards, SQL_GOBLIN, "foil")?.quantity, 1, "foil token imported")?;
    eq(find(&cards, SQL_PUNCHCARD, "")?.quantity, 1, "double-faced token imported as its front face")?;
    ensure(cards.iter().all(|c| c.provider == "MagicSQLite"), "every row belongs to MagicSQLite")?;
    ok("cards and tokens both resolve on import; unknown numbers are still skipped");

    // ── 4. Provider resolution ──────────────────────────────────────────────
    step("sql 4. Add a token without naming its provider");

    let col = "e2e-tokens-sql-add";
    client.add_collection(col).await?;
    client.add_cards(col, SQL_TREASURE, "", 2, None).await?;
    let cards = client.list_cards(col).await?;
    let treasure = find(&cards, SQL_TREASURE, "")?;
    eq(treasure.quantity, 2, "token added")?;
    eq(treasure.provider.as_str(), "MagicSQLite", "provider resolved for a token")?;
    ok("a token's provider is resolved like a card's");

    Ok(())
}

async fn scryfall(client: &GathersClient) -> eyre::Result<()> {
    // ── 1. Tokens by id ─────────────────────────────────────────────────────
    step("scryfall 1. Look up a token by id");

    let found = client.mtg_cards(&[SCRYFALL_GOBLIN]).await?;
    eq(found.get(SCRYFALL_GOBLIN).map(|c| c.name.as_str()), Some("Goblin"), "token by id")?;
    ok("tokens resolve by id");

    // ── 2. Search ───────────────────────────────────────────────────────────
    step("scryfall 2. Search finds tokens alongside cards");

    let spawns = client.search_mtg(&json!({ "name": "Eldrazi Spawn" }), 50).await?;
    ensure(!spawns.is_empty(), "a name only tokens have finds tokens")?;
    ensure(
        spawns.iter().all(|c| c.name == "Eldrazi Spawn" && c.type_line.starts_with("Token")),
        "only Eldrazi Spawn tokens are found",
    )?;

    let goblins = client.search_mtg(&json!({ "name": "Goblin" }), 175).await?;
    ensure(goblins.iter().any(|c| c.type_line.starts_with("Token")), "Goblin tokens found")?;
    ensure(goblins.iter().any(|c| !c.type_line.starts_with("Token")), "Goblin cards found")?;

    let meta = client.meta("mtg").await?;
    ensure(
        ["Artifact", "Creature", "Token"].iter().all(|t| meta.card_types.iter().any(|c| c == t)),
        "Scryfall's card types include Token",
    )?;
    ok("search finds tokens and cards together; Token is a card type");

    // ── 3. CSV import ───────────────────────────────────────────────────────
    step("scryfall 3. Import a CSV of cards and tokens");

    let col = "e2e-tokens-scryfall";
    client
        .import_csv(
            col,
            "Set,CollectorNumber,Quantity,FoilQuantity\n\
             M13,39,2,0\n\
             TM13,6,3,0\n\
             tm13,4,1,0\n\
             TM13,999,1,0\n",
        )
        .await?;
    let cards = client.list_cards(col).await?;
    eq(cards.len(), 3, "war priest, goblin and drake rows")?;
    eq(find(&cards, SCRYFALL_WAR_PRIEST, "")?.quantity, 2, "card imported from an upper-case set code")?;
    eq(find(&cards, SCRYFALL_GOBLIN, "")?.quantity, 3, "token imported")?;
    eq(find(&cards, SCRYFALL_DRAKE, "")?.quantity, 1, "token imported from a lower-case set code")?;
    ensure(cards.iter().all(|c| c.provider == "Scryfall"), "every row belongs to Scryfall")?;
    ok("cards and tokens both resolve on import");

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn ids(cards: &[e2e::models::MtgCard]) -> Vec<&str> {
    cards.iter().map(|c| c.id.as_str()).collect()
}

fn find<'a>(cards: &'a [CollectionCard], id: &str, finish: &str) -> eyre::Result<&'a CollectionCard> {
    cards
        .iter()
        .find(|c| c.id == id && c.finish == finish)
        .ok_or_else(|| eyre::eyre!("card '{id}' (finish {finish:?}) not found in collection"))
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
