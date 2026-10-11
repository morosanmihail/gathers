//! End-to-end test for filtering by card type, and for each game's search
//! metadata:
//!   - `/api/{game}/meta` lists a game's sets, the card types it can be
//!     filtered by and its grouping (`unique`) modes, in one call.
//!   - The `any_types` search filter keeps a card having any one of the
//!     selected types: Artifact + Enchantment finds artifacts and
//!     enchantments alike. MTG matches its `types` (tokens included),
//!     Riftbound its `type`, Pokemon its `cardType`, case aside.
//!   - The same filter narrows a collection holding several games' cards.
//!
//! Deploys its own server (see `e2e::harness`) on copies of
//! `data/testPrintings.db`, `data/riftbound.db` and `data/pokemon.db`:
//!   cargo run --example card_types

use std::collections::HashSet;

use e2e::GathersClient;
use e2e::harness::{Harness, ServerSetup};
use serde_json::{Value, json};

const MTG: &str = "MagicSQLite";
const POKEMON: &str = "PokemonSQLite";
const RIFTBOUND: &str = "RiftboundSQLite";

// War Priest of Thune — M13 #39, a creature
const MTG_CREATURE: &str = "0005d268-3fd0-5424-bc6b-573ecd713aa1";
// Rule of Law — M20 #35, an enchantment
const MTG_ENCHANTMENT: &str = "0004a4fb-92c6-59b2-bdbe-ceb584a9e401";
// Treasure — TM20 #10, a token artifact
const MTG_TOKEN: &str = "0060ce13-67e2-5607-a29b-721c743e6770";
// Primordial Altar, a stadium
const POKEMON_STADIUM: &str = "Silver-Tempest-Primordial-Altar-161";
// V Guard Energy
const POKEMON_ENERGY: &str = "Silver-Tempest-V-Guard-Energy-169";
// Altar of Memories, a gear
const RIFTBOUND_GEAR: &str = "sfd-169-221";
// Adaptatron, a unit
const RIFTBOUND_UNIT: &str = "ogn-056-298";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> eyre::Result<()> {
    let mut harness = Harness::new("card types")?;
    let mtg_db = harness.copy_data_file("testPrintings.db", "db/AllPrintings.db")?;
    let riftbound_db = harness.copy_data_file("riftbound.db", "db/riftbound.db")?;
    let pokemon_db = harness.copy_data_file("pokemon.db", "db/pokemon.db")?;
    let setup = ServerSetup::new()?
        .env("GATHERS_SYSTEMS", "sql,pokemon-sql,riftbound-sql")
        .env("MTG_DB_PATH", mtg_db.to_string_lossy())
        .env("RIFTBOUND_DB_PATH", riftbound_db.to_string_lossy())
        .env("POKEMON_DB_PATH", pokemon_db.to_string_lossy())
        .expect_system(MTG)
        .expect_system(POKEMON)
        .expect_system(RIFTBOUND);
    let client = harness.start_server(&setup).await?;
    let result = run(&client).await;
    harness.conclude(result)
}

async fn run(client: &GathersClient) -> eyre::Result<()> {
    // ── 1. Metadata ─────────────────────────────────────────────────────────
    step("1. Each game's metadata in one call");

    let mtg = client.meta("mtg").await?;
    ensure(mtg.sets.iter().any(|s| s.code == "M13"), "MTG sets listed")?;
    // The fixture has too few printings of anything else to pass the
    // threshold that keeps real data's one-off joke types out.
    eq(mtg.card_types.clone(), strings(&["Creature"]), "MTG card types")?;
    let modes: Vec<&str> = mtg.unique_modes.iter().map(|m| m.id.as_str()).collect();
    eq(modes, vec!["prints", "cards", "art"], "MTG grouping modes")?;

    let riftbound = client.meta("riftbound").await?;
    ensure(!riftbound.sets.is_empty(), "Riftbound sets listed")?;
    eq(
        riftbound.card_types.clone(),
        strings(&["Battlefield", "Gear", "Legend", "Rune", "Spell", "Unit"]),
        "Riftbound card types",
    )?;

    let pokemon = client.meta("pokemon").await?;
    ensure(!pokemon.sets.is_empty(), "Pokemon sets listed")?;
    eq(
        pokemon.card_types.clone(),
        strings(&["Energy", "Item", "Pokemon", "Stadium", "Supporter", "Tool", "Trainer"]),
        "Pokemon card types",
    )?;
    ok("sets, card types and grouping modes for every game");

    // ── 2. Search ───────────────────────────────────────────────────────────
    step("2. Searching by several types finds cards of any of them");

    let cards = client.search("mtg", &json!({ "anyTypes": ["Enchantment", "Token"] }), 100).await?;
    let lines: Vec<&str> = cards.iter().map(|c| str_field(c, "typeLine")).collect();
    ensure(lines.iter().all(|l| l.contains("Enchantment") || l.starts_with("Token")), "only enchantments and tokens")?;
    ensure(lines.iter().any(|l| l.contains("Enchantment")), "enchantments found")?;
    ensure(lines.iter().any(|l| l.starts_with("Token")), "tokens found")?;
    ok("MTG: enchantments and tokens");

    let cards = client.search("riftbound", &json!({ "anyTypes": ["gear", "Rune"] }), 1000).await?;
    eq(types_of(&cards), set(&["Gear", "Rune"]), "Riftbound result types")?;
    eq(cards.len(), 91, "every gear (79) and rune (12)")?;
    ok("Riftbound: gear and runes, case aside");

    let cards = client.search("pokemon", &json!({ "anyTypes": ["Stadium", "Energy"] }), 1000).await?;
    eq(types_of(&cards), set(&["Stadium", "Energy"]), "Pokemon result types")?;
    ok("Pokemon: stadiums and energy");

    // ── 3. Collection ───────────────────────────────────────────────────────
    step("3. Filtering a collection of several games by type");

    let col = "e2e-card-types";
    client.add_collection(col).await?;
    for (id, provider) in [
        (MTG_CREATURE, MTG),
        (MTG_ENCHANTMENT, MTG),
        (MTG_TOKEN, MTG),
        (POKEMON_STADIUM, POKEMON),
        (POKEMON_ENERGY, POKEMON),
        (RIFTBOUND_GEAR, RIFTBOUND),
        (RIFTBOUND_UNIT, RIFTBOUND),
    ] {
        client.add_cards_with_provider(col, id, "", 1, None, Some(provider)).await?;
    }

    let found = client
        .search_collection(col, &json!({ "anyTypes": ["Enchantment", "Artifact", "Stadium", "gear"] }))
        .await?;
    let ids: HashSet<&str> = found.iter().map(|c| c.id.as_str()).collect();
    eq(
        ids,
        HashSet::from([MTG_ENCHANTMENT, MTG_TOKEN, POKEMON_STADIUM, RIFTBOUND_GEAR]),
        "collection entries of any selected type, from every game",
    )?;
    ok("a collection filters by type across games");

    println!("\n✓ All assertions passed");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn str_field<'a>(card: &'a Value, field: &str) -> &'a str {
    card.get(field).and_then(Value::as_str).unwrap_or_default()
}

fn types_of(cards: &[Value]) -> HashSet<String> {
    cards.iter().map(|c| str_field(c, "cardType").to_string()).collect()
}

fn set(values: &[&str]) -> HashSet<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
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
