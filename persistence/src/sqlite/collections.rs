use models::CollectionID;
use rusqlite::{Connection, OptionalExtension, params};

use crate::{CollectionInfo, PersistenceError};

pub(super) fn add_collection(conn: &Connection, name: &CollectionID) -> eyre::Result<CollectionID> {
    conn.execute(
        "INSERT OR IGNORE INTO collection (name, can_remove) VALUES (?1, ?2)",
        params![name, true],
    )?;
    Ok(name.clone())
}

/// `Some(can_remove)` if the collection exists.
fn removable(conn: &Connection, name: &CollectionID) -> eyre::Result<Option<bool>> {
    Ok(conn
        .query_row(
            "SELECT can_remove FROM collection WHERE name = ?1",
            params![name],
            |r| r.get::<_, bool>(0),
        )
        .optional()?)
}

/// Must run inside a transaction: it's several statements that have to
/// apply all together or not at all.
pub(super) fn remove_collection(
    conn: &Connection,
    name: &CollectionID,
    move_to: Option<&CollectionID>,
) -> eyre::Result<CollectionID> {
    let can_remove = removable(conn, name)?
        .ok_or_else(|| PersistenceError::CollectionNotFound(name.clone()))?;

    if let Some(target) = move_to {
        if target == name {
            return Err(PersistenceError::InvalidInput(
                "Can't move a collection's cards into itself".to_string(),
            )
            .into());
        }
        if removable(conn, target)?.is_none() {
            return Err(PersistenceError::CollectionNotFound(target.clone()).into());
        }
        let query = "INSERT INTO cards (uuid, finish, collection, quantity, want_quantity, timeadded, timeupdated, provider)
            SELECT uuid, finish, ?1 as collection, quantity, want_quantity, timeadded, strftime('%Y-%m-%dT%H:%M:%SZ', 'now') as timeupdated, provider
            FROM cards WHERE collection = ?2
            ON CONFLICT (uuid, finish, collection)
            DO UPDATE SET
                quantity = min(cards.quantity + EXCLUDED.quantity, 2147483647),
                want_quantity = min(cards.want_quantity + EXCLUDED.want_quantity, 2147483647),
                timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');";
        conn.execute(query, params![target, name])?;
        // The cards' cost basis travels with them.
        conn.execute(
            "UPDATE purchase_history SET collection_id = ?1 WHERE collection_id = ?2",
            params![target, name],
        )?;
    } else if !can_remove {
        return Err(PersistenceError::CollectionNotRemovable(name.clone()).into());
    }

    conn.execute("DELETE FROM cards WHERE collection = ?1", params![name])?;
    conn.execute(
        "DELETE FROM purchase_history WHERE collection_id = ?1",
        params![name],
    )?;
    if can_remove {
        // Otherwise a later collection reusing this name would be exposed
        // through the old links.
        conn.execute("DELETE FROM share_links WHERE collection_id = ?1", params![name])?;
        conn.execute("DELETE FROM collection WHERE name = ?1", params![name])?;
    }
    Ok(name.clone())
}

pub(super) fn list_collections(
    conn: &Connection,
    filter: Option<&str>,
) -> eyre::Result<Vec<CollectionID>> {
    let pattern = filter.map(|f| format!("%{f}%"));
    let collections = if let Some(p) = &pattern {
        let mut stmt = conn.prepare("SELECT name FROM collection WHERE name LIKE ?1")?;
        stmt.query_map(params![p], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?
    } else {
        let mut stmt = conn.prepare("SELECT name FROM collection")?;
        stmt.query_map(params![], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?
    };
    Ok(collections)
}

pub(super) fn list_collection_info(conn: &Connection) -> eyre::Result<Vec<CollectionInfo>> {
    let mut stmt = conn.prepare("SELECT name, can_remove FROM collection")?;
    let rows = stmt.query_map(params![], |r| {
        Ok(CollectionInfo {
            name: r.get(0)?,
            removable: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Must run inside a transaction, like `remove_collection`.
pub(super) fn rename_collection(
    conn: &Connection,
    old_name: &CollectionID,
    new_name: &CollectionID,
) -> eyre::Result<()> {
    if removable(conn, old_name)?.is_none() {
        return Err(PersistenceError::CollectionNotFound(old_name.clone()).into());
    }
    if old_name == new_name {
        return Ok(());
    }
    if removable(conn, new_name)?.is_some() {
        return Err(PersistenceError::CollectionExists(new_name.clone()).into());
    }
    conn.execute(
        "UPDATE collection SET name = ?1 WHERE name = ?2",
        params![new_name, old_name],
    )?;
    conn.execute(
        "UPDATE cards SET collection = ?1 WHERE collection = ?2",
        params![new_name, old_name],
    )?;
    conn.execute(
        "UPDATE purchase_history SET collection_id = ?1 WHERE collection_id = ?2",
        params![new_name, old_name],
    )?;
    conn.execute(
        "UPDATE share_links SET collection_id = ?1 WHERE collection_id = ?2",
        params![new_name, old_name],
    )?;
    Ok(())
}

/// Number of card *entries* (rows) in a collection.
pub(super) fn get_cards_count(
    conn: &Connection,
    collection_id: &CollectionID,
    providers: &[String],
    enabled_plugin_providers: Option<&[String]>,
) -> eyre::Result<usize> {
    let mut conditions = vec!["collection = ?1".to_string()];
    let mut query_params: Vec<String> = vec![collection_id.clone()];

    if !providers.is_empty() {
        let placeholders: Vec<String> =
            (2..=providers.len() + 1).map(|i| format!("?{i}")).collect();
        conditions.push(format!("provider IN ({})", placeholders.join(", ")));
        query_params.extend_from_slice(providers);
    }
    if let Some((condition, scope_params)) =
        super::cards::plugin_scope_condition(enabled_plugin_providers, query_params.len() + 1)
    {
        conditions.push(condition);
        query_params.extend(scope_params);
    }

    // Deliberately COUNT(*), not COUNT(DISTINCT uuid): since 08-card-finishes, a card owned in
    // several finishes is one row per finish, and this count is the pagination total for
    // get_cards_in_collection_paginated, which yields those same rows. Counting distinct cards here
    // would under-report the total and make the tail of a collection unreachable in the UI.
    let query = format!("SELECT COUNT(*) FROM cards WHERE {}", conditions.join(" AND "));
    let mut stmt = conn.prepare(&query)?;
    let count = stmt.query_row(rusqlite::params_from_iter(query_params.iter()), |r| {
        r.get::<_, u32>(0)
    })? as usize;
    Ok(count)
}
