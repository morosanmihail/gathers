use crate::{CollectionCard, CollectionCardsParams, CollectionSortField};
use models::CollectionID;
use models::filters::SortOrder;
use rusqlite::{Connection, params};

pub(super) fn add_cards(
    conn: &Connection,
    collection_id: &CollectionID,
    cards: &[CollectionCard],
) -> eyre::Result<Vec<CollectionCard>> {
    if cards.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = cards
        .iter()
        .map(|_| "(?, ?, ?, ?, ?, ?, ?)")
        .collect::<Vec<_>>()
        .join(",");
    let mut query_params: Vec<String> = vec![];
    for c in cards {
        query_params.push(c.uuid.clone());
        query_params.push(collection_id.clone());
        query_params.push(c.quantity.to_string());
        query_params.push(c.foil_quantity.to_string());
        query_params.push(c.time_added.clone());
        query_params.push(c.time_added.clone()); // timeupdated = timeadded on creation
        query_params.push(c.provider.clone());
    }
    let query = format!(
        "INSERT INTO cards (uuid, collection, quantity, foilquantity, timeadded, timeupdated, provider)
VALUES {}
ON CONFLICT (uuid, collection) DO UPDATE SET
 quantity = max(cards.quantity + EXCLUDED.quantity, 0),
 foilquantity = max(cards.foilquantity + EXCLUDED.foilquantity, 0),
 timeupdated = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
RETURNING uuid, collection, quantity, foilquantity, timeadded, provider",
        placeholders
    );
    let mut stmt = conn.prepare(&query)?;
    let result: Vec<CollectionCard> = stmt
        .query_map(rusqlite::params_from_iter(query_params.iter()), |row| {
            Ok(CollectionCard {
                uuid: row.get(0)?,
                collection: row.get(1)?,
                quantity: row.get(2)?,
                foil_quantity: row.get(3)?,
                time_added: row.get(4)?,
                provider: row.get(5)?,
            })
        })?
        .flatten()
        .collect();

    conn.execute(
        "DELETE FROM cards WHERE quantity = 0 AND foilquantity = 0",
        [],
    )?;

    Ok(result)
}

pub(super) fn get_paginated(
    conn: &Connection,
    collection_id: &CollectionID,
    params: CollectionCardsParams,
) -> eyre::Result<Vec<CollectionCard>> {
    let mut conditions = vec!["collection = ?1".to_string()];
    let mut query_params: Vec<String> = vec![collection_id.clone()];
    let mut i = 2usize;

    if let Some(provider) = &params.provider {
        conditions.push(format!("provider = ?{i}"));
        query_params.push(provider.clone());
        i += 1;
    } else if !params.providers.is_empty() {
        let placeholders: Vec<String> = params
            .providers
            .iter()
            .enumerate()
            .map(|(j, _)| format!("?{}", i + j))
            .collect();
        conditions.push(format!("provider IN ({})", placeholders.join(", ")));
        query_params.extend(params.providers.clone());
        i += params.providers.len();
    }

    let sort_col = match &params.sort_by {
        Some(CollectionSortField::Quantity) => "quantity",
        Some(CollectionSortField::FoilQuantity) => "foilquantity",
        Some(CollectionSortField::Provider) => "provider",
        _ => "timeadded",
    };
    let sort_dir = if matches!(&params.sort_order, Some(SortOrder::Desc)) {
        "DESC"
    } else {
        "ASC"
    };

    let query = format!(
        "SELECT uuid, quantity, foilquantity, timeadded, provider \
         FROM cards WHERE {} ORDER BY {} {} LIMIT ?{} OFFSET ?{}",
        conditions.join(" AND "),
        sort_col,
        sort_dir,
        i,
        i + 1,
    );
    query_params.push(params.limit.to_string());
    query_params.push(params.offset.to_string());

    let collection_id = collection_id.clone();
    let mut stmt = conn.prepare(&query)?;
    let cards: Vec<CollectionCard> = stmt
        .query_map(rusqlite::params_from_iter(query_params.iter()), |row| {
            Ok(CollectionCard {
                uuid: row.get(0)?,
                quantity: row.get(1)?,
                foil_quantity: row.get(2)?,
                time_added: row.get(3)?,
                collection: collection_id.clone(),
                provider: row.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(cards)
}
