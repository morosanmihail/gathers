// Data retrieved from https://github.com/poketrax/pokedata

mod models;
mod scraper;

use std::{collections::HashMap, sync::Arc};

use ::models::{Card, CardID, CollectorNumber, Set, SetCode, filters::{CardSearchFilters, SortField, SortOrder}};
use models::SqlPokemonCard;
use rusqlite::Connection;
use tokio::sync::Mutex;

use crate::{NamedRetrievalSystem, RetrievalSystemTrait};

impl NamedRetrievalSystem for PokemonSQLiteRetrievalSystem {
    fn name(&self) -> &str {
        "PokemonSQLite"
    }
}

#[derive(Debug, Clone)]
pub struct PokemonSQLiteRetrievalSystem {
    connection: Arc<tokio::sync::Mutex<Connection>>,
    _db_path: String,
}

impl PokemonSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/pokemon.db".to_string());
        let conn = Connection::open(path.clone())?;
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            _db_path: path,
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
            params.push(rarity.to_single_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use ::models::pokemon::EnergyType;
    use tempfile::TempDir;

    async fn setup_test_db() -> PokemonSQLiteRetrievalSystem {
        PokemonSQLiteRetrievalSystem::new(None).unwrap()
    }

    #[tokio::test]
    async fn test_new_with_custom_path() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let system = PokemonSQLiteRetrievalSystem::new(Some(db_path.to_string_lossy().to_string()));
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
}
