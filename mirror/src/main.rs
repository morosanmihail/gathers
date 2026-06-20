use std::{path::PathBuf, time::Duration};

use axum::Router;
use tower_http::services::ServeDir;
use tracing::{error, info};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let data_dir = PathBuf::from(std::env::var("MIRROR_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.local/share/gathers/mirror")
    }));
    let port: u16 = std::env::var("MIRROR_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5235);
    let interval_hours: u64 = std::env::var("MIRROR_INTERVAL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    std::fs::create_dir_all(&data_dir)?;
    info!(data_dir = ?data_dir, port, interval_hours, "GatheRs mirror starting");

    let update_dir = data_dir.clone();
    let interval = Duration::from_secs(interval_hours * 3600);
    tokio::spawn(async move {
        loop {
            info!("Starting mirror update cycle");
            // Components updated within `interval` are skipped internally;
            // only stale or previously-failed ones are actually refreshed,
            // so a restart doesn't redo work that already succeeded.
            if let Err(e) = retrieval::mirror::run_update_cycle(&update_dir, interval).await {
                error!(error = %e, "Mirror update cycle failed");
            } else {
                info!("Mirror update cycle complete");
            }
            tokio::time::sleep(interval).await;
        }
    });

    let app = Router::new().fallback_service(ServeDir::new(&data_dir));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    info!(port, "Serving mirror data");
    axum::serve(listener, app).await?;

    Ok(())
}
