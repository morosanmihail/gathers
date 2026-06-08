use models::CollectionID;
use rusqlite::{Connection, params};

pub(super) fn add_collection(conn: &Connection, name: &CollectionID) -> eyre::Result<CollectionID> {
    conn.execute(
        "INSERT OR IGNORE INTO collection (name, can_remove) VALUES (?1, ?2)",
        params![name, true],
    )?;
    Ok(name.clone())
}

pub(super) fn remove_collection(
    conn: &Connection,
    name: &CollectionID,
    move_to: Option<&CollectionID>,
) -> eyre::Result<CollectionID> {
    if let Some(target) = move_to {
        let query = "INSERT INTO cards (uuid, collection, quantity, foilquantity, timeadded, timeupdated, provider)
            SELECT uuid, ?1 as collection, quantity, foilquantity, timeadded, strftime('%Y-%m-%dT%H:%M:%SZ', 'now') as timeupdated, provider FROM
\t(SELECT uuid, ?2 as collection, quantity, foilquantity, timeadded, provider FROM cards WHERE collection = ?2) WHERE true
            ON CONFLICT (uuid, collection)
            DO UPDATE SET
                quantity = cards.quantity + EXCLUDED.quantity,
                foilquantity = cards.foilquantity + EXCLUDED.foilquantity,
                timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');";
        conn.execute(query, params![target, name])?;
    }
    conn.execute(
        "DELETE FROM cards WHERE collection = ?1",
        params![name],
    )?;
    conn.execute(
        "DELETE FROM collection WHERE name = ?1 AND can_remove = TRUE",
        params![name],
    )?;
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

pub(super) fn rename_collection(
    conn: &Connection,
    old_name: &CollectionID,
    new_name: &CollectionID,
) -> eyre::Result<()> {
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
    Ok(())
}

pub(super) fn get_cards_count(
    conn: &Connection,
    collection_id: &CollectionID,
    providers: &[String],
) -> eyre::Result<usize> {
    let count: usize = if providers.is_empty() {
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM cards WHERE collection = ?1")?;
        stmt.query_row(params![collection_id], |r| r.get::<_, u32>(0))? as usize
    } else {
        let placeholders: Vec<String> =
            (2..=providers.len() + 1).map(|i| format!("?{i}")).collect();
        let query = format!(
            "SELECT COUNT(*) FROM cards WHERE collection = ?1 AND provider IN ({})",
            placeholders.join(", ")
        );
        let mut query_params: Vec<String> = vec![collection_id.clone()];
        query_params.extend_from_slice(providers);
        let mut stmt = conn.prepare(&query)?;
        stmt.query_row(rusqlite::params_from_iter(query_params.iter()), |r| {
            r.get::<_, u32>(0)
        })? as usize
    };
    Ok(count)
}
