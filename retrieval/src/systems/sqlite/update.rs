use std::{fs, path::PathBuf, str::FromStr, sync::Arc};

use tokio::sync::Mutex;

use crate::{http::DownloadProgress, mirror};

pub async fn download_mtg_db(
    path: &str,
    progress: Option<Arc<Mutex<DownloadProgress>>>,
) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2";
    const CRC_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2.sha256";

    let target = PathBuf::from_str(path)?;
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    if mirror::try_mirrors("AllPrintings.sqlite", &target, progress.as_ref()).await {
        return Ok(());
    }

    mirror::download_bz2_verified(
        DOWNLOAD_URL,
        CRC_URL,
        "AllPrintings.sqlite.bz2",
        &target,
        progress.as_ref(),
    )
    .await
}

pub async fn download_prices(path: &str) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str = "https://mtgjson.com/api/v5/AllPricesToday.sqlite.bz2";
    const CRC_URL: &str = "https://mtgjson.com/api/v5/AllPricesToday.sqlite.bz2.sha256";

    let target = PathBuf::from_str(path)?;
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    if mirror::try_mirrors("AllPricesToday.sqlite", &target, None).await {
        return Ok(());
    }

    mirror::download_bz2_verified(
        DOWNLOAD_URL,
        CRC_URL,
        "AllPricesToday.sqlite.bz2",
        &target,
        None,
    )
    .await
}
