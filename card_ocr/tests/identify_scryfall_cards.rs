/// Integration tests using card images downloaded from Scryfall (data/cards/).
///
/// Images were fetched via https://api.scryfall.com/cards/{id}?format=image
/// and cover multiple MTG frame eras:
///   Modern 4-digit (post-2022): LCI, MKC, BLC, FDN, EOC
///   NNN/NNN fraction (2015-2021): DDS, OGW, BFZ, KHM, CMA, C19, DMU
///   Old / digital (no SET•EN line): RTR, TSP, HBG  → name fallback
///
/// Tests requiring AllPrintings.db are skipped when the database is absent.
use card_ocr::identify_card;
use models::Card;
use retrieval::MagicSQLiteRetrievalSystem;
use std::{
    env,
    path::{Path, PathBuf},
};

fn db_path() -> Option<PathBuf> {
    if let Ok(p) = env::var("MTG_DB_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default = PathBuf::from(home).join(".local/share/gathers/DB/AllPrintings.db");
    if default.exists() { Some(default) } else { None }
}

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/cards")
}

async fn retrieval(db: &Path) -> MagicSQLiteRetrievalSystem {
    MagicSQLiteRetrievalSystem::new(Some(db.to_string_lossy().into_owned()), None)
        .expect("Failed to open database")
}

/// Assert the card matches by set code and collector number.
fn assert_exact(card: Option<Card>, name: &str, set: &str, number: &str) {
    let card = card.unwrap_or_else(|| panic!("Expected {name:?} but got None"));
    match &card {
        Card::Magic(c) => {
            assert_eq!(c.set_code.to_uppercase(), set.to_uppercase(), "{name}: wrong set");
            assert_eq!(c.collector_number, number, "{name}: wrong collector number");
        }
        _ => panic!("{name}: expected Magic card"),
    }
}

/// Assert the card's name matches (used where name fallback is the only option
/// and multiple printings of the same card exist in the DB).
fn assert_name(card: Option<Card>, expected_name: &str) {
    let card = card.unwrap_or_else(|| panic!("Expected {expected_name:?} but got None"));
    let actual = match &card {
        Card::Magic(c) => c.name.as_str(),
        Card::Riftbound(c) => c.name.as_str(),
        Card::Pokemon(c) => c.name.as_str(),
    };
    assert_eq!(actual, expected_name, "Wrong card name");
}

// ── Modern 4-digit frames ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_mkc_303_temple_of_mystery() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("mkc-303-temple-of-mystery.jpg"), &r).await.unwrap();
    assert_exact(card, "Temple of Mystery", "MKC", "303");
}

#[tokio::test]
async fn test_blc_5_arthur_marigold_knight() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("blc-5-arthur-marigold-knight.jpg"), &r).await.unwrap();
    assert_exact(card, "Arthur, Marigold Knight", "BLC", "5");
}

#[tokio::test]
async fn test_lci_126_tarrians_journal() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("lci-126-tarrians-journal.jpg"), &r).await.unwrap();
    assert_exact(card, "Tarrian's Journal // The Tomb of Aclazotz", "LCI", "126");
}

#[tokio::test]
async fn test_fdn_251_campus_guide() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("fdn-251-campus-guide.jpg"), &r).await.unwrap();
    assert_exact(card, "Campus Guide", "FDN", "251");
}

#[tokio::test]
async fn test_eoc_35_horizon_explorer() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("eoc-35-horizon-explorer.jpg"), &r).await.unwrap();
    assert_exact(card, "Horizon Explorer", "EOC", "35");
}

// ── NNN/NNN fraction frames ───────────────────────────────────────────────────

#[tokio::test]
async fn test_dds_30_island() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("dds-30-island.jpg"), &r).await.unwrap();
    assert_exact(card, "Island", "DDS", "30");
}

#[tokio::test]
async fn test_ogw_150_void_grafter() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("ogw-150-void-grafter.jpg"), &r).await.unwrap();
    assert_exact(card, "Void Grafter", "OGW", "150");
}

#[tokio::test]
async fn test_bfz_186_retreat_to_kazandu() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("bfz-186-retreat-to-kazandu.jpg"), &r).await.unwrap();
    assert_exact(card, "Retreat to Kazandu", "BFZ", "186");
}

#[tokio::test]
async fn test_khm_58_frostpyre_arcanist() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("khm-58-frostpyre-arcanist.jpg"), &r).await.unwrap();
    assert_exact(card, "Frostpyre Arcanist", "KHM", "58");
}

#[tokio::test]
async fn test_cma_301_swamp() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("cma-301-swamp.jpg"), &r).await.unwrap();
    assert_exact(card, "Swamp", "CMA", "301");
}

#[tokio::test]
async fn test_c19_272_selesnya_sanctuary() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("c19-272-selesnya-sanctuary.jpg"), &r).await.unwrap();
    assert_exact(card, "Selesnya Sanctuary", "C19", "272");
}

#[tokio::test]
async fn test_dmu_188_the_weatherseed_treaty() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("dmu-188-the-weatherseed-treaty.jpg"), &r).await.unwrap();
    assert_exact(card, "The Weatherseed Treaty", "DMU", "188");
}

// ── Old frame / digital — name fallback ──────────────────────────────────────

#[tokio::test]
async fn test_rtr_53_stealer_of_secrets() {
    // RTR 2012 frame lacks the "RTR•EN" bottom-strip pattern; the name fallback
    // may return a different printing of the card.
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("rtr-53-stealer-of-secrets.jpg"), &r).await.unwrap();
    assert_name(card, "Stealer of Secrets");
}

#[tokio::test]
async fn test_hbg_57_incessant_provocation() {
    // HBG is a digital-only Alchemy set with no standard bottom strip.
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("hbg-57-incessant-provocation.jpg"), &r).await.unwrap();
    // Name is unique in the DB so either approach returns the right card.
    assert_exact(card, "Incessant Provocation", "HBG", "57");
}

#[tokio::test]
async fn test_tsp_13_divine_congregation() {
    // TSP 2006 old frame lacks the "TSP•EN" pattern; name fallback is reliable
    // because "Divine Congregation" has exactly one printing in the DB.
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("tsp-13-divine-congregation.jpg"), &r).await.unwrap();
    assert_exact(card, "Divine Congregation", "TSP", "13");
}

// ── Extended set ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_emn_62_geist_of_the_archives() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("emn-62-geist-of-the-archives.jpg"), &r).await.unwrap();
    assert_exact(card, "Geist of the Archives", "EMN", "62");
}

#[tokio::test]
async fn test_mh1_186_treetop_ambusher() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("mh1-186-treetop-ambusher.jpg"), &r).await.unwrap();
    assert_exact(card, "Treetop Ambusher", "MH1", "186");
}

#[tokio::test]
async fn test_mid_74_shipwreck_sifters() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("mid-74-shipwreck-sifters.jpg"), &r).await.unwrap();
    assert_exact(card, "Shipwreck Sifters", "MID", "74");
}

#[tokio::test]
async fn test_blb_20_lifecreed_duo() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("blb-20-lifecreed-duo.jpg"), &r).await.unwrap();
    assert_exact(card, "Lifecreed Duo", "BLB", "20");
}

#[tokio::test]
async fn test_mkm_201_evidence_examiner() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("mkm-201-evidence-examiner.jpg"), &r).await.unwrap();
    assert_exact(card, "Evidence Examiner", "MKM", "201");
}

#[tokio::test]
async fn test_rna_163_combine_guildmage() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("rna-163-combine-guildmage.jpg"), &r).await.unwrap();
    assert_exact(card, "Combine Guildmage", "RNA", "163");
}

#[tokio::test]
async fn test_otj_117_caught_in_the_crossfire() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("otj-117-caught-in-the-crossfire.jpg"), &r).await.unwrap();
    assert_exact(card, "Caught in the Crossfire", "OTJ", "117");
}

#[tokio::test]
async fn test_one_110_stinging_hivemaster() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("one-110-stinging-hivemaster.jpg"), &r).await.unwrap();
    assert_exact(card, "Stinging Hivemaster", "ONE", "110");
}

#[tokio::test]
async fn test_m20_217_risen_reef() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("m20-217-risen-reef.jpg"), &r).await.unwrap();
    assert_exact(card, "Risen Reef", "M20", "217");
}

#[tokio::test]
async fn test_thb_52_kiora_bests_the_sea_god() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("thb-52-kiora-bests-the-sea-god.jpg"), &r).await.unwrap();
    assert_exact(card, "Kiora Bests the Sea God", "THB", "52");
}

#[tokio::test]
async fn test_frf_3_abzan_runemark() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("frf-3-abzan-runemark.jpg"), &r).await.unwrap();
    assert_exact(card, "Abzan Runemark", "FRF", "3");
}

#[tokio::test]
async fn test_tsr_9_blade_of_the_sixth_pride() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("tsr-9-blade-of-the-sixth-pride.jpg"), &r).await.unwrap();
    assert_exact(card, "Blade of the Sixth Pride", "TSR", "9");
}

#[tokio::test]
async fn test_grn_137_might_of_the_masses() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("grn-137-might-of-the-masses.jpg"), &r).await.unwrap();
    assert_exact(card, "Might of the Masses", "GRN", "137");
}

#[tokio::test]
async fn test_mic_135_biogenic_upgrade() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("mic-135-biogenic-upgrade.jpg"), &r).await.unwrap();
    assert_exact(card, "Biogenic Upgrade", "MIC", "135");
}

#[tokio::test]
async fn test_gpt_58_restless_bones() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("gpt-58-restless-bones.jpg"), &r).await.unwrap();
    assert_exact(card, "Restless Bones", "GPT", "58");
}

#[tokio::test]
async fn test_csp_59_gristle_grinner() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("csp-59-gristle-grinner.jpg"), &r).await.unwrap();
    assert_exact(card, "Gristle Grinner", "CSP", "59");
}

#[tokio::test]
async fn test_sok_36_eternal_dominion() {
    // SOK 2005 bottom strip sometimes OCRs without the SET•EN pair; name fallback
    // is reliable because "Eternal Dominion" has exactly one unique name in the DB.
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("sok-36-eternal-dominion.jpg"), &r).await.unwrap();
    assert_name(card, "Eternal Dominion");
}

#[tokio::test]
async fn test_c14_8_hallowed_spiritkeeper() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("c14-8-hallowed-spiritkeeper.jpg"), &r).await.unwrap();
    assert_exact(card, "Hallowed Spiritkeeper", "C14", "8");
}

#[tokio::test]
async fn test_iko_83_dead_weight() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("iko-83-dead-weight.jpg"), &r).await.unwrap();
    assert_exact(card, "Dead Weight", "IKO", "83");
}

#[tokio::test]
async fn test_dka_148_executioners_hood() {
    let Some(db) = db_path() else { eprintln!("SKIP: DB not found"); return; };
    let r = retrieval(&db).await;
    let card = identify_card(&cards_dir().join("dka-148-executioners-hood.jpg"), &r).await.unwrap();
    assert_exact(card, "Executioner's Hood", "DKA", "148");
}
