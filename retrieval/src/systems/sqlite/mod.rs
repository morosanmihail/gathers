mod models;
mod prices;
mod unique;
pub mod update;

pub use update::{download_mtg_db, download_prices};

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use ::models::{
    Card, CardID, CardPrices, CollectorNumber, Set, SetCode,
    filters::{CardSearchFilters, SortField, SortOrder, UNIQUE_PRINTS, UniqueMode},
};
use models::{LEGALITY_FORMATS, SqlCard};
use rusqlite::{Connection, types::Value};
use tokio::sync::Mutex;
use tracing::info;

use crate::systems::sql_helpers::{sql_pair_placeholders, sql_placeholders, sql_sort_dir};
use crate::systems::mtg_unique_modes;
use crate::{NamedRetrievalSystem, RetrievalSystemTrait, resolve_unique_mode};

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
    /// SQL ranking which printing best represents a card in the `unique` modes, built once
    /// from the columns this database has (see `unique::build_rank_sql`).
    rank_sql: Arc<String>,
}

/// Indexes that keep searches off full-table scans and sorts. `CREATE INDEX IF NOT EXISTS`, so
/// a freshly downloaded DB gets them on first open (~1s) and an existing one pays nothing.
const SEARCH_INDEXES: &str = "
    CREATE INDEX IF NOT EXISTS idx_cards_name_nocase ON cards (name COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_cards_setcode_number ON cards (setCode, number);
    -- One per sortable column, in the collation `search_cards` sorts with, so ORDER BY streams
    -- off the index instead of sorting every matching card.
    CREATE INDEX IF NOT EXISTS idx_cards_artist_nocase ON cards (artist COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_cards_rarity_nocase ON cards (rarity COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_cards_setcode_nocase ON cards (setCode COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_cards_number_int ON cards (CAST(number AS INTEGER));
    -- Covers every column a filter can test (plus what `FRONT_FACE_UUID` reads), in name order.
    -- A filtered search walks this in name order and stops at the first page of matches, so
    -- covering it means filters like `power = 9` never touch the wide `cards` rows.
    CREATE INDEX IF NOT EXISTS idx_cards_search_cover ON cards (
        name COLLATE NOCASE, rarity, setCode, colorIdentity, colors, types, subtypes,
        supertypes, keywords, manaValue, power, toughness, loyalty, defense, borderColor,
        isReserved, isPromo, isReprint, isFullArt, side, number, uuid
    );";

fn open_mtg_connection(path: &str) -> eyre::Result<Connection> {
    let conn = Connection::open(path)?;
    // Several connections can open the same file at once (tests, mirror); wait out a
    // concurrent index build rather than failing with SQLITE_BUSY.
    conn.busy_timeout(Duration::from_secs(5))?;
    // Search statements come in a handful of shapes; keep them prepared.
    conn.set_prepared_statement_cache_capacity(64);
    // The DB is ~750MB and read-mostly: a bigger page cache and memory-mapped reads keep hot
    // pages out of read() calls, and in-memory temp storage keeps sorts off disk. The default
    // 2MB cache thrashes on a broad search.
    conn.execute_batch(
        "PRAGMA cache_size = -65536;
         PRAGMA mmap_size = 1073741824;
         PRAGMA temp_store = MEMORY;",
    )?;

    // Blank DBs (e.g. tests) won't have a cards table; nothing to index.
    let cards_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cards')",
        [],
        |row| row.get(0),
    )?;
    if !cards_exists {
        return Ok(conn);
    }

    let indexed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_cards_search_cover')",
        [],
        |row| row.get(0),
    )?;
    if !indexed {
        info!("Building MTG search indexes...");
    }
    conn.execute_batch(SEARCH_INDEXES)?;

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
        let conn = open_mtg_connection(&path)?;
        let rank_sql = Arc::new(unique::build_rank_sql(&conn));
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            db_path: path,
            prices_path,
            prices_cache,
            rank_sql,
        })
    }
}

/// SQL expression (over a `cards AS a` row) giving the uuid of the front face of the printing
/// `a` belongs to. MTGJSON stores every face of a double-faced, split or adventure card as its
/// own `cards` row; the faces share `setCode` and `number` and differ by `side` ('a' is the
/// front). Rows without a `number`, or with no siblings, are their own front face. Most cards
/// have no `side` at all, so those skip the sibling lookup.
const FRONT_FACE_UUID: &str = "CASE WHEN a.side IS NULL THEN a.uuid ELSE \
     COALESCE((SELECT f.uuid FROM cards AS f \
     WHERE f.setCode = a.setCode AND f.number = a.number ORDER BY f.side LIMIT 1), a.uuid) END";

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

/// Cards by uuid, straight from the `cards` table (faces are not collapsed). Rows that fail
/// to parse are skipped.
fn fetch_by_uuids(conn: &Connection, ids: &[String]) -> eyre::Result<HashMap<String, SqlCard>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let query = format!(
        "{} WHERE a.uuid IN ({})",
        select_base(),
        sql_placeholders(ids.len())
    );
    let mut stmt = conn.prepare_cached(&query)?;
    let iter = stmt.query_map(rusqlite::params_from_iter(ids), SqlCard::from_row)?;
    Ok(iter.flatten().map(|c| (c.id.clone(), c)).collect())
}

impl RetrievalSystemTrait for MagicSQLiteRetrievalSystem {
    fn unique_modes(&self) -> Vec<UniqueMode> {
        mtg_unique_modes()
    }

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

        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();
        let mut needs_legalities = false;
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
            // `LIKE` can't use an index, so a plain code is matched with `=` against the
            // NOCASE index; only a pattern with wildcards needs `LIKE`.
            if set_code.contains(['%', '_']) {
                conditions.push(format!("a.setCode LIKE ?{i}"));
            } else {
                conditions.push(format!("a.setCode = ?{i} COLLATE NOCASE"));
            }
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
                // The unary `+` stops the planner building a throwaway automatic index over
                // all of `cardLegalities` for this test (~800ms); it looks legalities up by
                // uuid instead.
                conditions.push(format!("+l.{format} = ?{i}"));
                params.push("Legal".to_string());
                needs_legalities = true;
            }
        }
        // Searching is two queries. The first works on narrow rows only: it applies the
        // filters and the sort and yields the front-face uuid of each match. Carrying the
        // wide card rows (rules text, legalities, ...) through the sort is what made broad
        // searches and non-name sorts slow. The second query then loads full rows for just
        // the page that is returned.
        //
        // `front_uuid` (see `FRONT_FACE_UUID`) identifies the printing a row belongs to.
        let mut from_where = String::from(" FROM cards AS a");
        if use_fts {
            from_where.push_str(" JOIN cards_fts ON cards_fts.rowid = a.rowid");
        }
        if needs_legalities {
            from_where.push_str(" JOIN cardLegalities AS l ON l.uuid = a.uuid");
        }
        if !conditions.is_empty() {
            from_where.push_str(" WHERE ");
            from_where.push_str(&conditions.join(" AND "));
        }
        // Each expression matches an index (see `SEARCH_INDEXES`) so the sort can stream.
        let sort_col = match &filters.sort_by {
            Some(SortField::Rarity) => "a.rarity COLLATE NOCASE",
            Some(SortField::SetCode) => "a.setCode COLLATE NOCASE",
            Some(SortField::CollectorNumber) => "CAST(a.number AS INTEGER)",
            Some(SortField::Artist) => "a.artist COLLATE NOCASE",
            _ => "a.name COLLATE NOCASE",
        };
        let sorted_by_name = sort_col.starts_with("a.name");
        let descending = matches!(&filters.sort_order, Some(SortOrder::Desc));

        let limit = limit.unwrap_or(1); // same default `sql_limit_offset` applies
        let skip = skip.unwrap_or(0);

        let modes = self.unique_modes();
        let mode = resolve_unique_mode(&modes, filters.unique.as_deref())?
            .map_or(UNIQUE_PRINTS, |m| m.id.as_str());

        let page: Vec<String> = if mode != UNIQUE_PRINTS {
            let key_sql = unique::key_sql(mode)?;
            let query = unique::UniqueQuery {
                from_where: &from_where,
                params: params.into_iter().map(Value::Text).collect(),
                key_sql: &key_sql,
                rank_sql: &self.rank_sql,
            };
            if sorted_by_name {
                query.by_name(&conn, descending, skip, limit)?
            } else {
                query.grouped(&conn, sort_col, descending, skip, limit)?
            }
        } else {
            let mut query = format!("SELECT {FRONT_FACE_UUID} AS front_uuid{from_where}");
            // The window size is a bound parameter so the statement can be cached.
            let mut values: Vec<Value> = params.into_iter().map(Value::Text).collect();
            let limit_param = values.len();
            query.push_str(&format!(
                " ORDER BY {sort_col} {} LIMIT ?{}",
                sql_sort_dir(&filters.sort_order),
                limit_param + 1,
            ));
            values.push(Value::Integer(0));

            // Each face of a double-faced card is its own row, but a card should be listed once.
            // Collapsing faces in SQL (by re-applying the filters to sibling rows) stops SQLite
            // from streaming rows off an index and stopping at LIMIT, which is very slow for
            // broad filters. Instead faces are collapsed here: the first row seen for a printing
            // wins, and pagination is applied after that. A small margin over what's needed almost
            // always suffices; if faces ate too many rows, retry with a bigger window.
            let wanted = skip + limit;
            let mut fetch = wanted + wanted / 8;
            loop {
                values[limit_param] = Value::Integer(fetch as i64);
                let mut stmt = conn.prepare_cached(&query)?;
                let mut rows = stmt.query(rusqlite::params_from_iter(values.iter()))?;
                let mut seen = HashSet::new();
                let mut fetched = 0;
                let mut position = 0; // printings seen so far, including the skipped ones
                let mut page = Vec::with_capacity(limit);
                while let Some(row) = rows.next()? {
                    fetched += 1;
                    let front_uuid: String = row.get(0)?;
                    if !seen.insert(front_uuid.clone()) {
                        continue;
                    }
                    position += 1;
                    if position > skip {
                        page.push(front_uuid);
                    }
                    if position == wanted {
                        break;
                    }
                }
                if position >= wanted || fetched < fetch {
                    break page;
                }
                fetch *= 2;
            }
        };

        // Always load the front face, so a card has the same id (and details) however it was
        // found, including when only its back face matched. Ids that don't resolve (rows
        // that fail to parse) are skipped, as before.
        let mut cards = fetch_by_uuids(&conn, &page)?;
        Ok(page
            .iter()
            .filter_map(|id| cards.remove(id))
            .map(|c| Card::Magic(c.into()))
            .collect())
    }

    async fn get_cards_by_ids(&self, ids: Vec<String>) -> eyre::Result<HashMap<String, Card>> {
        let conn = self.connection.lock().await;
        Ok(fetch_by_uuids(&conn, &ids)?
            .into_iter()
            .map(|(id, c)| (id, Card::Magic(c.into())))
            .collect())
    }

    async fn get_sets(&self) -> eyre::Result<Vec<Set>> {
        let conn = self.connection.lock().await;
        // Walk the (small) `sets` table and probe `cards` by set code, instead of scanning every
        // card for its distinct set code. Every card's set has a `sets` row in MTGJSON.
        let query = "SELECT s.code, COALESCE(s.name, '') FROM sets s \
                     WHERE EXISTS (SELECT 1 FROM cards c WHERE c.setCode = s.code) \
                     ORDER BY s.code";
        let mut stmt = conn.prepare_cached(query)?;
        let iter = stmt.query_map([], |row| {
            Ok(Set {
                code: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        Ok(iter.flatten().collect())
    }

    async fn get_random_card(&self) -> eyre::Result<Option<Card>> {
        let conn = self.connection.lock().await;
        let base = select_base();
        // LIMIT to a batch rather than 1: a handful of legacy/malformed rows
        // fail to parse (same tolerance `search_cards` relies on via
        // `.flatten()`), so pick the first parseable row out of a
        // randomly-ordered batch rather than erroring on a single bad draw.
        let query = format!("{base} WHERE a.uuid = {FRONT_FACE_UUID} ORDER BY RANDOM() LIMIT 50");
        let mut stmt = conn.prepare_cached(&query)?;
        let user_iter = stmt.query_map([], SqlCard::from_row)?;
        Ok(user_iter.flatten().next().map(|c| Card::Magic(c.into())))
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
            "SELECT uuid, setCode, number, side FROM cards WHERE (setCode, number) IN (VALUES {});",
            sql_pair_placeholders(cards.len())
        );
        let mut stmt = conn.prepare_cached(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        // Every face of a double-faced card matches its set and number; keep the front one
        // (no `side`, else the lowest) so a card resolves to a single id.
        let mut resolved: Vec<(SetCode, CollectorNumber, CardID, Option<String>)> = vec![];
        let mut index: HashMap<(String, String), usize> = HashMap::new();
        for (id, set, num, side) in iter.flatten() {
            match index.get(&(set.clone(), num.clone())) {
                Some(&i) => {
                    if side < resolved[i].3 {
                        resolved[i] = (set, num, id, side);
                    }
                }
                None => {
                    index.insert((set.clone(), num.clone()), resolved.len());
                    resolved.push((set, num, id, side));
                }
            }
        }
        Ok(resolved
            .into_iter()
            .map(|(set, num, id, _)| (set, num, id))
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
