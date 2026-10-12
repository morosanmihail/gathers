//! Full backups of the user's own data: the storage database and, when
//! price history is on, the price history database, zipped together with a
//! `manifest.json` describing them.

use std::io::{Cursor, Read as _, Write as _};
use std::path::{Path, PathBuf};

use axum::{
    Json,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
    routing::{MethodRouter, get},
};
use persistence::{PersistenceSystem, PersistenceSystemTrait as _, SQLitePersistenceSystem};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ApiError, ErrorPayload, GathersState, bad_request, demo_err, demo_mode, storage_error};

const MANIFEST_FILE: &str = "manifest.json";
const STORAGE_FILE: &str = "storage.db";
const PRICE_HISTORY_FILE: &str = "price_history.db";
/// Bumped when the archive's layout changes incompatibly.
const BACKUP_FORMAT: u32 = 1;
/// Largest backup upload accepted.
const MAX_UPLOAD_BYTES: usize = 1024 * 1024 * 1024;
/// Largest a single database may be once unzipped, so a small archive can't
/// expand to fill the disk.
const MAX_DB_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// `GET` downloads a backup, `POST` restores one.
pub fn backup_route() -> MethodRouter<GathersState> {
    get(download_backup)
        .post(restore_backup)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
}

/// What a backup archive holds. Schema versions are the number of
/// migrations applied to each database; a restore runs only the ones the
/// backup is missing, and refuses backups newer than the server.
#[derive(Debug, Serialize, Deserialize)]
struct BackupManifest {
    format: u32,
    /// Server version that made the backup.
    app_version: String,
    /// RFC 3339 time the backup was made.
    created_at: String,
    storage_schema_version: usize,
    /// Absent when price history was off, and the archive has no
    /// price history database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price_history_schema_version: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct RestoreResponse {
    /// Whether price history was replaced too. False when the backup has
    /// none or price history is off on this server; existing price history
    /// is then left as it was.
    price_history_restored: bool,
}

fn internal(context: &str, e: impl std::fmt::Display) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorPayload { error: format!("{context}: {e}") }),
    )
}

async fn sqlite_storage(state: &GathersState) -> SQLitePersistenceSystem {
    let PersistenceSystem::SQLitePersistenceSystem(sqlite) = state.1.lock().await.storage.clone();
    sqlite
}

/// Downloads a zip of the storage database and, when price history is on,
/// the price history database.
async fn download_backup(State(state): State<GathersState>) -> Result<Response, ApiError> {
    if demo_mode() {
        return Err(demo_err());
    }
    let storage = sqlite_storage(&state).await;
    let dir = tempfile::tempdir().map_err(|e| internal("Failed to create a temporary directory", e))?;
    let storage_path = dir.path().join(STORAGE_FILE);
    let price_path = dir.path().join(PRICE_HISTORY_FILE);
    let storage_schema_version = storage
        .export_storage(&storage_path)
        .await
        .map_err(|e| storage_error("Failed to back up the storage database", e))?;
    let price_history_schema_version = storage
        .export_price_history(&price_path)
        .await
        .map_err(|e| storage_error("Failed to back up the price history database", e))?;
    let now = chrono::Utc::now();
    let manifest = BackupManifest {
        format: BACKUP_FORMAT,
        app_version: env!("GATHERS_VERSION").to_string(),
        created_at: now.to_rfc3339(),
        storage_schema_version,
        price_history_schema_version,
    };

    let zip = tokio::task::spawn_blocking(move || {
        let mut files = vec![(STORAGE_FILE, storage_path)];
        if manifest.price_history_schema_version.is_some() {
            files.push((PRICE_HISTORY_FILE, price_path));
        }
        let zip = write_zip(&manifest, &files);
        drop(dir);
        zip
    })
    .await
    .map_err(|e| internal("Backup task failed", e))?
    .map_err(|e| internal("Failed to write the backup archive", e))?;

    let disposition = format!("attachment; filename=\"gathers-backup-{}.zip\"", now.format("%Y%m%d-%H%M%S"));
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"gathers-backup.zip\"")),
        )
        .body(axum::body::Body::from(zip))
        .unwrap())
}

fn write_zip(manifest: &BackupManifest, files: &[(&str, PathBuf)]) -> eyre::Result<Vec<u8>> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(MANIFEST_FILE, options)?;
    zip.write_all(&serde_json::to_vec_pretty(manifest)?)?;
    for (name, path) in files {
        zip.start_file(*name, options)?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut zip)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Replaces the storage database (and price history, when the backup has it
/// and price history is on) with the contents of an uploaded backup zip,
/// given as the multipart field `file`. Backups from older servers are
/// migrated; ones from newer servers are refused. Nothing changes unless
/// the whole backup is valid.
async fn restore_backup(
    State(state): State<GathersState>,
    mut multipart: Multipart,
) -> Result<Json<RestoreResponse>, ApiError> {
    if demo_mode() {
        return Err(demo_err());
    }
    let mut archive = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| bad_request(format!("Failed to read multipart field: {e}")))?
    {
        if field.name() == Some("file") {
            archive = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| bad_request(format!("Failed to read the uploaded file: {e}")))?,
            );
        }
    }
    let archive = archive.ok_or_else(|| bad_request("No backup file uploaded (expected a 'file' field)"))?;

    let dir = tempfile::tempdir().map_err(|e| internal("Failed to create a temporary directory", e))?;
    let dir_path = dir.path().to_path_buf();
    let (storage_path, price_path) = tokio::task::spawn_blocking(move || unpack(&archive, &dir_path))
        .await
        .map_err(|e| internal("Restore task failed", e))??;

    let storage = sqlite_storage(&state).await;
    let price_history_restored = price_path.is_some() && storage.price_history_enabled();
    storage
        .restore(&storage_path, price_path.as_deref())
        .await
        .map_err(|e| storage_error("Failed to restore the backup", e))?;
    tracing::info!(price_history_restored, "Restored backup");
    Ok(Json(RestoreResponse { price_history_restored }))
}

/// Checks the archive's manifest and extracts its databases into `dir`,
/// returning the storage database's path and the price history one's, if
/// the backup has it.
fn unpack(archive: &[u8], dir: &Path) -> Result<(PathBuf, Option<PathBuf>), ApiError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))
        .map_err(|e| bad_request(format!("The uploaded file is not a zip archive: {e}")))?;

    let manifest: BackupManifest = {
        let mut entry = zip
            .by_name(MANIFEST_FILE)
            .map_err(|_| bad_request(format!("Not a gathers backup: {MANIFEST_FILE} is missing")))?;
        let mut json = String::new();
        entry
            .by_ref()
            .take(1024 * 1024)
            .read_to_string(&mut json)
            .map_err(|e| bad_request(format!("Failed to read {MANIFEST_FILE}: {e}")))?;
        serde_json::from_str(&json).map_err(|e| bad_request(format!("Invalid {MANIFEST_FILE}: {e}")))?
    };
    if manifest.format != BACKUP_FORMAT {
        return Err(bad_request(format!(
            "Unsupported backup format {} (this server reads format {BACKUP_FORMAT})",
            manifest.format
        )));
    }
    let too_new = |what: &str, version: usize, latest: usize| {
        bad_request(format!(
            "The backup's {what} database has schema version {version}, newer than this server supports ({latest}). It was made by gathers {}; upgrade the server first",
            manifest.app_version
        ))
    };
    let latest = persistence::latest_storage_schema_version();
    if manifest.storage_schema_version > latest {
        return Err(too_new("storage", manifest.storage_schema_version, latest));
    }
    let latest = persistence::latest_price_history_schema_version();
    if let Some(version) = manifest.price_history_schema_version
        && version > latest
    {
        return Err(too_new("price history", version, latest));
    }

    let storage_path = extract(&mut zip, STORAGE_FILE, dir)?;
    let price_path = manifest
        .price_history_schema_version
        .map(|_| extract(&mut zip, PRICE_HISTORY_FILE, dir))
        .transpose()?;
    Ok((storage_path, price_path))
}

fn extract(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, dir: &Path) -> Result<PathBuf, ApiError> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| bad_request(format!("The backup is missing {name}")))?;
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).map_err(|e| internal("Failed to unpack the backup", e))?;
    let written = std::io::copy(&mut entry.by_ref().take(MAX_DB_BYTES + 1), &mut file)
        .map_err(|e| bad_request(format!("Failed to unpack {name}: {e}")))?;
    if written > MAX_DB_BYTES {
        return Err(bad_request(format!("{name} in the backup is too large")));
    }
    Ok(path)
}
