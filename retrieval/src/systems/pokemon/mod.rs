// Data retrieved from https://github.com/poketrax/pokedata

mod models;
mod prices;
mod scraper;

pub use prices::download_pokemon_prices;

/// Runs the live Pokémon card scraper, upserting into the db at `path`
/// (created if missing). The only authoritative source for this data —
/// used both as the live system's fallback when no mirror is configured,
/// and by the mirror server itself to build the snapshot it publishes.
///
/// Always incremental (`fresh: false`): the scrape hits many upstream
/// sources across many requests, and partial failures are routine (a
/// single set or source erroring just leaves its rows untouched). Wiping
/// the db first would turn a transient upstream hiccup into data loss —
/// callers that want a persistent, self-healing db should pass the same
/// `path` across repeated calls.
pub(crate) async fn scrape_to_path(path: &str) -> eyre::Result<()> {
    scraper::run(scraper::Options {
        db_path: path.to_string(),
        recent: None,
        fresh: false,
    })
    .await
}

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use ::models::{
    Card, CardID, CardPrices, CollectorNumber, Set, SetCode, filters::CardSearchFilters,
};
use models::SqlPokemonCard;
use rusqlite::Connection;
use tokio::sync::Mutex;

use tracing::info;

use crate::systems::sql_helpers::{
    sql_limit_offset, sql_pair_placeholders, sql_placeholders, sql_sort_dir,
};
use crate::{NamedRetrievalSystem, RetrievalSystemTrait};

impl NamedRetrievalSystem for PokemonSQLiteRetrievalSystem {
    fn name(&self) -> &str {
        "PokemonSQLite"
    }
}

#[derive(Debug, Clone)]
pub struct PokemonSQLiteRetrievalSystem {
    connection: Arc<tokio::sync::Mutex<Connection>>,
    pub(super) _db_path: String,
    prices_db_path: Option<String>,
    prices_connection: Arc<Mutex<Option<Connection>>>,
}

fn open_prices_connection(path: &str) -> eyre::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_prices_cardid_date ON prices(cardId, date DESC);
         CREATE INDEX IF NOT EXISTS idx_prices_covering ON prices(cardId, date DESC, rawPrice, gradedPriceTen, gradedPriceNine);"
    )?;
    Ok(conn)
}

impl PokemonSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>, prices_db_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/pokemon.db".to_string());
        let conn = Connection::open(path.clone())?;
        let prices_conn = if let Some(ref p) = prices_db_path
            && PathBuf::from(p).exists()
        {
            Arc::new(Mutex::new(Some(open_prices_connection(p)?)))
        } else {
            Arc::new(Mutex::new(None))
        };
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            _db_path: path,
            prices_db_path,
            prices_connection: prices_conn,
        })
    }
}

impl RetrievalSystemTrait for PokemonSQLiteRetrievalSystem {
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let conn = self.connection.lock().await;
        let mut query =
            "SELECT cardId, name, expName, rarity, energyType, cardType, img, expCardNumber, pokedex FROM cards"
                .to_string();
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        let mut i = 1;
        if let Some(name) = &filters.name
            && !name.is_empty()
        {
            conditions.push(format!("name LIKE ?{i}"));
            params.push(format!("%{name}%"));
            i += 1;
        }
        if let Some(set_code) = &filters.set_code
            && !set_code.is_empty()
        {
            conditions.push(format!(
                "(expName LIKE ?{i} OR expIdTCGP LIKE ?{i} OR expCodeTCGP LIKE ?{i})"
            ));
            params.push(format!("%{set_code}%"));
            i += 1;
        }
        if let Some(collector_number) = &filters.collector_number
            && !collector_number.is_empty()
        {
            conditions.push(format!("expCardNumber = ?{i}"));
            params.push(format!("{:0>3}", collector_number));
            i += 1;
        }
        if let Some(rarity) = &filters.rarity {
            conditions.push(format!("rarity = ?{i}"));
            params.push(rarity.to_single_string().to_owned());
            i += 1;
        }
        if let Some(energy_types) = &filters.energy_types {
            for energy_type in energy_types {
                conditions.push(format!("energyType LIKE ?{i}"));
                params.push(format!("%{energy_type}%"));
                i += 1;
            }
        }
        if let Some(types) = &filters.types
            && !types.is_empty()
        {
            for t in types {
                conditions.push(format!("cardType LIKE ?{i}"));
                params.push(format!("%{t}%"));
                i += 1;
            }
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }
        use ::models::filters::SortField;
        let sort_col = match &filters.sort_by {
            Some(SortField::Rarity) => "rarity",
            Some(SortField::SetCode) => "expName",
            Some(SortField::CollectorNumber) => "CAST(expCardNumber AS INTEGER)",
            Some(SortField::Artist) => "name",
            _ => "name",
        };
        query.push_str(&format!(
            " ORDER BY {sort_col} COLLATE NOCASE {}{}",
            sql_sort_dir(&filters.sort_order),
            sql_limit_offset(limit, skip),
        ));

        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            SqlPokemonCard::from_row,
        )?;
        Ok(iter.flatten().map(|c| Card::Pokemon(c.into())).collect())
    }

    async fn get_cards_by_ids(&self, ids: Vec<String>) -> eyre::Result<HashMap<String, Card>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.connection.lock().await;
        let query = format!(
            "SELECT cardId, name, expName, rarity, energyType, cardType, img, expCardNumber, pokedex FROM cards WHERE cardId IN ({})",
            sql_placeholders(ids.len())
        );
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(ids), SqlPokemonCard::from_row)?;
        Ok(iter
            .flatten()
            .map(|c| (c.id.clone(), Card::Pokemon(c.into())))
            .collect())
    }

    async fn get_sets(&self) -> eyre::Result<Vec<Set>> {
        let conn = self.connection.lock().await;
        let mut stmt =
            conn.prepare("SELECT DISTINCT expName FROM cards WHERE expName IS NOT NULL")?;
        let iter = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(Set {
                code: name.clone(),
                name,
            })
        })?;
        Ok(iter.flatten().collect())
    }

    async fn bulk_search_cards(
        &self,
        cards: Vec<(SetCode, CollectorNumber)>,
    ) -> eyre::Result<Vec<(SetCode, CollectorNumber, CardID)>> {
        if cards.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.connection.lock().await;
        let mut params = vec![];
        cards.iter().for_each(|c| {
            params.push(c.0.clone());
            params.push(c.1.clone());
        });
        let query = format!(
            "SELECT cardId, expName, expCardNumber FROM cards WHERE (expName, expCardNumber) IN (VALUES {});",
            sql_pair_placeholders(cards.len())
        );
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(iter
            .flatten()
            .map(|(id, set, num): (String, String, String)| (set, num, id))
            .collect())
    }

    async fn get_card_prices(&self, uuid: &str) -> eyre::Result<Option<CardPrices>> {
        let prices_path = match &self.prices_db_path {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        if !PathBuf::from(&prices_path).exists() {
            return Ok(None);
        }
        let mut conn_guard = self.prices_connection.lock().await;
        if conn_guard.is_none() {
            *conn_guard = Some(open_prices_connection(&prices_path)?);
        }
        let conn = conn_guard.as_ref().unwrap();
        let result = conn.query_row(
            "SELECT \
               (SELECT rawPrice        FROM prices WHERE cardId = ?1 AND rawPrice        > 0 AND rawPrice        != 20.0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceTen  FROM prices WHERE cardId = ?1 AND gradedPriceTen  > 0 AND gradedPriceTen  != 20.0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceNine FROM prices WHERE cardId = ?1 AND gradedPriceNine > 0 AND gradedPriceNine != 20.0 ORDER BY date DESC LIMIT 1) \
             WHERE EXISTS (SELECT 1 FROM prices WHERE cardId = ?1)",
            rusqlite::params![uuid],
            |row| Ok((
                row.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            )),
        );
        match result {
            Ok((raw, psa10, psa9)) => {
                let prices = prices::row_to_card_prices(uuid, raw, psa10, psa9);
                if prices.paper.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(prices))
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn get_bulk_card_prices(
        &self,
        uuids: Vec<String>,
    ) -> eyre::Result<HashMap<String, CardPrices>> {
        if uuids.is_empty() {
            return Ok(HashMap::new());
        }
        let prices_path = match &self.prices_db_path {
            Some(p) => p.clone(),
            None => return Ok(HashMap::new()),
        };
        if !PathBuf::from(&prices_path).exists() {
            return Ok(HashMap::new());
        }
        let mut conn_guard = self.prices_connection.lock().await;
        if conn_guard.is_none() {
            *conn_guard = Some(open_prices_connection(&prices_path)?);
        }
        let conn = conn_guard.as_ref().unwrap();
        let query = format!(
            "SELECT \
               id_list.cardId, \
               (SELECT rawPrice        FROM prices WHERE cardId = id_list.cardId AND rawPrice        > 0 AND rawPrice        != 20.0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceTen  FROM prices WHERE cardId = id_list.cardId AND gradedPriceTen  > 0 AND gradedPriceTen  != 20.0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceNine FROM prices WHERE cardId = id_list.cardId AND gradedPriceNine > 0 AND gradedPriceNine != 20.0 ORDER BY date DESC LIMIT 1) \
             FROM (SELECT DISTINCT cardId FROM prices WHERE cardId IN ({})) id_list",
            sql_placeholders(uuids.len())
        );
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(uuids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            ))
        })?;
        let mut result = HashMap::new();
        for row in iter.flatten() {
            let (card_id, raw, psa10, psa9) = row;
            let card_prices = prices::row_to_card_prices(&card_id, raw, psa10, psa9);
            if !card_prices.paper.is_empty() {
                result.insert(card_id, card_prices);
            }
        }
        Ok(result)
    }

    async fn update_prices(&self) -> eyre::Result<bool> {
        let prices_path = match &self.prices_db_path {
            Some(p) => p.clone(),
            None => return Ok(false),
        };
        prices::download_pokemon_prices(&prices_path).await?;
        *self.prices_connection.lock().await = None;
        Ok(true)
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        let target = PathBuf::from(&self._db_path);
        if crate::mirror::try_mirrors("pokemon.sqlite", &target, None).await {
            return Ok(true);
        }
        info!("Updating Pokemon backend");
        scraper::run(scraper::Options {
            db_path: self._db_path.clone(),
            recent: None,
            fresh: false,
        })
        .await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
