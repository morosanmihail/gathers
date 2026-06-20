mod models;
mod update;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use ::models::{
    Card, CardID, CollectorNumber, Set, SetCode,
    filters::{CardSearchFilters, SortField},
};
use models::SqlCard;
use rusqlite::{Connection, params};
use tokio::sync::Mutex;

use crate::systems::sql_helpers::{
    sql_limit_offset, sql_pair_placeholders, sql_placeholders, sql_sort_dir,
};
use crate::{
    NamedRetrievalSystem, RetrievalSystemTrait, systems::riftsqlite::update::RiftboundCardFetcher,
};

impl NamedRetrievalSystem for RiftboundSQLiteRetrievalSystem {
    fn name(&self) -> &str {
        "RiftboundSQLite"
    }
}

#[derive(Debug, Clone)]
pub struct RiftboundSQLiteRetrievalSystem {
    connection: Arc<tokio::sync::Mutex<Connection>>,
    db_path: String,
}

impl RiftboundSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/riftbound.db".to_string());
        Ok(Self {
            connection: Arc::new(Mutex::new(Connection::open(path.clone())?)),
            db_path: path,
        })
    }
}

/// Scrapes the live Riftbound card gallery and writes the `cards` table to
/// `path`. The only authoritative source for this data — used both as the
/// live system's fallback when no mirror is configured, and by the mirror
/// server itself to build the snapshot it publishes.
pub(crate) async fn build_riftbound_db(path: &str) -> eyre::Result<()> {
    let db_path = path.to_string();
    tokio::task::spawn_blocking(move || -> eyre::Result<()> {
        let mut fetcher = RiftboundCardFetcher::new()?;
        let cards = fetcher.fetch_all_simplified()?;

        let mut conn = Connection::open(&db_path)?;
        let tx = conn.transaction()?;

        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS cards (
            id TEXT PRIMARY KEY,
            name TEXT,
            code TEXT,
            set_id TEXT,
            type TEXT,
            rarity TEXT,
            energy INTEGER,
            might INTEGER,
            image_url TEXT,
            domains TEXT,
            artists TEXT,
            text TEXT
        )",
        )?;

        for card in &cards {
            let collector_number: Option<String> = card.collector_number.as_ref().and_then(|v| {
                if v.is_null() {
                    None
                } else if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    Some(v.to_string())
                }
            });

            let domains = card.domain_ids.as_deref().unwrap_or(&[]).join(",");
            let artists = card.artists.as_deref().unwrap_or(&[]).join(",");

            tx.execute(
                "INSERT OR REPLACE INTO cards VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    card.id.as_deref(),
                    card.name.as_deref(),
                    collector_number.as_deref(),
                    card.set.as_deref(),
                    card.card_type.as_deref(),
                    card.rarity.as_deref(),
                    card.energy.as_deref(),
                    card.might.as_deref(),
                    card.image_url.as_deref(),
                    domains.as_str(),
                    artists.as_str(),
                    card.ability_html.as_deref(),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| eyre::eyre!("build_riftbound_db task panicked: {e}"))?
}

impl RetrievalSystemTrait for RiftboundSQLiteRetrievalSystem {
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let conn = self.connection.lock().await;
        let mut query =
            "SELECT id, name, set_id, rarity, artists, domains, text, image_url, code FROM cards"
                .to_string();
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        let mut i = 1;
        if let Some(name) = &filters.name
            && !name.is_empty()
        {
            conditions.push(format!("name LIKE ?{i}"));
            params.push(format!("%{name}%"));
            i += 1;
        }
        if let Some(colours) = &filters.color_identities {
            for colour in colours {
                conditions.push(format!("domains LIKE ?{i}"));
                params.push(format!("%{colour}%"));
                i += 1;
            }
        }
        if let Some(artist) = &filters.artist
            && !artist.is_empty()
        {
            conditions.push(format!("artists LIKE ?{i}"));
            params.push(format!("%{artist}%"));
            i += 1;
        }
        if let Some(text) = &filters.text
            && !text.is_empty()
        {
            conditions.push(format!("text LIKE ?{i}"));
            params.push(format!("%{text}%"));
            i += 1;
        }
        if let Some(set_code) = &filters.set_code
            && !set_code.is_empty()
        {
            conditions.push(format!("set_id LIKE ?{i}"));
            params.push(set_code.to_string());
            i += 1;
        }
        if let Some(rarity) = &filters.rarity {
            conditions.push(format!("rarity = ?{i}"));
            params.push(rarity.to_single_string().to_owned());
            i += 1;
        }
        if let Some(collector_number) = &filters.collector_number
            && !collector_number.is_empty()
        {
            conditions.push(format!("code = ?{i}"));
            params.push(collector_number.to_string());
            i += 1;
        }
        if let Some(domains) = &filters.domains {
            for domain in domains {
                conditions.push(format!("domains LIKE ?{i}"));
                params.push(format!("%{domain}%"));
                i += 1;
            }
        }
        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }
        let sort_col = match &filters.sort_by {
            Some(SortField::Rarity) => "rarity",
            Some(SortField::SetCode) => "set_id",
            Some(SortField::CollectorNumber) => "CAST(code AS INTEGER)",
            Some(SortField::Artist) => "artists",
            _ => "name",
        };
        query.push_str(&format!(
            " ORDER BY {sort_col} COLLATE NOCASE {}{}",
            sql_sort_dir(&filters.sort_order),
            sql_limit_offset(limit, skip),
        ));

        let mut stmt = conn.prepare(&query)?;
        let user_iter =
            stmt.query_map(rusqlite::params_from_iter(params.iter()), SqlCard::from_row)?;

        Ok(user_iter
            .flatten()
            .map(|c| Card::Riftbound(c.into()))
            .collect())
    }

    async fn get_cards_by_ids(&self, ids: Vec<String>) -> eyre::Result<HashMap<String, Card>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.connection.lock().await;
        let query = format!(
            "SELECT id, name, set_id, rarity, artists, domains, text, image_url, code FROM cards WHERE id IN ({})",
            sql_placeholders(ids.len())
        );
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(ids), SqlCard::from_row)?;
        Ok(iter
            .flatten()
            .map(|c| (c.id.clone(), Card::Riftbound(c.into())))
            .collect())
    }

    async fn get_sets(&self) -> eyre::Result<Vec<Set>> {
        let conn = self.connection.lock().await;
        let query = "SELECT DISTINCT set_id FROM cards".to_string();
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map([], |row| {
            Ok(Set {
                code: row.get(0)?,
                name: "".to_string(),
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
            "SELECT id, set_id, code FROM cards WHERE (set_id, code) IN (VALUES {});",
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

    async fn update_backend(&self) -> eyre::Result<bool> {
        let target = PathBuf::from(&self.db_path);
        if crate::mirror::try_mirrors("riftbound.sqlite", &target, None).await {
            return Ok(true);
        }
        build_riftbound_db(&self.db_path).await?;
        Ok(true)
    }
}
