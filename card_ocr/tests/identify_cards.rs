/// Integration tests using the card images in data/cards/.
///
/// Requires AllPrintings.db (set MTG_DB_PATH or rely on the default
/// ~/.local/share/gathers/DB/AllPrintings.db). Tests that cannot find the
/// database are skipped rather than failed so they pass in CI without the
/// full database.
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
    let default = {
        let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home).join(".local/share/gathers/DB/AllPrintings.db")
    };
    if default.exists() { Some(default) } else { None }
}

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/cards")
}

async fn make_retrieval(db: &Path) -> MagicSQLiteRetrievalSystem {
    MagicSQLiteRetrievalSystem::new(Some(db.to_string_lossy().into_owned()), None)
        .expect("Failed to open database")
}

fn assert_magic(card: Option<Card>, expected_name: &str, expected_set: &str, expected_number: &str) {
    let card =
        card.unwrap_or_else(|| panic!("Expected card {:?} but got None", expected_name));
    match card {
        Card::Magic(c) => {
            assert_eq!(c.name, expected_name, "Wrong card name");
            assert_eq!(
                c.set_code.to_uppercase(),
                expected_set.to_uppercase(),
                "Wrong set code"
            );
            assert_eq!(c.collector_number, expected_number, "Wrong collector number");
        }
        _ => panic!("Expected a Magic card"),
    }
}

#[tokio::test]
async fn test_identify_rooftop_percher() {
    let Some(db) = db_path() else {
        eprintln!("SKIP: AllPrintings.db not found");
        return;
    };
    let retrieval = make_retrieval(&db).await;
    let card = identify_card(&cards_dir().join("ecl-2-rooftop-percher.jpg"), &retrieval)
        .await
        .expect("identify_card returned an error");
    assert_magic(card, "Rooftop Percher", "ECL", "2");
}

#[tokio::test]
async fn test_identify_ajani_outland_chaperone() {
    let Some(db) = db_path() else {
        eprintln!("SKIP: AllPrintings.db not found");
        return;
    };
    let retrieval = make_retrieval(&db).await;
    let card = identify_card(
        &cards_dir().join("ecl-4-ajani-outland-chaperone.jpg"),
        &retrieval,
    )
    .await
    .expect("identify_card returned an error");
    assert_magic(card, "Ajani, Outland Chaperone", "ECL", "4");
}

#[tokio::test]
async fn test_identify_bark_of_doran() {
    let Some(db) = db_path() else {
        eprintln!("SKIP: AllPrintings.db not found");
        return;
    };
    let retrieval = make_retrieval(&db).await;
    let card = identify_card(&cards_dir().join("ecl-6-bark-of-doran.jpg"), &retrieval)
        .await
        .expect("identify_card returned an error");
    assert_magic(card, "Bark of Doran", "ECL", "6");
}

#[tokio::test]
async fn test_identify_reluctant_dounguard() {
    let Some(db) = db_path() else {
        eprintln!("SKIP: AllPrintings.db not found");
        return;
    };
    let retrieval = make_retrieval(&db).await;
    let card = identify_card(&cards_dir().join("ecl-31-reluctant-dounguard.jpg"), &retrieval)
        .await
        .expect("identify_card returned an error");
    assert_magic(card, "Reluctant Dounguard", "ECL", "31");
}

#[tokio::test]
async fn test_identify_deepchannel_duelist() {
    let Some(db) = db_path() else {
        eprintln!("SKIP: AllPrintings.db not found");
        return;
    };
    let retrieval = make_retrieval(&db).await;
    let card =
        identify_card(&cards_dir().join("ecl-213-deepchannel-duelist.jpg"), &retrieval)
            .await
            .expect("identify_card returned an error");
    assert_magic(card, "Deepchannel Duelist", "ECL", "213");
}
