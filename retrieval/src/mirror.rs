use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bzip2::{Compression, read::BzDecoder, write::BzEncoder};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::http::{DownloadProgress, stream_to_file};

#[derive(Debug, Default, Deserialize)]
struct MirrorConfig {
    #[serde(default)]
    mirrors: Vec<String>,
}

/// When `{stem}` last successfully refreshed in this `data_dir`, if ever.
/// Tracked per component so a restart re-attempts only what didn't finish
/// last time, instead of redoing everything.
pub fn last_update_for(data_dir: &Path, stem: &str) -> Option<SystemTime> {
    let secs: u64 = fs::read_to_string(data_dir.join(format!("{stem}.last_update")))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(UNIX_EPOCH + Duration::from_secs(secs))
}

fn write_last_update_for(data_dir: &Path, stem: &str) -> eyre::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    fs::write(
        data_dir.join(format!("{stem}.last_update")),
        now.to_string(),
    )?;
    Ok(())
}

fn is_fresh(data_dir: &Path, stem: &str, interval: Duration) -> bool {
    last_update_for(data_dir, stem)
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|elapsed| elapsed < interval)
}

/// Runs `fut` unless `{stem}` was successfully refreshed within `interval`,
/// recording a fresh per-component timestamp only on success — a failed or
/// cancelled refresh is retried on the next cycle, a successful one isn't
/// redone.
async fn refresh_if_stale(
    data_dir: &Path,
    stem: &str,
    interval: Duration,
    fut: impl std::future::Future<Output = eyre::Result<()>>,
) {
    if is_fresh(data_dir, stem, interval) {
        info!(stem, "Skipping, recently updated");
        return;
    }
    match fut.await {
        Ok(()) => {
            if let Err(e) = write_last_update_for(data_dir, stem) {
                warn!(stem, error = %e, "Failed to record last-update marker");
            }
        }
        Err(e) => warn!(stem, error = %e, "Failed to refresh"),
    }
}

/// Ordered list of mirror base URLs, highest priority first. Empty if no
/// `mirrors.toml` is configured — callers then fall back to the original
/// upstream source unchanged.
pub fn load_mirror_urls() -> Vec<String> {
    let path = std::env::var("GATHERS_MIRRORS_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.local/share/gathers/mirrors.toml")
    });
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str::<MirrorConfig>(&s).ok())
        .map(|c| c.mirrors)
        .unwrap_or_default()
}

/// Tries each configured mirror in order for `{stem}.bz2` (+ `.sha256`
/// sidecar). Returns `true` on first success, `false` if none are
/// configured or all of them failed.
pub async fn try_mirrors(
    stem: &str,
    target: &Path,
    progress: Option<&Arc<Mutex<DownloadProgress>>>,
) -> bool {
    for base in load_mirror_urls() {
        let base = base.trim_end_matches('/');
        let dl_url = format!("{base}/{stem}.bz2");
        let crc_url = format!("{dl_url}.sha256");
        match download_bz2_verified(&dl_url, &crc_url, &format!("{stem}.bz2"), target, progress)
            .await
        {
            Ok(()) => {
                info!(mirror = %base, stem, "Downloaded from mirror");
                return true;
            }
            Err(e) => {
                warn!(mirror = %base, stem, error = %e, "Mirror failed, trying next source");
            }
        }
    }
    false
}

pub async fn download_bz2_verified(
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
    stream_to_file(
        download_url,
        "Download complete",
        &bz2_path,
        progress,
        "downloading",
    )
    .await?;

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

pub fn calculate_sha256(path: &Path) -> eyre::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

pub fn decompress_bz2(src: &Path, dst: &Path) -> eyre::Result<()> {
    let parent = dst.parent().unwrap_or(Path::new("."));
    let file = fs::File::open(src)?;
    let mut decoder = BzDecoder::new(file);
    let mut staging = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut decoder, &mut staging)?;
    staging.persist(dst).map_err(|e| e.error)?;
    Ok(())
}

pub fn compress_bz2(src: &Path, dst: &Path) -> eyre::Result<()> {
    let parent = dst.parent().unwrap_or(Path::new("."));
    let mut input = fs::File::open(src)?;
    let mut staging = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut encoder = BzEncoder::new(&mut staging, Compression::best());
        std::io::copy(&mut input, &mut encoder)?;
        encoder.finish()?;
    }
    staging.persist(dst).map_err(|e| e.error)?;
    Ok(())
}

/// Mirror-server side: relays an upstream bz2 + sha256 sidecar pair
/// byte-for-byte into `data_dir/{stem}.bz2`, verifying the checksum before
/// committing. Used for components that already have a real upstream bz2
/// (MTG cards, MTG prices).
pub async fn cache_upstream_bz2(
    download_url: &str,
    crc_url: &str,
    stem: &str,
    data_dir: &Path,
) -> eyre::Result<()> {
    let bz2_target = data_dir.join(format!("{stem}.bz2"));
    let sidecar = data_dir.join(format!("{stem}.bz2.sha256"));
    let temp_dir = tempfile::tempdir()?;

    let crc_path = temp_dir.path().join("remote.sha256");
    stream_to_file(crc_url, "SHA256 fetched", &crc_path, None, "checking").await?;
    let remote_crc = fs::read_to_string(&crc_path)?.trim().to_lowercase();

    let bz2_path = temp_dir.path().join(format!("{stem}.bz2"));
    stream_to_file(
        download_url,
        "Download complete",
        &bz2_path,
        None,
        "downloading",
    )
    .await?;
    let downloaded_crc = calculate_sha256(&bz2_path)?;
    if downloaded_crc != remote_crc {
        eyre::bail!("{stem}.bz2 CRC mismatch: expected {remote_crc}, got {downloaded_crc}");
    }

    let mut staging = tempfile::NamedTempFile::new_in(data_dir)?;
    std::io::copy(&mut fs::File::open(&bz2_path)?, &mut staging)?;
    staging.persist(&bz2_target).map_err(|e| e.error)?;
    fs::write(&sidecar, &downloaded_crc)?;
    info!(stem, crc = %downloaded_crc, "Mirror cached upstream bz2");
    Ok(())
}

/// Mirror-server side: for components the mirror compresses itself
/// (Pokémon prices, Riftbound cards, Pokémon cards). Computes the sha256 of
/// the given bz2, publishes it into `data_dir/{stem}.bz2`, writes the
/// sidecar.
pub fn write_with_sha256(bz2_path: &Path, data_dir: &Path, stem: &str) -> eyre::Result<()> {
    let crc = calculate_sha256(bz2_path)?;
    let bz2_target = data_dir.join(format!("{stem}.bz2"));
    let sidecar = data_dir.join(format!("{stem}.bz2.sha256"));
    let mut staging = tempfile::NamedTempFile::new_in(data_dir)?;
    std::io::copy(&mut fs::File::open(bz2_path)?, &mut staging)?;
    staging.persist(&bz2_target).map_err(|e| e.error)?;
    fs::write(&sidecar, &crc)?;
    info!(stem, crc = %crc, "Mirror cached compressed snapshot");
    Ok(())
}

/// Sibling of `data_dir`, holding working state that isn't meant to be
/// served publicly (currently: the persistent, incrementally-scraped
/// Pokémon db). Kept outside `data_dir` because `ServeDir` serves
/// everything under it.
fn state_dir(data_dir: &Path) -> PathBuf {
    let name = data_dir
        .file_name()
        .map(|n| format!("{}-state", n.to_string_lossy()))
        .unwrap_or_else(|| "mirror-state".to_string());
    data_dir
        .parent()
        .map(|p| p.join(&name))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Refreshes all five mirrored components into `data_dir`. Each component
/// is independent — one failing (e.g. a scraper target changing layout)
/// doesn't block the others. A component refreshed successfully within
/// `interval` is skipped; a component that failed or never finished is
/// always retried, regardless of `interval`.
pub async fn run_update_cycle(data_dir: &Path, interval: Duration) -> eyre::Result<()> {
    fs::create_dir_all(data_dir)?;

    refresh_if_stale(
        data_dir,
        "AllPrintings.sqlite",
        interval,
        cache_upstream_bz2(
            "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2",
            "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2.sha256",
            "AllPrintings.sqlite",
            data_dir,
        ),
    )
    .await;

    refresh_if_stale(
        data_dir,
        "AllPricesToday.sqlite",
        interval,
        cache_upstream_bz2(
            "https://mtgjson.com/api/v5/AllPricesToday.sqlite.bz2",
            "https://mtgjson.com/api/v5/AllPricesToday.sqlite.bz2.sha256",
            "AllPricesToday.sqlite",
            data_dir,
        ),
    )
    .await;

    refresh_if_stale(
        data_dir,
        "pokemon_prices.sqlite",
        interval,
        refresh_pokemon_prices(data_dir),
    )
    .await;

    refresh_if_stale(
        data_dir,
        "riftbound.sqlite",
        interval,
        refresh_riftbound_cards(data_dir),
    )
    .await;

    refresh_if_stale(
        data_dir,
        "pokemon.sqlite",
        interval,
        refresh_pokemon_cards(data_dir),
    )
    .await;

    Ok(())
}

async fn refresh_pokemon_prices(data_dir: &Path) -> eyre::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let raw = temp_dir.path().join("pokemon_prices.sqlite");
    crate::systems::pokemon::download_pokemon_prices(raw.to_str().unwrap()).await?;
    let bz2 = temp_dir.path().join("pokemon_prices.sqlite.bz2");
    compress_bz2(&raw, &bz2)?;
    write_with_sha256(&bz2, data_dir, "pokemon_prices.sqlite")
}

/// Persists its working db at a fixed path across cycles instead of a
/// fresh tempdir. `build_riftbound_db` fetches the whole card list in one
/// request and either gets it all or fails outright, so this is lower-risk
/// than the Pokémon scrape — but the upstream blade index is hardcoded
/// (`blades[2]`), so a site layout change could silently return a partial
/// list without erroring. A persistent db means that still only merges in
/// whatever came back, instead of replacing the full snapshot.
async fn refresh_riftbound_cards(data_dir: &Path) -> eyre::Result<()> {
    let state_dir = state_dir(data_dir);
    fs::create_dir_all(&state_dir)?;
    let raw = state_dir.join("riftbound.sqlite");
    crate::systems::riftsqlite::build_riftbound_db(raw.to_str().unwrap()).await?;

    let temp_dir = tempfile::tempdir()?;
    let bz2 = temp_dir.path().join("riftbound.sqlite.bz2");
    compress_bz2(&raw, &bz2)?;
    write_with_sha256(&bz2, data_dir, "riftbound.sqlite")
}

/// Unlike the other components, this persists its working db at a fixed
/// path across cycles instead of starting from a fresh tempdir. The
/// scraper hits many upstream sources and partial failures are routine —
/// scraping incrementally into a long-lived db means a bad cycle merges
/// in whatever succeeded and leaves the rest untouched, instead of
/// publishing a near-empty snapshot. See `pokemon::scrape_to_path`.
async fn refresh_pokemon_cards(data_dir: &Path) -> eyre::Result<()> {
    let state_dir = state_dir(data_dir);
    fs::create_dir_all(&state_dir)?;
    let raw = state_dir.join("pokemon.sqlite");
    crate::systems::pokemon::scrape_to_path(raw.to_str().unwrap()).await?;

    let temp_dir = tempfile::tempdir()?;
    let bz2 = temp_dir.path().join("pokemon.sqlite.bz2");
    compress_bz2(&raw, &bz2)?;
    write_with_sha256(&bz2, data_dir, "pokemon.sqlite")
}

#[cfg(test)]
mod tests;
