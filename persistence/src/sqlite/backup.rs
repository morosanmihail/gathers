//! Whole-database backup and restore, for the storage and price history
//! databases. Each database's schema version is the number of migrations
//! applied to it (kept by `rusqlite_migration` in `PRAGMA user_version`),
//! so a restored backup only needs the migrations it is missing.

use std::path::Path;

use include_dir::Dir;
use rusqlite::{Connection, OpenFlags};
use rusqlite::backup::Backup;
use rusqlite_migration::Migrations;

use super::{MIGRATIONS, MIGRATIONS_DIR, PRICE_MIGRATIONS, PRICE_MIGRATIONS_DIR, SQLitePersistenceSystem};
use crate::PersistenceError;

/// Schema version of a database brought fully up to date by this build.
fn latest_version(dir: &Dir) -> usize {
    dir.dirs().count()
}

pub fn latest_storage_schema_version() -> usize {
    latest_version(&MIGRATIONS_DIR)
}

pub fn latest_price_history_schema_version() -> usize {
    latest_version(&PRICE_MIGRATIONS_DIR)
}

fn schema_version(conn: &Connection) -> rusqlite::Result<usize> {
    conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map(|v| v.max(0) as usize)
}

/// Writes a consistent copy of `conn`'s database to `dest`, which must not
/// exist yet. Returns the copy's schema version.
fn export(conn: &Connection, dest: &Path) -> eyre::Result<usize> {
    let dest = dest
        .to_str()
        .ok_or_else(|| eyre::eyre!("Backup path is not valid UTF-8: {}", dest.display()))?;
    conn.execute("VACUUM INTO ?1", [dest])?;
    Ok(schema_version(conn)?)
}

/// Opens an uploaded database and brings it up to date, refusing anything
/// that isn't an intact database of the expected kind (one with
/// `required_table`) or that comes from a newer build than this one.
fn prepare_restore(
    path: &Path,
    what: &str,
    migrations: &Migrations<'static>,
    latest: usize,
    required_table: &str,
) -> eyre::Result<Connection> {
    let invalid = |msg: String| eyre::Report::new(PersistenceError::InvalidInput(msg));
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| invalid(format!("The {what} backup can't be opened: {e}")))?;
    // Not a database at all fails here rather than at open.
    let check: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| invalid(format!("The {what} backup is not a valid SQLite database: {e}")))?;
    if check != "ok" {
        return Err(invalid(format!("The {what} backup is corrupt: {check}")));
    }
    let has_table: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [required_table],
        |row| row.get(0),
    )?;
    if !has_table {
        return Err(invalid(format!("The {what} backup is not a gathers {what} database")));
    }
    let version = schema_version(&conn)?;
    if version > latest {
        return Err(invalid(format!(
            "The {what} backup has schema version {version}, newer than this server supports ({latest}); upgrade the server first"
        )));
    }
    migrations
        .to_latest(&mut conn)
        .map_err(|e| invalid(format!("Failed to upgrade the {what} backup: {e}")))?;
    Ok(conn)
}

/// Replaces everything in `dest` with the contents of `src`.
fn copy_into(src: &Connection, dest: &mut Connection) -> eyre::Result<()> {
    Backup::new(src, dest)?.run_to_completion(256, std::time::Duration::ZERO, None)?;
    Ok(())
}

impl SQLitePersistenceSystem {
    /// Writes a copy of the storage database to `dest` (which must not
    /// exist). Returns its schema version.
    pub async fn export_storage(&self, dest: &Path) -> eyre::Result<usize> {
        let conn = self.connection.lock().await;
        export(&conn, dest)
    }

    /// Writes a copy of the price history database to `dest` (which must
    /// not exist) and returns its schema version, or does nothing and
    /// returns `None` when price history is off.
    pub async fn export_price_history(&self, dest: &Path) -> eyre::Result<Option<usize>> {
        let Some(price_connection) = &self.price_connection else {
            return Ok(None);
        };
        let conn = price_connection.lock().await;
        export(&conn, dest).map(Some)
    }

    /// Replaces the storage database (and, when `price_history` is given,
    /// the price history one) with the databases at these paths, migrating
    /// them up to date first. Both are checked before either is replaced,
    /// so a bad backup changes nothing — failing with
    /// `PersistenceError::InvalidInput`. A price history backup is ignored
    /// when price history is off. The files at the paths are modified.
    pub async fn restore(&self, storage: &Path, price_history: Option<&Path>) -> eyre::Result<()> {
        let storage_src = prepare_restore(
            storage,
            "storage",
            &MIGRATIONS,
            latest_storage_schema_version(),
            "collection",
        )?;
        let price_src = match (price_history, &self.price_connection) {
            (Some(path), Some(_)) => Some(prepare_restore(
                path,
                "price history",
                &PRICE_MIGRATIONS,
                latest_price_history_schema_version(),
                "price_history",
            )?),
            _ => None,
        };

        let mut conn = self.connection.lock().await;
        copy_into(&storage_src, &mut conn)?;
        if let (Some(src), Some(price_connection)) = (price_src, &self.price_connection) {
            let mut price_conn = price_connection.lock().await;
            copy_into(&src, &mut price_conn)?;
        }
        Ok(())
    }
}
