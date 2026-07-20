mod models;
mod prices;
pub mod update;

pub use update::{download_mtg_db, download_prices};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use ::models::{
    Card, CardID, CardPrices, CollectorNumber, Set, SetCode,
    filters::{CardSearchFilters, SortField},
};
use models::{LEGALITY_FORMATS, SqlCard};
use rusqlite::Connection;
use tokio::sync::Mutex;
use tracing::info;

use crate::systems::sql_helpers::{
    sql_limit_offset, sql_pair_placeholders, sql_placeholders, sql_sort_dir,
};
use crate::{NamedRetrievalSystem, RetrievalSystemTrait};

impl NamedRetrievalSystem for MagicSQLiteRetrievalSystem {
    fn name(&self) -> &str {
        "MagicSQLite"
    }
}

#[derive(Debug, Clone)]
pub struct MagicSQLiteRetrievalSystem {
    connection: Arc<tokio::sync::Mutex<Connection>>,
    db_path: String,
    prices_path: Option<String>,
    prices_cache: Arc<Mutex<Option<HashMap<String, CardPrices>>>>,
}

fn open_mtg_connection(path: &str) -> eyre::Result<Connection> {
    let conn = Connection::open(path)?;

    // Blank DBs (e.g. tests) won't have a cards table; nothing to index.
    let cards_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cards')",
        [],
        |row| row.get(0),
    )?;
    if !cards_exists {
        return Ok(conn);
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cards_name_nocase ON cards (name COLLATE NOCASE);
         CREATE INDEX IF NOT EXISTS idx_cards_setcode_number ON cards (setCode, number);",
    )?;

    // user_version tracks FTS schema version:
    //   0 = fresh/re-downloaded DB, no FTS built yet
    //   1 = FTS built with default tokenizer (legacy, needs upgrade)
    //   2 = FTS built with trigram tokenizer (substring matching)
    // Drop and recreate whenever version < 2 so re-downloads and schema upgrades both work.
    let fts_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if fts_version < 2 {
        info!("Building MTG full-text search index...");
        conn.execute_batch(
            "DROP TABLE IF EXISTS cards_fts;
             CREATE VIRTUAL TABLE cards_fts USING fts5(
                 name, text, artist,
                 content='cards', content_rowid='rowid',
                 tokenize='trigram'
             );
             INSERT INTO cards_fts(cards_fts) VALUES('rebuild');",
        )?;
        conn.pragma_update(None, "user_version", 2i64)?;
        info!("MTG full-text search index ready");
    }

    Ok(conn)
}

impl MagicSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>, prices_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/testPrintings.db".to_string());
        let prices_cache = if let Some(ref p) = prices_path {
            if PathBuf::from(p).exists() {
                Arc::new(Mutex::new(Some(prices::load_prices_file(p)?)))
            } else {
                Arc::new(Mutex::new(None))
            }
        } else {
            Arc::new(Mutex::new(None))
        };
        Ok(Self {
            connection: Arc::new(Mutex::new(open_mtg_connection(&path)?)),
            db_path: path,
            prices_path,
            prices_cache,
        })
    }
}

/// `SELECT ... FROM ...` clause shared by `search_cards` and `get_cards_by_ids`, kept in
/// sync with the column names `SqlCard::from_row` reads by name.
fn select_base() -> String {
    let legality_cols: String = LEGALITY_FORMATS
        .iter()
        .map(|f| format!("l.{f}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT a.uuid, a.name, a.setCode, a.rarity, a.artist, a.colorIdentity, a.text, \
         b.scryfallId, a.number, a.subtypes, a.supertypes, a.types, a.manaCost, a.manaValue, \
         a.type, a.power, a.toughness, a.loyalty, a.defense, a.keywords, a.colors, a.finishes, \
         a.isReserved, a.isPromo, a.isReprint, a.borderColor, a.frameEffects, a.isFullArt, \
         a.watermark, a.flavorText, s.name AS set_name, {legality_cols} \
         FROM cards as a \
         JOIN cardIdentifiers as b ON a.uuid = b.uuid \
         LEFT JOIN sets as s ON s.code = a.setCode \
         LEFT JOIN cardLegalities as l ON l.uuid = a.uuid"
    )
}

impl RetrievalSystemTrait for MagicSQLiteRetrievalSystem {
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let conn = self.connection.lock().await;

        // Build FTS MATCH expression for name/text/artist (whole-word tokenised search).
        // Wrap each term in double-quotes so multi-word phrases match exactly; escape any
        // literal double-quotes in user input.
        let mut fts_parts: Vec<String> = Vec::new();
        if let Some(name) = &filters.name
            && !name.is_empty()
        {
            fts_parts.push(format!("name:\"{}\"", name.replace('"', "\"\"")));
        }
        if let Some(artist) = &filters.artist
            && !artist.is_empty()
        {
            fts_parts.push(format!("artist:\"{}\"", artist.replace('"', "\"\"")));
        }
        if let Some(text) = &filters.text
            && !text.is_empty()
        {
            fts_parts.push(format!("text:\"{}\"", text.replace('"', "\"\"")));
        }
        let use_fts = !fts_parts.is_empty();

        let base = select_base();
        let mut query = if use_fts {
            format!("{base} JOIN cards_fts ON cards_fts.rowid = a.rowid")
        } else {
            base
        };

        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();
        let mut i = 1;

        if use_fts {
            conditions.push(format!("cards_fts MATCH ?{i}"));
            params.push(fts_parts.join(" "));
            i += 1;
        }

        if let Some(colours) = &filters.color_identities {
            for colour in colours {
                conditions.push(format!("a.colorIdentity LIKE ?{i}"));
                params.push(format!("%{colour}%"));
                i += 1;
            }
        }
        if let Some(set_code) = &filters.set_code
            && !set_code.is_empty()
        {
            conditions.push(format!("a.setCode LIKE ?{i}"));
            params.push(set_code.to_string());
            i += 1;
        }
        if let Some(rarity) = &filters.rarity {
            conditions.push(format!("a.rarity = ?{i}"));
            params.push(rarity.to_single_string().to_owned());
            i += 1;
        }
        if let Some(collector_number) = &filters.collector_number
            && !collector_number.is_empty()
        {
            conditions.push(format!("a.number = ?{i}"));
            params.push(collector_number.to_string());
            i += 1;
        }
        if let Some(subtype) = &filters.subtypes
            && !subtype.is_empty()
        {
            for s in subtype {
                conditions.push(format!("a.subtypes LIKE ?{i}"));
                params.push(format!("%{s}%"));
                i += 1;
            }
        }
        if let Some(supertype) = &filters.supertypes
            && !supertype.is_empty()
        {
            conditions.push(format!("a.supertypes LIKE ?{i}"));
            params.push(format!("%{supertype}%"));
            i += 1;
        }
        if let Some(types) = &filters.types
            && !types.is_empty()
        {
            for t in types {
                conditions.push(format!("a.types LIKE ?{i}"));
                params.push(format!("%{t}%"));
                i += 1;
            }
        }
        if let Some(min) = filters.mana_value_min {
            conditions.push(format!("a.manaValue >= ?{i}"));
            params.push(min.to_string());
            i += 1;
        }
        if let Some(max) = filters.mana_value_max {
            conditions.push(format!("a.manaValue <= ?{i}"));
            params.push(max.to_string());
            i += 1;
        }
        if let Some(colors) = &filters.colors {
            for colour in colors {
                conditions.push(format!("a.colors LIKE ?{i}"));
                params.push(format!("%{colour}%"));
                i += 1;
            }
        }
        if let Some(keywords) = &filters.keywords
            && !keywords.is_empty()
        {
            for k in keywords {
                conditions.push(format!("a.keywords LIKE ?{i}"));
                params.push(format!("%{k}%"));
                i += 1;
            }
        }
        if let Some(power) = &filters.power
            && !power.is_empty()
        {
            conditions.push(format!("a.power = ?{i}"));
            params.push(power.to_string());
            i += 1;
        }
        if let Some(toughness) = &filters.toughness
            && !toughness.is_empty()
        {
            conditions.push(format!("a.toughness = ?{i}"));
            params.push(toughness.to_string());
            i += 1;
        }
        if let Some(loyalty) = &filters.loyalty
            && !loyalty.is_empty()
        {
            conditions.push(format!("a.loyalty = ?{i}"));
            params.push(loyalty.to_string());
            i += 1;
        }
        if let Some(defense) = &filters.defense
            && !defense.is_empty()
        {
            conditions.push(format!("a.defense = ?{i}"));
            params.push(defense.to_string());
            i += 1;
        }
        if let Some(is_reserved) = filters.is_reserved {
            conditions.push(if is_reserved {
                "a.isReserved = 1".to_string()
            } else {
                "(a.isReserved IS NULL OR a.isReserved = 0)".to_string()
            });
        }
        if let Some(is_promo) = filters.is_promo {
            conditions.push(if is_promo {
                "a.isPromo = 1".to_string()
            } else {
                "(a.isPromo IS NULL OR a.isPromo = 0)".to_string()
            });
        }
        if let Some(is_reprint) = filters.is_reprint {
            conditions.push(if is_reprint {
                "a.isReprint = 1".to_string()
            } else {
                "(a.isReprint IS NULL OR a.isReprint = 0)".to_string()
            });
        }
        if let Some(is_full_art) = filters.is_full_art {
            conditions.push(if is_full_art {
                "a.isFullArt = 1".to_string()
            } else {
                "(a.isFullArt IS NULL OR a.isFullArt = 0)".to_string()
            });
        }
        if let Some(border_color) = &filters.border_color
            && !border_color.is_empty()
        {
            conditions.push(format!("a.borderColor = ?{i} COLLATE NOCASE"));
            params.push(border_color.to_string());
            i += 1;
        }
        // Format name is interpolated directly into the column reference, so it must be
        // whitelisted against known `cardLegalities` columns to avoid SQL injection.
        if let Some(legal_in) = &filters.legal_in {
            let format = legal_in.to_lowercase();
            if LEGALITY_FORMATS.contains(&format.as_str()) {
                conditions.push(format!("l.{format} = ?{i}"));
                params.push("Legal".to_string());
            }
        }
        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }
        let sort_col = match &filters.sort_by {
            Some(SortField::Rarity) => "a.rarity",
            Some(SortField::SetCode) => "a.setCode",
            Some(SortField::CollectorNumber) => "CAST(a.number AS INTEGER)",
            Some(SortField::Artist) => "a.artist",
            _ => "a.name",
        };
        query.push_str(&format!(
            " ORDER BY {sort_col} COLLATE NOCASE {}{}",
            sql_sort_dir(&filters.sort_order),
            sql_limit_offset(limit, skip),
        ));

        let mut stmt = conn.prepare(&query)?;
        let user_iter =
            stmt.query_map(rusqlite::params_from_iter(params.iter()), SqlCard::from_row)?;

        Ok(user_iter.flatten().map(|c| Card::Magic(c.into())).collect())
    }

    async fn get_cards_by_ids(&self, ids: Vec<String>) -> eyre::Result<HashMap<String, Card>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.connection.lock().await;
        let base = select_base();
        let query = format!("{base} WHERE a.uuid IN ({})", sql_placeholders(ids.len()));
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(ids), SqlCard::from_row)?;
        Ok(iter
            .flatten()
            .map(|c| (c.id.clone(), Card::Magic(c.into())))
            .collect())
    }

    async fn get_sets(&self) -> eyre::Result<Vec<Set>> {
        let conn = self.connection.lock().await;
        let query = "SELECT DISTINCT c.setCode, COALESCE(s.name, '') FROM cards c LEFT JOIN sets s ON s.code = c.setCode ORDER BY c.setCode".to_string();
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map([], |row| {
            Ok(Set {
                code: row.get(0)?,
                name: row.get(1)?,
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
            "SELECT uuid, setCode, number FROM cards WHERE (setCode, number) IN (VALUES {});",
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
        let prices_path = match &self.prices_path {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        if !PathBuf::from(&prices_path).exists() {
            return Ok(None);
        }
        let mut cache = self.prices_cache.lock().await;
        if cache.is_none() {
            *cache = Some(prices::load_prices_file(&prices_path)?);
        }
        Ok(cache.as_ref().and_then(|m| m.get(uuid)).cloned())
    }

    async fn get_bulk_card_prices(
        &self,
        uuids: Vec<String>,
    ) -> eyre::Result<HashMap<String, CardPrices>> {
        let prices_path = match &self.prices_path {
            Some(p) => p.clone(),
            None => return Ok(HashMap::new()),
        };
        if !PathBuf::from(&prices_path).exists() {
            return Ok(HashMap::new());
        }
        let mut cache = self.prices_cache.lock().await;
        if cache.is_none() {
            *cache = Some(prices::load_prices_file(&prices_path)?);
        }
        let result = cache
            .as_ref()
            .map(|m| {
                uuids
                    .iter()
                    .filter_map(|id| m.get(id).map(|p| (id.clone(), p.clone())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(result)
    }

    async fn update_prices(&self) -> eyre::Result<bool> {
        let prices_path = match &self.prices_path {
            Some(p) => p.clone(),
            None => return Ok(false),
        };
        update::download_prices(&prices_path).await?;
        *self.prices_cache.lock().await = None;
        Ok(true)
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        update::download_mtg_db(&self.db_path, None).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
