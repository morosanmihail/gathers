use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::info;

use crate::http::{DownloadProgress, stream_to_file};

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

    download_bz2_verified(
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

    download_bz2_verified(DOWNLOAD_URL, CRC_URL, "AllPricesToday.sqlite.bz2", &target, None).await
}

pub(super) async fn download_bz2_verified(
    download_url: &str,
    crc_url: &str,
    bz2_filename: &str,
    target: &Path,
    progress: Option<&Arc<Mutex<DownloadProgress>>>,
) -> eyre::Result<()> {
    let sidecar_path = PathBuf::from(format!("{}.bz2.sha256", target.display()));
    let temp_dir = tempfile::tempdir()?;

    let crc_path = temp_dir.path().join("remote.sha256");
    stream_to_file(crc_url, "SHA256 fetched", &crc_path, progress, "checking").await?;
    let remote_crc = fs::read_to_string(&crc_path)?.trim().to_lowercase();

    let local_crc = if sidecar_path.exists() {
        fs::read_to_string(&sidecar_path)?.trim().to_lowercase()
    } else {
        String::new()
    };

    if target.exists() && local_crc == remote_crc {
        info!(file = bz2_filename, crc = %remote_crc, "Already up to date");
        return Ok(());
    }

    info!(file = bz2_filename, dest = ?target, "Downloading");
    let bz2_path = temp_dir.path().join(bz2_filename);
    stream_to_file(download_url, "Download complete", &bz2_path, progress, "downloading").await?;

    if let Some(p) = progress {
        p.lock().await.phase = "verifying".to_string();
    }
    let downloaded_crc = calculate_sha256(&bz2_path)?;
    if downloaded_crc != remote_crc {
        eyre::bail!(
            "{bz2_filename} CRC mismatch after download: expected {remote_crc}, got {downloaded_crc}"
        );
    }

    if let Some(p) = progress {
        p.lock().await.phase = "decompressing".to_string();
    }
    decompress_bz2(&bz2_path, target)?;
    fs::write(&sidecar_path, &downloaded_crc)?;
    info!(file = bz2_filename, crc = %downloaded_crc, "Downloaded, verified, and decompressed");
    Ok(())
}

pub(super) fn calculate_sha256(path: &Path) -> eyre::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

pub(super) fn decompress_bz2(src: &Path, dst: &Path) -> eyre::Result<()> {
    let file = fs::File::open(src)?;
    let mut decoder = BzDecoder::new(file);
    let mut out = fs::File::create(dst)?;
    std::io::copy(&mut decoder, &mut out)?;
    Ok(())
}
