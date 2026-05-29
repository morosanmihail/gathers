// Data retrieved from https://github.com/poketrax/pokedata

mod models;
mod scraper;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use ::models::{Card, CardID, CardPrices, CollectorNumber, RetailerPrices, Set, SetCode, filters::{CardSearchFilters, SortField, SortOrder}};
use models::SqlPokemonCard;
use rusqlite::Connection;
use tokio::sync::Mutex;

use crate::{NamedRetrievalSystem, RetrievalSystemTrait};
use crate::http::stream_to_file;

impl NamedRetrievalSystem for PokemonSQLiteRetrievalSystem {
    fn name(&self) -> &str {
        "PokemonSQLite"
    }
}

#[derive(Debug, Clone)]
pub struct PokemonSQLiteRetrievalSystem {
    connection: Arc<tokio::sync::Mutex<Connection>>,
    _db_path: String,
    prices_db_path: Option<String>,
    prices_connection: Arc<Mutex<Option<Connection>>>,
}

impl PokemonSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>, prices_db_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/pokemon.db".to_string());
        let conn = Connection::open(path.clone())?;
        let prices_conn = if let Some(ref p) = prices_db_path
            && PathBuf::from(p).exists()
        {
            Arc::new(Mutex::new(Some(Connection::open(p)?)))
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
            conditions.push(format!("expName LIKE ?{i}"));
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
        let sort_col = match &filters.sort_by {
            Some(SortField::Rarity) => "rarity",
            Some(SortField::SetCode) => "expName",
            Some(SortField::CollectorNumber) => "CAST(expCardNumber AS INTEGER)",
            Some(SortField::Artist) => "name",
            _ => "name",
        };
        let sort_dir = if matches!(&filters.sort_order, Some(SortOrder::Desc)) { "DESC" } else { "ASC" };
        query.push_str(&format!(" ORDER BY {sort_col} COLLATE NOCASE {sort_dir}"));
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {limit}"));
        } else {
            query.push_str(" LIMIT 1");
        }
        if let Some(skip) = skip {
            query.push_str(format!(" OFFSET {skip}").as_str());
        }

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
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT cardId, name, expName, rarity, energyType, cardType, img, expCardNumber, pokedex FROM cards WHERE cardId IN ({})",
            placeholders
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
        let placeholders = cards.iter().map(|_| "(?,?)").collect::<Vec<_>>().join(",");
        let mut params = vec![];
        cards.iter().for_each(|c| {
            params.push(c.0.clone());
            params.push(c.1.clone());
        });
        let query = format!(
            "SELECT cardId, expName, expCardNumber FROM cards WHERE (expName, expCardNumber) IN (VALUES {});",
            placeholders
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
            *conn_guard = Some(Connection::open(&prices_path)?);
        }
        let conn = conn_guard.as_ref().unwrap();
        // Use correlated subqueries to get the most recent non-zero value per field
        // independently. The latest row often has all zeros when a card stops being
        // tracked, but older rows retain the last known price.
        let result = conn.query_row(
            "SELECT \
               (SELECT rawPrice        FROM prices WHERE cardId = ?1 AND rawPrice        > 0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceTen  FROM prices WHERE cardId = ?1 AND gradedPriceTen  > 0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceNine FROM prices WHERE cardId = ?1 AND gradedPriceNine > 0 ORDER BY date DESC LIMIT 1) \
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
                let prices = row_to_card_prices(uuid, raw, psa10, psa9);
                if prices.paper.is_empty() { Ok(None) } else { Ok(Some(prices)) }
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
            *conn_guard = Some(Connection::open(&prices_path)?);
        }
        let conn = conn_guard.as_ref().unwrap();
        let placeholders = uuids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // Correlated subqueries find the most recent non-zero value per field per card,
        // avoiding rows where recent updates zeroed out previously-known prices.
        let query = format!(
            "SELECT \
               id_list.cardId, \
               (SELECT rawPrice        FROM prices WHERE cardId = id_list.cardId AND rawPrice        > 0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceTen  FROM prices WHERE cardId = id_list.cardId AND gradedPriceTen  > 0 ORDER BY date DESC LIMIT 1), \
               (SELECT gradedPriceNine FROM prices WHERE cardId = id_list.cardId AND gradedPriceNine > 0 ORDER BY date DESC LIMIT 1) \
             FROM (SELECT DISTINCT cardId FROM prices WHERE cardId IN ({})) id_list",
            placeholders
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
            let prices = row_to_card_prices(&card_id, raw, psa10, psa9);
            if !prices.paper.is_empty() {
                result.insert(card_id, prices);
            }
        }
        Ok(result)
    }

    async fn update_prices(&self) -> eyre::Result<bool> {
        let prices_path = match &self.prices_db_path {
            Some(p) => p.clone(),
            None => return Ok(false),
        };
        download_pokemon_prices(&prices_path).await?;
        *self.prices_connection.lock().await = None;
        Ok(true)
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        scraper::run(scraper::Options {
            db_path: self._db_path.clone(),
            recent: None,
            fresh: false,
        })
        .await?;
        Ok(true)
    }
}

fn row_to_card_prices(uuid: &str, raw: f64, psa10: f64, psa9: f64) -> CardPrices {
    let mut paper = HashMap::new();
    if raw > 0.0 {
        paper.insert("raw".to_string(), RetailerPrices { normal: Some(raw), foil: None });
    }
    if psa10 > 0.0 {
        paper.insert("graded_psa10".to_string(), RetailerPrices { normal: Some(psa10), foil: None });
    }
    if psa9 > 0.0 {
        paper.insert("graded_psa9".to_string(), RetailerPrices { normal: Some(psa9), foil: None });
    }
    CardPrices { uuid: uuid.to_string(), paper }
}

pub async fn download_pokemon_prices(path: &str) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str =
        "https://github.com/poketrax/pokedata/raw/refs/heads/main/databases/prices.sqlite";

    let target = PathBuf::from(path);
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("prices.sqlite");

    println!("Downloading Pokemon prices from {DOWNLOAD_URL}...");
    stream_to_file(DOWNLOAD_URL, "Download complete", &temp_path, None, "downloading").await?;

    std::fs::copy(&temp_path, &target)?;
    println!("Pokemon prices saved to {target:?}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::models::pokemon::EnergyType;
    use tempfile::TempDir;

    async fn setup_test_db() -> PokemonSQLiteRetrievalSystem {
        PokemonSQLiteRetrievalSystem::new(None, None).unwrap()
    }

    #[tokio::test]
    async fn test_new_with_custom_path() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let system = PokemonSQLiteRetrievalSystem::new(Some(db_path.to_string_lossy().to_string()), None);
        assert!(system.is_ok());
        let system = system.unwrap();
        assert_eq!(system._db_path, db_path.to_string_lossy().to_string());
    }

    #[tokio::test]
    async fn test_search_by_name() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            name: Some("Bulbasaur".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(2))
            .await
            .unwrap();
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().all(|c| {
            if let Card::Pokemon(p) = c {
                p.name.contains("Bulbasaur")
            } else {
                false
            }
        }));
    }

    #[tokio::test]
    async fn test_search_by_name_partial() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            name: Some("charme".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
        for card in cards {
            if let Card::Pokemon(p) = card {
                assert!(p.name.contains("Charmeleon"))
            }
        }
    }

    #[tokio::test]
    async fn test_search_by_set_code() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            set_code: Some("Jungle".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
        for card in cards {
            if let Card::Pokemon(p) = card {
                assert_eq!(p.set_code, "Jungle");
            } else {
                panic!("expected Pokemon card");
            }
        }
    }

    #[tokio::test]
    async fn test_search_by_collector_number() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            collector_number: Some("63".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
        if let Card::Pokemon(p) = &cards[0] {
            assert_eq!(p.collector_number, "063");
        } else {
            panic!("expected Pokemon card");
        }
    }

    #[tokio::test]
    async fn test_search_by_energy_type() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            energy_types: Some(vec![EnergyType::Fire]),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
        assert!(cards.iter().all(|c| {
            if let Card::Pokemon(p) = c {
                p.energy_types.contains(&EnergyType::Fire)
            } else {
                false
            }
        }));
    }

    #[tokio::test]
    async fn test_search_by_card_type() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            types: Some(vec!["Trainer".to_string()]),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert_eq!(cards.len(), 10);
        for card in cards {
            if let Card::Pokemon(p) = card {
                assert_eq!(p.card_type, "Trainer");
                assert!(p.pokedex.is_none());
            } else {
                panic!("expected Pokemon card");
            }
        }
    }

    #[tokio::test]
    async fn test_search_with_limit() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters::default();
        let cards = system
            .search_cards(filters, Some(0), Some(3))
            .await
            .unwrap();
        assert_eq!(cards.len(), 3);
    }

    #[tokio::test]
    async fn test_search_with_skip_and_limit() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters::default();
        let all = system
            .search_cards(filters.clone(), Some(0), Some(10))
            .await
            .unwrap();
        let page2 = system
            .search_cards(filters, Some(3), Some(3))
            .await
            .unwrap();
        assert_eq!(all.len(), 10);
        assert_eq!(page2.len(), 3);
    }

    #[tokio::test]
    async fn test_search_empty_result() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            name: Some("Cucuriguuuuu".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(10))
            .await
            .unwrap();
        assert!(cards.is_empty());
    }

    #[tokio::test]
    async fn test_get_cards_by_ids() {
        let system = setup_test_db().await;
        let ids = vec![
            "Pokemon-Go-Bulbasaur-001".to_string(),
            "Supreme-Victors-Bulbasaur-93".to_string(),
        ];
        let cards = system.get_cards_by_ids(ids).await.unwrap();
        assert_eq!(cards.len(), 2);
        assert!(cards.contains_key("Pokemon-Go-Bulbasaur-001"));
        assert!(cards.contains_key("Supreme-Victors-Bulbasaur-93"));
        if let Card::Pokemon(p) = &cards["Pokemon-Go-Bulbasaur-001"] {
            assert_eq!(p.name, "Bulbasaur");
        } else {
            panic!("expected Pokemon card");
        }
        if let Card::Pokemon(p) = &cards["Supreme-Victors-Bulbasaur-93"] {
            assert_eq!(p.name, "Bulbasaur");
        } else {
            panic!("expected Pokemon card");
        }
    }

    #[tokio::test]
    async fn test_get_cards_by_ids_empty() {
        let system = setup_test_db().await;
        let cards = system.get_cards_by_ids(vec![]).await.unwrap();
        assert!(cards.is_empty());
    }

    #[tokio::test]
    async fn test_get_sets() {
        let system = setup_test_db().await;
        let sets = system.get_sets().await.unwrap();
        assert!(sets.len() >= 157);
        let codes: Vec<&str> = sets.iter().map(|s| s.code.as_str()).collect();
        assert!(codes.contains(&"Base Set"));
        assert!(codes.contains(&"Jungle"));
    }

    #[tokio::test]
    async fn test_bulk_search_cards() {
        let system = setup_test_db().await;
        let query = vec![
            ("Base Set".to_string(), "044".to_string()),
            ("Base Set".to_string(), "004".to_string()),
        ];
        let results = system.bulk_search_cards(query).await.unwrap();
        assert_eq!(results.len(), 4);

        println!("{results:?}");
        let bulbasaur = results
            .iter()
            .find(|r| r.2 == "Base-Set-Bulbasaur-044")
            .unwrap();
        assert_eq!(bulbasaur.0, "Base Set");
        assert_eq!(bulbasaur.1, "044");
        let charizard = results
            .iter()
            .find(|r| r.2 == "Base-Set-Shadowless-Charizard-004")
            .unwrap();
        assert_eq!(charizard.0, "Base Set");
        assert_eq!(charizard.1, "004");
    }

    #[tokio::test]
    async fn test_bulk_search_cards_empty() {
        let system = setup_test_db().await;
        let results = system.bulk_search_cards(vec![]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_named_retrieval_system_trait() {
        let system = setup_test_db().await;
        assert_eq!(system.name(), "PokemonSQLite");
    }

    #[tokio::test]
    async fn test_pokedex_is_none_for_trainers() {
        let system = setup_test_db().await;
        let filters = CardSearchFilters {
            name: Some("Professor Oak".to_string()),
            ..Default::default()
        };
        let cards = system
            .search_cards(filters, Some(0), Some(1))
            .await
            .unwrap();
        assert_eq!(cards.len(), 1);
        if let Card::Pokemon(p) = &cards[0] {
            assert!(p.pokedex.is_none());
        } else {
            panic!("expected Pokemon card");
        }
    }

    // ── Price tests ───────────────────────────────────────────────────────────

    fn make_prices_db(dir: &TempDir) -> String {
        let path = dir.path().join("prices.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE prices (date TEXT, cardId TEXT, variant TEXT, rawPrice REAL, gradedPriceTen REAL, gradedPriceNine REAL);
             INSERT INTO prices VALUES ('2024-01-01', 'card-alpha', '', 1.50, 10.0, 8.0);
             INSERT INTO prices VALUES ('2024-01-10', 'card-alpha', '', 2.00, 12.0, 9.0);
             INSERT INTO prices VALUES ('2024-01-01', 'card-beta',  '', 0.25, 0.0,  0.0);
             INSERT INTO prices VALUES ('2024-01-01', 'card-zero',  '', 0.0,  0.0,  0.0);",
        ).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[tokio::test]
    async fn test_get_card_prices_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let result = system.get_card_prices("card-alpha").await.unwrap();
        assert!(result.is_some());
        let prices = result.unwrap();
        assert_eq!(prices.uuid, "card-alpha");
        let raw = prices.paper.get("raw").unwrap();
        assert_eq!(raw.normal, Some(2.00));
        assert_eq!(raw.foil, None);
        let psa10 = prices.paper.get("graded_psa10").unwrap();
        assert_eq!(psa10.normal, Some(12.0));
        let psa9 = prices.paper.get("graded_psa9").unwrap();
        assert_eq!(psa9.normal, Some(9.0));
    }

    #[tokio::test]
    async fn test_get_card_prices_latest_row_used() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        // card-alpha has two rows; latest (2024-01-10) must win
        let prices = system.get_card_prices("card-alpha").await.unwrap().unwrap();
        assert_eq!(prices.paper.get("raw").unwrap().normal, Some(2.00));
    }

    #[tokio::test]
    async fn test_get_card_prices_not_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let result = system.get_card_prices("card-nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_card_prices_all_zero_returns_none() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        // card-zero has all prices = 0.0 → paper map is empty → None
        let result = system.get_card_prices("card-zero").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_card_prices_no_prices_path() {
        let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system.get_card_prices("card-alpha").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_card_prices_file_missing() {
        let system = PokemonSQLiteRetrievalSystem::new(None, Some("/tmp/does_not_exist_pokemon_prices.sqlite".to_string())).unwrap();
        let result = system.get_card_prices("card-alpha").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_all_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let result = system
            .get_bulk_card_prices(vec!["card-alpha".to_string(), "card-beta".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("card-alpha"));
        assert!(result.contains_key("card-beta"));
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_partial_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let result = system
            .get_bulk_card_prices(vec!["card-alpha".to_string(), "card-missing".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("card-alpha"));
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_empty_input() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let result = system.get_bulk_card_prices(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_no_prices_path() {
        let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system
            .get_bulk_card_prices(vec!["card-alpha".to_string()])
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_update_prices_no_path_returns_false() {
        let system = PokemonSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system.update_prices().await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_prices_beta_raw_only() {
        let dir = TempDir::new().unwrap();
        let prices_path = make_prices_db(&dir);
        let system = PokemonSQLiteRetrievalSystem::new(None, Some(prices_path)).unwrap();

        let prices = system.get_card_prices("card-beta").await.unwrap().unwrap();
        assert_eq!(prices.paper.len(), 1);
        assert_eq!(prices.paper.get("raw").unwrap().normal, Some(0.25));
        assert!(!prices.paper.contains_key("graded_psa10"));
        assert!(!prices.paper.contains_key("graded_psa9"));
    }
}
