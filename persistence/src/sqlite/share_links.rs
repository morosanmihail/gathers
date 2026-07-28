use models::CollectionID;
use rusqlite::{Connection, OptionalExtension, params};

use crate::ShareLink;

pub(super) fn create(conn: &Connection, collection_id: &CollectionID) -> eyre::Result<ShareLink> {
    let token = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO share_links (token, collection_id, created_at) VALUES (?1, ?2, ?3)",
        params![token, collection_id, created_at],
    )?;
    Ok(ShareLink {
        token,
        collection_id: collection_id.clone(),
        created_at,
    })
}

pub(super) fn list(conn: &Connection, collection_id: &CollectionID) -> eyre::Result<Vec<ShareLink>> {
    let mut stmt = conn.prepare(
        "SELECT token, collection_id, created_at FROM share_links WHERE collection_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![collection_id], |r| {
        Ok(ShareLink {
            token: r.get(0)?,
            collection_id: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

pub(super) fn revoke(conn: &Connection, collection_id: &CollectionID, token: &str) -> eyre::Result<bool> {
    let n = conn.execute(
        "DELETE FROM share_links WHERE token = ?1 AND collection_id = ?2",
        params![token, collection_id],
    )?;
    Ok(n > 0)
}

pub(super) fn resolve(conn: &Connection, token: &str) -> eyre::Result<Option<CollectionID>> {
    conn.query_row(
        "SELECT collection_id FROM share_links WHERE token = ?1",
        params![token],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}
