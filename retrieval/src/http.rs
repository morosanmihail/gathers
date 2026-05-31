use std::{io::Write, path::Path, sync::Arc};

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::Mutex;
use tracing::warn;

const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub phase: String,
}

pub async fn stream_to_file(
    url: &str,
    label: &str,
    path: &Path,
    progress: Option<&Arc<Mutex<DownloadProgress>>>,
    phase: &str,
) -> eyre::Result<()> {
    let mut last_err = eyre::eyre!("no attempts made");
    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        match stream_to_file_once(url, label, path, progress, phase).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt < MAX_DOWNLOAD_ATTEMPTS {
                    warn!(
                        attempt,
                        max_attempts = MAX_DOWNLOAD_ATTEMPTS,
                        error = %e,
                        "Download attempt {attempt}/{MAX_DOWNLOAD_ATTEMPTS} failed, retrying"
                    );
                }
                last_err = e;
            }
        }
    }
    Err(last_err)
}

async fn stream_to_file_once(
    url: &str,
    label: &str,
    path: &Path,
    progress: Option<&Arc<Mutex<DownloadProgress>>>,
    phase: &str,
) -> eyre::Result<()> {
    let response = reqwest::Client::new().get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0);

    if let Some(p) = progress {
        let mut p = p.lock().await;
        p.total = total_size;
        p.downloaded = 0;
        p.phase = phase.to_string();
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% ({eta_precise}) {bytes} / {total_bytes}")?
            .progress_chars("#>-"),
    );

    let mut file = std::fs::File::create(path)?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        let len = chunk.len() as u64;
        pb.inc(len);
        if let Some(p) = progress {
            let mut p = p.lock().await;
            p.downloaded += len;
        }
    }
    pb.finish_with_message(label.to_string());
    Ok(())
}
