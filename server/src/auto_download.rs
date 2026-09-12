use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use retrieval::RetrievalSystemTrait as _;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::RetrievalState;

pub fn default_enabled() -> bool {
    false
}

pub fn default_interval_hours() -> u64 {
    24
}

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

/// Re-downloads card and price databases for every currently active system,
/// reusing the same trigger paths as the manual `/update` HTTP endpoints.
async fn run_all(retrieval: &Arc<Mutex<RetrievalState>>) {
    let mtg = retrieval.lock().await.mtg.clone();
    if let Some(mtg) = mtg {
        match mtg.update_backend().await {
            Ok(_) => match retrieval.lock().await.reload_mtg() {
                Ok(_) => info!("MTG card DB auto-downloaded"),
                Err(e) => error!(error = %e, "Failed to reload MTG after auto-download"),
            },
            Err(e) => error!(error = %e, "Auto-download of MTG card DB failed"),
        }
        match mtg.update_prices().await {
            Ok(_) => info!("MTG price DB auto-downloaded"),
            Err(e) => error!(error = %e, "Auto-download of MTG price DB failed"),
        }
    }

    let riftbound = retrieval.lock().await.riftbound.clone();
    if let Some(riftbound) = riftbound {
        match riftbound.update_backend().await {
            Ok(_) => match retrieval.lock().await.reload_riftbound() {
                Ok(_) => info!("Riftbound card DB auto-downloaded"),
                Err(e) => error!(error = %e, "Failed to reload Riftbound after auto-download"),
            },
            Err(e) => error!(error = %e, "Auto-download of Riftbound card DB failed"),
        }
    }

    let pokemon = retrieval.lock().await.pokemon.clone();
    if let Some(pokemon) = pokemon {
        match pokemon.update_backend().await {
            Ok(_) => match retrieval.lock().await.reload_pokemon() {
                Ok(_) => info!("Pokemon card DB auto-downloaded"),
                Err(e) => error!(error = %e, "Failed to reload Pokemon after auto-download"),
            },
            Err(e) => error!(error = %e, "Auto-download of Pokemon card DB failed"),
        }
        match pokemon.update_prices().await {
            Ok(_) => info!("Pokemon price DB auto-downloaded"),
            Err(e) => error!(error = %e, "Auto-download of Pokemon price DB failed"),
        }
    }
}

/// Spawns the periodic auto-download loop. Resumes from the persisted
/// last-run timestamp instead of restarting the interval on every boot.
pub fn spawn(retrieval: Arc<Mutex<RetrievalState>>, gathers_dir: PathBuf, interval_hours: u64) {
    let interval_hours = interval_hours.max(1);
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
            run_all(&retrieval).await;
            if let Err(e) = write_last_run(&gathers_dir) {
                error!(error = %e, "Failed to record auto-download timestamp");
            }
            tokio::time::sleep(interval).await;
        }
    });
}
