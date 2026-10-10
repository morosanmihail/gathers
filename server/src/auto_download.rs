use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use retrieval::RetrievalSystemTrait as _;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::{RetrievalState, StorageState, price_history};

pub fn default_enabled() -> bool {
    false
}

pub fn default_interval_hours() -> u64 {
    24
}

/// Longest allowed auto-download interval: a year.
pub const MAX_INTERVAL_HOURS: u64 = 24 * 365;

fn marker_path(gathers_dir: &Path) -> PathBuf {
    gathers_dir.join("auto_download.last_run")
}

/// When the auto-download cycle last completed, if ever. Persisted to disk so
/// a server restart resumes the schedule instead of restarting the interval.
fn last_run(gathers_dir: &Path) -> Option<SystemTime> {
    let secs: u64 = std::fs::read_to_string(marker_path(gathers_dir))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(UNIX_EPOCH + Duration::from_secs(secs))
}

fn write_last_run(gathers_dir: &Path) -> eyre::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    std::fs::write(marker_path(gathers_dir), now.to_string())?;
    Ok(())
}

/// Runs `update` for the database behind `key` unless a manual update of it
/// is already running (see `RetrievalState::start_download`), marking it as
/// running meanwhile so a manual update can't start mid-way either.
async fn run_exclusive<F>(retrieval: &Arc<Mutex<RetrievalState>>, key: &str, update: F)
where
    F: std::future::Future<Output = ()>,
{
    if retrieval.lock().await.start_download(key).is_err() {
        info!(key, "Skipping auto-download: an update is already running");
        return;
    }
    update.await;
    retrieval.lock().await.finish_download(key);
}

/// Re-downloads card and price databases for every currently active system,
/// reusing the same trigger paths as the manual `/update` HTTP endpoints.
/// After a price database is updated, records price history from it.
async fn run_all(retrieval: &Arc<Mutex<RetrievalState>>, storage: &Arc<Mutex<StorageState>>) {
    let mtg = retrieval.lock().await.mtg.clone();
    if let Some(mtg) = mtg {
        run_exclusive(retrieval, "Sql", async {
            match mtg.update_backend().await {
                Ok(_) => match retrieval.lock().await.reload_mtg() {
                    Ok(_) => info!("MTG card DB auto-downloaded"),
                    Err(e) => error!(error = %e, "Failed to reload MTG after auto-download"),
                },
                Err(e) => error!(error = %e, "Auto-download of MTG card DB failed"),
            }
        })
        .await;
        run_exclusive(retrieval, "Sql-prices", async {
            match mtg.update_prices().await {
                Ok(_) => {
                    info!("MTG price DB auto-downloaded");
                    price_history::snapshot_tracked(retrieval, storage, &mtg, false).await;
                }
                Err(e) => error!(error = %e, "Auto-download of MTG price DB failed"),
            }
        })
        .await;
    }

    let riftbound = retrieval.lock().await.riftbound.clone();
    if let Some(riftbound) = riftbound {
        run_exclusive(retrieval, "RiftboundSql", async {
            match riftbound.update_backend().await {
                Ok(_) => match retrieval.lock().await.reload_riftbound() {
                    Ok(_) => info!("Riftbound card DB auto-downloaded"),
                    Err(e) => error!(error = %e, "Failed to reload Riftbound after auto-download"),
                },
                Err(e) => error!(error = %e, "Auto-download of Riftbound card DB failed"),
            }
        })
        .await;
    }

    let pokemon = retrieval.lock().await.pokemon.clone();
    if let Some(pokemon) = pokemon {
        run_exclusive(retrieval, "PokemonSql", async {
            match pokemon.update_backend().await {
                Ok(_) => match retrieval.lock().await.reload_pokemon() {
                    Ok(_) => info!("Pokemon card DB auto-downloaded"),
                    Err(e) => error!(error = %e, "Failed to reload Pokemon after auto-download"),
                },
                Err(e) => error!(error = %e, "Auto-download of Pokemon card DB failed"),
            }
        })
        .await;
        run_exclusive(retrieval, "PokemonSql-prices", async {
            match pokemon.update_prices().await {
                Ok(_) => {
                    info!("Pokemon price DB auto-downloaded");
                    price_history::snapshot_tracked(retrieval, storage, &pokemon, false).await;
                }
                Err(e) => error!(error = %e, "Auto-download of Pokemon price DB failed"),
            }
        })
        .await;
    }
}

/// Spawns the periodic auto-download loop. Resumes from the persisted
/// last-run timestamp instead of restarting the interval on every boot.
pub fn spawn(
    retrieval: Arc<Mutex<RetrievalState>>,
    storage: Arc<Mutex<StorageState>>,
    gathers_dir: PathBuf,
    interval_hours: u64,
) {
    let interval_hours = interval_hours.clamp(1, MAX_INTERVAL_HOURS);
    let interval = Duration::from_secs(interval_hours * 3600);
    info!(interval_hours, "Periodic DB auto-download enabled");
    tokio::spawn(async move {
        let wait = last_run(&gathers_dir)
            .and_then(|t| t.elapsed().ok())
            .map(|elapsed| interval.saturating_sub(elapsed))
            .unwrap_or(Duration::ZERO);
        if !wait.is_zero() {
            info!(wait_secs = wait.as_secs(), "Resuming auto-download schedule");
            tokio::time::sleep(wait).await;
        }
        loop {
            info!("Running scheduled DB auto-download");
            run_all(&retrieval, &storage).await;
            if let Err(e) = write_last_run(&gathers_dir) {
                error!(error = %e, "Failed to record auto-download timestamp");
            }
            tokio::time::sleep(interval).await;
        }
    });
}
