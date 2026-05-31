use std::{collections::HashMap, path::PathBuf};

use ::models::{CardPrices, RetailerPrices};
use tracing::info;

use crate::http::stream_to_file;

pub(super) fn row_to_card_prices(uuid: &str, raw: f64, psa10: f64, psa9: f64) -> CardPrices {
    let mut paper = HashMap::new();
    if raw > 0.0 {
        paper.insert("raw".to_string(), RetailerPrices { normal: Some(raw), foil: None });
    }
    if psa10 > 0.0 {
        paper.insert("graded_psa10".to_string(), RetailerPrices { normal: Some(psa10), foil: None });
    }
    if psa9 > 0.0 {
        paper.insert("graded_psa9".to_string(), RetailerPrices { normal: Some(psa9), foil: None });
    }
    CardPrices { uuid: uuid.to_string(), paper }
}

pub async fn download_pokemon_prices(path: &str) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str =
        "https://github.com/poketrax/pokedata/raw/refs/heads/main/databases/prices.sqlite";

    let target = PathBuf::from(path);
    let target_parent = target.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));

    if !target_parent.exists() {
        std::fs::create_dir_all(target_parent)?;
    }

    // Download to system temp dir, then stage in target's directory for atomic rename.
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("prices.sqlite");

    info!(url = DOWNLOAD_URL, "Downloading Pokemon prices");
    stream_to_file(DOWNLOAD_URL, "Download complete", &temp_path, None, "downloading").await?;

    let mut staging = tempfile::NamedTempFile::new_in(target_parent)?;
    std::io::copy(&mut std::fs::File::open(&temp_path)?, &mut staging)?;
    staging.persist(&target).map_err(|e| e.error)?;
    info!(dest = ?target, "Pokemon prices saved");
    Ok(())
}
