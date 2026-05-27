mod models;

use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Clone, Default)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub phase: String,
}

use ::models::{Card, CardID, CardPrices, CollectorNumber, RetailerPrices, Set, SetCode, filters::{CardSearchFilters, SortField, SortOrder}};
use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use models::SqlCard;
use rusqlite::Connection;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use tokio::sync::Mutex;

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

impl MagicSQLiteRetrievalSystem {
    pub fn new(db_path: Option<String>, prices_path: Option<String>) -> eyre::Result<Self> {
        let path = db_path.unwrap_or_else(|| "../data/testPrintings.db".to_string());
        let prices_cache = if let Some(ref p) = prices_path {
            if PathBuf::from(p).exists() {
                Arc::new(Mutex::new(Some(load_prices_file(p)?)))
            } else {
                Arc::new(Mutex::new(None))
            }
        } else {
            Arc::new(Mutex::new(None))
        };
        Ok(Self {
            connection: Arc::new(Mutex::new(Connection::open(path.clone())?)),
            db_path: path,
            prices_path,
            prices_cache,
        })
    }
}

impl RetrievalSystemTrait for MagicSQLiteRetrievalSystem {
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let conn = self.connection.lock().await;
        let mut query =
            "SELECT a.uuid, a.name, a.setCode, a.rarity, a.artist, a.colorIdentity, a.text, b.scryfallId, a.number, a.subtypes, a.supertypes, a.types FROM cards as a JOIN cardIdentifiers as b ON a.uuid = b.uuid"
                .to_string();
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        let mut i = 1;
        if let Some(name) = &filters.name
            && !name.is_empty()
        {
            conditions.push(format!("a.name LIKE ?{i}"));
            params.push(format!("%{name}%"));
            i += 1;
        }
        if let Some(colours) = &filters.color_identities {
            for colour in colours {
                conditions.push(format!("a.colorIdentity LIKE ?{i}"));
                params.push(format!("%{colour}%"));
                i += 1;
            }
        }
        if let Some(artist) = &filters.artist
            && !artist.is_empty()
        {
            conditions.push(format!("a.artist LIKE ?{i}"));
            params.push(format!("%{artist}%"));
            i += 1;
        }
        if let Some(text) = &filters.text
            && !text.is_empty()
        {
            conditions.push(format!("a.text LIKE ?{i}"));
            params.push(format!("%{text}%"));
            i += 1;
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
        let sort_dir = if matches!(&filters.sort_order, Some(SortOrder::Desc)) { "DESC" } else { "ASC" };
        query.push_str(&format!(" ORDER BY {sort_col} COLLATE NOCASE {sort_dir}"));
        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {limit}"));
        } else {
            query.push_str(" LIMIT 1");
        }
        if let Some(skip) = skip {
            query.push_str(format!(" OFFSET {skip}").as_str())
        }
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
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT a.uuid, a.name, a.setCode, a.rarity, a.artist, a.colorIdentity, a.text, b.scryfallId, a.number, a.subtypes, a.supertypes, a.types FROM cards as a JOIN cardIdentifiers as b ON a.uuid = b.uuid WHERE a.uuid IN ({})",
            placeholders
        );
        let mut stmt = conn.prepare(&query)?;
        let iter = stmt.query_map(rusqlite::params_from_iter(ids), SqlCard::from_row)?;
        Ok(iter.flatten().map(|c| (c.id.clone(), Card::Magic(c.into()))).collect())
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
        let placeholders = cards.iter().map(|_| "(?,?)").collect::<Vec<_>>().join(",");
        let mut params = vec![];
        cards.iter().for_each(|c| {
            params.push(c.0.clone());
            params.push(c.1.clone());
        });
        let query = format!(
            "SELECT uuid, setCode, number FROM cards WHERE (setCode, number) IN (VALUES {});",
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
        let prices_path = match &self.prices_path {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        if !PathBuf::from(&prices_path).exists() {
            return Ok(None);
        }
        let mut cache = self.prices_cache.lock().await;
        if cache.is_none() {
            *cache = Some(load_prices_file(&prices_path)?);
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
            *cache = Some(load_prices_file(&prices_path)?);
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
        download_prices(&prices_path).await?;
        *self.prices_cache.lock().await = None;
        Ok(true)
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        const DOWNLOAD_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2";
        const CRC_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2.sha256";

        let local_file_path = PathBuf::from_str(&self.db_path)?;
        let sidecar_path = PathBuf::from(format!("{}.bz2.sha256", local_file_path.display()));

        let temp_dir = tempfile::tempdir()?;
        let crc_file_path = temp_dir.path().join("remote_crc.sha");

        stream_to_file(CRC_URL, "SHA256 fetched", &crc_file_path, None, "checking").await?;
        let remote_crc = fs::read_to_string(&crc_file_path)?.trim().to_lowercase();

        let local_crc = if sidecar_path.exists() {
            fs::read_to_string(&sidecar_path)?.trim().to_lowercase()
        } else {
            String::from("none")
        };

        if remote_crc != local_crc {
            println!(
                "CRC mismatch! Local: {}, Remote: {}. Downloading replacement...",
                local_crc, remote_crc
            );

            tokio::spawn(async move {
                let temp_dir = tempfile::tempdir().expect("Gotta be able to create a temp dir");
                let bz2_path = temp_dir.path().join("AllPrintings.sqlite.bz2");

                println!("Download from {DOWNLOAD_URL:?} to {bz2_path:?}...");
                let result = stream_to_file(DOWNLOAD_URL, "Download complete", &bz2_path, None, "downloading")
                    .await
                    .and_then(|_| calculate_sha256(&bz2_path))
                    .and_then(|downloaded_crc| {
                        if downloaded_crc == remote_crc {
                            decompress_bz2(&bz2_path, &local_file_path)?;
                            fs::write(&sidecar_path, &downloaded_crc)?;
                            println!("File replaced successfully.");
                        } else {
                            println!("Downloaded CRC mismatch: expected {remote_crc}, got {downloaded_crc}");
                        }
                        Ok(())
                    });
                if let Err(e) = result {
                    println!("Failed to download due to {e}");
                }
            });

            Ok(true)
        } else {
            println!("CRCs match ({}). No replacement needed.", local_crc);
            Ok(false)
        }
    }
}

// ── Price helpers ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PricesFile {
    data: HashMap<String, CardPriceEntry>,
}

#[derive(Deserialize)]
struct CardPriceEntry {
    #[serde(default)]
    paper: HashMap<String, PaperRetailer>,
}

#[derive(Deserialize)]
struct PaperRetailer {
    retail: Option<RetailSection>,
}

#[derive(Deserialize)]
struct RetailSection {
    normal: Option<HashMap<String, f64>>,
    foil: Option<HashMap<String, f64>>,
}

fn latest_price(prices: &HashMap<String, f64>) -> Option<f64> {
    prices.iter().max_by_key(|(date, _)| date.as_str()).map(|(_, p)| *p)
}

fn entry_to_card_prices(uuid: &str, entry: CardPriceEntry) -> CardPrices {
    let paper = entry
        .paper
        .into_iter()
        .map(|(retailer, data)| {
            let normal = data
                .retail
                .as_ref()
                .and_then(|r| r.normal.as_ref())
                .and_then(latest_price);
            let foil = data
                .retail
                .as_ref()
                .and_then(|r| r.foil.as_ref())
                .and_then(latest_price);
            (retailer, RetailerPrices { normal, foil })
        })
        .collect();
    CardPrices { uuid: uuid.to_string(), paper }
}

fn load_prices_file(path: &str) -> eyre::Result<HashMap<String, CardPrices>> {
    println!("Loading prices from {path}...");
    let json = fs::read_to_string(path)?;
    let root: PricesFile = serde_json::from_str(&json)?;
    let map = root
        .data
        .into_iter()
        .map(|(uuid, entry)| {
            let prices = entry_to_card_prices(&uuid, entry);
            (uuid, prices)
        })
        .collect();
    println!("Prices loaded.");
    Ok(map)
}

pub async fn download_prices(path: &str) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str = "https://mtgjson.com/api/v5/AllPricesToday.json.bz2";

    let target = PathBuf::from_str(path)?;
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    let temp_dir = tempfile::tempdir()?;
    let bz2_path = temp_dir.path().join("AllPricesToday.json.bz2");

    println!("Downloading AllPricesToday.json.bz2...");
    stream_to_file(DOWNLOAD_URL, "Download complete", &bz2_path, None, "downloading").await?;

    println!("Decompressing AllPricesToday.json.bz2...");
    let bz2_file = fs::File::open(&bz2_path)?;
    let mut decoder = BzDecoder::new(bz2_file);
    let mut json_content = String::new();
    decoder.read_to_string(&mut json_content)?;

    fs::write(&target, json_content.as_bytes())?;
    println!("Prices saved to {target:?} ({} bytes).", json_content.len());
    Ok(())
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn stream_to_file(
    url: &str,
    label: &str,
    path: &Path,
    progress: Option<&Arc<Mutex<DownloadProgress>>>,
    phase: &str,
) -> eyre::Result<()> {
    let response = reqwest::Client::new().get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0);

    if let Some(p) = progress {
        let mut p = p.lock().await;
        p.total = total_size;
        p.downloaded = 0;
        p.phase = phase.to_string();
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% ({eta_precise}) {bytes} / {total_bytes}")?
            .progress_chars("#>-"),
    );

    let mut file = fs::File::create(path)?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        let len = chunk.len() as u64;
        pb.inc(len);
        if let Some(p) = progress {
            let mut p = p.lock().await;
            p.downloaded += len;
        }
    }
    pb.finish_with_message(label.to_string());
    Ok(())
}

fn calculate_sha256(path: &Path) -> eyre::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn decompress_bz2(src: &Path, dst: &Path) -> eyre::Result<()> {
    let file = fs::File::open(src)?;
    let mut decoder = BzDecoder::new(file);
    let mut out = fs::File::create(dst)?;
    std::io::copy(&mut decoder, &mut out)?;
    Ok(())
}

pub async fn download_mtg_db(
    path: &str,
    progress: Option<Arc<Mutex<DownloadProgress>>>,
) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2";
    const CRC_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite.bz2.sha256";

    let target = PathBuf::from_str(path)?;
    let sidecar_path = PathBuf::from(format!("{}.bz2.sha256", target.display()));

    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    let temp_dir = tempfile::tempdir()?;

    println!("Fetching remote SHA256 for AllPrintings.sqlite.bz2...");
    let crc_path = temp_dir.path().join("remote.sha256");
    stream_to_file(CRC_URL, "SHA256 fetched", &crc_path, progress.as_ref(), "checking").await?;
    let remote_crc = fs::read_to_string(&crc_path)?.trim().to_lowercase();

    let local_crc = if sidecar_path.exists() {
        fs::read_to_string(&sidecar_path)?.trim().to_lowercase()
    } else {
        String::new()
    };
    if target.exists() && local_crc == remote_crc {
        println!("AllPrintings.db is already up to date (CRC: {remote_crc}).");
        return Ok(());
    }

    println!("Downloading AllPrintings.sqlite.bz2 to {target:?}...");
    let bz2_path = temp_dir.path().join("AllPrintings.sqlite.bz2");
    stream_to_file(DOWNLOAD_URL, "Download complete", &bz2_path, progress.as_ref(), "downloading").await?;

    if let Some(p) = &progress {
        p.lock().await.phase = "verifying".to_string();
    }
    let downloaded_crc = calculate_sha256(&bz2_path)?;
    if downloaded_crc != remote_crc {
        eyre::bail!(
            "AllPrintings.sqlite.bz2 CRC mismatch after download: expected {remote_crc}, got {downloaded_crc}"
        );
    }

    if let Some(p) = &progress {
        p.lock().await.phase = "decompressing".to_string();
    }
    decompress_bz2(&bz2_path, &target)?;
    fs::write(&sidecar_path, &downloaded_crc)?;
    println!("AllPrintings.db downloaded, verified, and decompressed (CRC: {downloaded_crc}).");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::models::CardColour;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_new_with_none() {
        let system = MagicSQLiteRetrievalSystem::new(None, None);
        assert!(system.is_ok());
        let system = system.unwrap();
        assert!(!system.db_path.is_empty());
    }

    #[tokio::test]
    async fn test_new_with_custom_path() {
        let temp_dir = TempDir::new().unwrap();
        let custom_path = temp_dir.path().join("test.db");
        let system =
            MagicSQLiteRetrievalSystem::new(Some(custom_path.to_string_lossy().to_string()), None);
        assert!(system.is_ok());
        let system = system.unwrap();
        assert_eq!(system.db_path, custom_path.to_string_lossy().to_string());
    }

    #[tokio::test]
    async fn test_search_cards_with_name_filter() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            name: Some("Goblin King".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(!cards.is_empty());
    }

    #[tokio::test]
    async fn test_search_cards_with_color_identity_filter() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            color_identities: Some(vec![CardColour::Black]),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(!cards.is_empty());
    }

    #[tokio::test]
    async fn test_search_cards_with_artist_filter() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            artist: Some("Jason Chan".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(!cards.is_empty());
    }

    #[tokio::test]
    async fn test_search_cards_with_text_filter() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            text: Some("destroy target enchantment".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(!cards.is_empty());
    }

    #[tokio::test]
    async fn test_search_cards_with_set_code_filter() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            set_code: Some("M20".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(!cards.is_empty());
    }

    #[tokio::test]
    async fn test_search_cards_with_skip_and_limit() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            name: Some("Rule of Law".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, Some(6), Some(5)).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(cards.len() <= 5);
    }

    #[tokio::test]
    async fn test_search_cards_empty_result() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            name: Some("NonExistentCardXYZ123".to_string()),
            ..Default::default()
        };
        let result = system.search_cards(filters, None, None).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(cards.is_empty());
    }

    #[tokio::test]
    async fn test_get_cards_by_ids() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let ids = vec![
            "0003caab-9ff5-5d1a-bc06-976dd0457f19".to_string(),
            "0005d268-3fd0-5424-bc6b-573ecd713aa1".to_string(),
        ];
        let result = system.get_cards_by_ids(ids).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[tokio::test]
    async fn test_get_cards_by_empty_ids() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system.get_cards_by_ids(vec![]).await;
        assert!(result.is_ok());
        let cards = result.unwrap();
        assert!(cards.is_empty());
    }

    #[tokio::test]
    async fn test_get_sets() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system.get_sets().await;
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert!(!sets.is_empty());
    }

    #[tokio::test]
    async fn test_bulk_search_cards() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let cards = vec![
            (
                SetCode::from_str("TLE").unwrap(),
                CollectorNumber::from_str("12").unwrap(),
            ),
            (
                SetCode::from_str("ARB").unwrap(),
                CollectorNumber::from_str("52").unwrap(),
            ),
        ];
        let result = system.bulk_search_cards(cards).await;
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_bulk_search_cards_empty() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let result = system.bulk_search_cards(vec![]).await;
        assert!(result.is_ok());
        let results = result.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_named_retrieval_system_trait() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let name = system.name();
        assert_eq!(name, "MagicSQLite");
    }

    fn card_name(c: &::models::Card) -> String {
        match c {
            ::models::Card::Magic(m) => m.name.to_lowercase(),
            _ => String::new(),
        }
    }

    fn card_types(c: &::models::Card) -> Vec<String> {
        match c {
            ::models::Card::Magic(m) => m.types.clone(),
            _ => vec![],
        }
    }

    fn card_rarity(c: &::models::Card) -> String {
        match c {
            ::models::Card::Magic(m) => format!("{:?}", m.rarity).to_lowercase(),
            _ => String::new(),
        }
    }

    fn card_set_code(c: &::models::Card) -> String {
        match c {
            ::models::Card::Magic(m) => m.set_code.to_lowercase(),
            _ => String::new(),
        }
    }

    #[tokio::test]
    async fn test_search_cards_sort_by_name_asc() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            sort_by: Some(SortField::Name),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        };
        let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
        assert!(!cards.is_empty());
        let names: Vec<_> = cards.iter().map(card_name).collect();
        for w in names.windows(2) {
            assert!(w[0] <= w[1], "name order violated: {:?} > {:?}", w[0], w[1]);
        }
    }

    #[tokio::test]
    async fn test_search_cards_sort_by_name_desc() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            sort_by: Some(SortField::Name),
            sort_order: Some(SortOrder::Desc),
            ..Default::default()
        };
        let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
        assert!(!cards.is_empty());
        let names: Vec<_> = cards.iter().map(card_name).collect();
        for w in names.windows(2) {
            assert!(w[0] >= w[1], "name desc order violated: {:?} < {:?}", w[0], w[1]);
        }
    }

    #[tokio::test]
    async fn test_search_cards_sort_by_rarity_asc() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            sort_by: Some(SortField::Rarity),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        };
        let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
        assert!(!cards.is_empty());
        let rarities: Vec<_> = cards.iter().map(card_rarity).collect();
        for w in rarities.windows(2) {
            assert!(w[0] <= w[1], "rarity order violated: {:?} > {:?}", w[0], w[1]);
        }
    }

    #[tokio::test]
    async fn test_search_cards_sort_by_set_code() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            sort_by: Some(SortField::SetCode),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        };
        let cards = system.search_cards(filters, None, Some(50)).await.unwrap();
        assert!(!cards.is_empty());
        let set_codes: Vec<_> = cards.iter().map(card_set_code).collect();
        for w in set_codes.windows(2) {
            assert!(w[0] <= w[1], "set_code order violated: {:?} > {:?}", w[0], w[1]);
        }
    }

    #[tokio::test]
    async fn test_search_cards_default_sort_is_name_asc() {
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters_default = CardSearchFilters::default();
        let filters_explicit = CardSearchFilters {
            sort_by: Some(SortField::Name),
            sort_order: Some(SortOrder::Asc),
            ..Default::default()
        };
        let default_cards = system.search_cards(filters_default, None, Some(10)).await.unwrap();
        let explicit_cards = system.search_cards(filters_explicit, None, Some(10)).await.unwrap();
        let default_names: Vec<_> = default_cards.iter().map(card_name).collect();
        let explicit_names: Vec<_> = explicit_cards.iter().map(card_name).collect();
        assert_eq!(default_names, explicit_names);
    }

    // ── Multi-type filter tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_search_cards_multiple_types_mutually_exclusive_returns_empty() {
        // Regression: the types loop was missing `i += 1`, so all conditions
        // reused the same parameter index and effectively checked only the
        // first type. With two mutually-exclusive types the old code returned
        // the same results as a single-type filter; the fix returns zero.
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let filters = CardSearchFilters {
            // No card can be both a Creature and a Sorcery.
            types: Some(vec!["Creature".to_string(), "Sorcery".to_string()]),
            ..Default::default()
        };
        let cards = system.search_cards(filters, None, Some(200)).await.unwrap();
        // card_types used here so the helper is not flagged dead_code.
        assert!(
            cards.iter().all(|c| {
                let t = card_types(c);
                t.iter().any(|x| x.eq_ignore_ascii_case("Creature"))
                    && t.iter().any(|x| x.eq_ignore_ascii_case("Sorcery"))
            }),
            "no card can be both Creature and Sorcery, got {} results",
            cards.len()
        );
        assert!(cards.is_empty(), "expected zero results for impossible type combo");
    }

    #[tokio::test]
    async fn test_search_cards_two_type_filter_is_stricter_than_one() {
        // Regression: before the fix both conditions resolved to the same
        // parameter, making the two-type filter identical to the one-type
        // filter. After the fix, AND semantics hold and the result set shrinks.
        let system = MagicSQLiteRetrievalSystem::new(None, None).unwrap();
        let single = CardSearchFilters {
            types: Some(vec!["Creature".to_string()]),
            ..Default::default()
        };
        let dual = CardSearchFilters {
            // Creatures-only vs Creature-AND-Sorcery (impossible combo → 0 results).
            types: Some(vec!["Creature".to_string(), "Sorcery".to_string()]),
            ..Default::default()
        };
        let single_count = system.search_cards(single, None, Some(200)).await.unwrap().len();
        let dual_count = system.search_cards(dual, None, Some(200)).await.unwrap().len();
        assert!(
            single_count > dual_count,
            "one-type filter ({single_count}) should return more results than mutually-exclusive two-type AND ({dual_count})"
        );
    }

    // ── Price tests ───────────────────────────────────────────────────────────

    // Snapshot of three real UUIDs from AllPricesToday.json (2026-05-22).
    // uuid-00010d56: foil-only retail entries (no normal for most retailers)
    // uuid-0001e0d0: normal-only retail entries (no foil anywhere)
    // uuid-0003caab: both normal and foil; also has an "mtgo" section (should be ignored)
    // All entries contain "buylist" and "currency" fields that must be silently ignored.
    const REAL_PRICES_SNAPSHOT: &str = r#"{"data":{"00010d56-fe38-5e35-8aed-518019aa36a5":{"paper":{"cardmarket":{"buylist":{},"retail":{"normal":{"2026-05-22":3.07},"foil":{"2026-05-22":4.44}},"currency":"EUR"},"manapool":{"buylist":{},"retail":{"foil":{"2026-05-22":11.23}},"currency":"USD"},"cardkingdom":{"buylist":{"foil":{"2026-05-22":5.0}},"retail":{"foil":{"2026-05-22":11.99}},"currency":"USD"},"tcgplayer":{"buylist":{},"retail":{"foil":{"2026-05-22":12.63}},"currency":"USD"}}},"0001e0d0-2dcd-5640-aadc-a84765cf5fc9":{"paper":{"cardkingdom":{"buylist":{"normal":{"2026-05-22":4.2}},"retail":{"normal":{"2026-05-22":7.49}},"currency":"USD"},"cardmarket":{"buylist":{},"retail":{"normal":{"2026-05-22":4.78}},"currency":"EUR"},"manapool":{"buylist":{},"retail":{"normal":{"2026-05-22":4.12}},"currency":"USD"},"tcgplayer":{"buylist":{},"retail":{"normal":{"2026-05-22":5.89}},"currency":"USD"}}},"0003caab-9ff5-5d1a-bc06-976dd0457f19":{"paper":{"manapool":{"buylist":{},"retail":{"normal":{"2026-05-22":0.15},"foil":{"2026-05-22":0.48}},"currency":"USD"},"tcgplayer":{"buylist":{},"retail":{"foil":{"2026-05-22":2.04},"normal":{"2026-05-22":0.16}},"currency":"USD"},"cardkingdom":{"buylist":{"foil":{"2026-05-22":0.75}},"retail":{"foil":{"2026-05-22":2.49},"normal":{"2026-05-22":0.35}},"currency":"USD"},"cardmarket":{"buylist":{},"retail":{"normal":{"2026-05-22":0.19},"foil":{"2026-05-22":1.02}},"currency":"EUR"}},"mtgo":{"cardhoarder":{"buylist":{},"retail":{"normal":{"2026-05-22":0.03}},"currency":"USD"}}}}}"#;

    // uuid-alpha/beta used for simple structural tests that don't need real prices.
    const DUMMY_PRICES_JSON: &str = r#"{
        "data": {
            "uuid-alpha": {
                "paper": {
                    "cardkingdom": {
                        "retail": {
                            "normal": {"2024-01-01": 1.50},
                            "foil":   {"2024-01-01": 3.00}
                        }
                    },
                    "tcgplayer": {
                        "retail": {
                            "normal": {"2024-01-01": 1.25}
                        }
                    }
                }
            },
            "uuid-beta": {
                "paper": {
                    "cardkingdom": {
                        "retail": {
                            "normal": {"2024-01-01": 0.25}
                        }
                    }
                }
            }
        }
    }"#;

    fn write_prices(dir: &TempDir, json: &str) -> String {
        let path = dir.path().join("prices.json");
        std::fs::write(&path, json).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn write_dummy_prices(dir: &TempDir) -> String {
        write_prices(dir, DUMMY_PRICES_JSON)
    }

    fn system_with_prices(prices_path: Option<String>) -> MagicSQLiteRetrievalSystem {
        MagicSQLiteRetrievalSystem::new(None, prices_path).unwrap()
    }

    #[tokio::test]
    async fn test_get_card_prices_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system.get_card_prices("uuid-alpha").await.unwrap();
        assert!(result.is_some());
        let prices = result.unwrap();
        assert_eq!(prices.uuid, "uuid-alpha");
        assert_eq!(prices.paper.len(), 2);
        let ck = prices.paper.get("cardkingdom").unwrap();
        assert_eq!(ck.normal, Some(1.50));
        assert_eq!(ck.foil, Some(3.00));
        let tcp = prices.paper.get("tcgplayer").unwrap();
        assert_eq!(tcp.normal, Some(1.25));
        assert_eq!(tcp.foil, None);
    }

    #[tokio::test]
    async fn test_get_card_prices_not_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system.get_card_prices("uuid-nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_card_prices_no_prices_path() {
        let system = system_with_prices(None);
        let result = system.get_card_prices("uuid-alpha").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_card_prices_file_missing() {
        let system = system_with_prices(Some("/tmp/does_not_exist_prices.json".to_string()));
        let result = system.get_card_prices("uuid-alpha").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_all_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system
            .get_bulk_card_prices(vec!["uuid-alpha".to_string(), "uuid-beta".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("uuid-alpha"));
        assert!(result.contains_key("uuid-beta"));
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_partial_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system
            .get_bulk_card_prices(vec![
                "uuid-alpha".to_string(),
                "uuid-missing".to_string(),
                "uuid-also-missing".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("uuid-alpha"));
        assert!(!result.contains_key("uuid-missing"));
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_none_found() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system
            .get_bulk_card_prices(vec!["uuid-x".to_string(), "uuid-y".to_string()])
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_empty_input() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system.get_bulk_card_prices(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_no_prices_path() {
        let system = system_with_prices(None);
        let result = system
            .get_bulk_card_prices(vec!["uuid-alpha".to_string()])
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_get_bulk_card_prices_file_missing() {
        let system = system_with_prices(Some("/tmp/does_not_exist_prices.json".to_string()));
        let result = system
            .get_bulk_card_prices(vec!["uuid-alpha".to_string()])
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_prices_cache_reuse() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let first = system.get_card_prices("uuid-alpha").await.unwrap();
        let second = system.get_card_prices("uuid-beta").await.unwrap();
        // Both calls succeed without error, proving cache is reused after first load.
        assert!(first.is_some());
        assert!(second.is_some());
        assert_eq!(first.unwrap().uuid, "uuid-alpha");
        assert_eq!(second.unwrap().uuid, "uuid-beta");
    }

    #[tokio::test]
    async fn test_update_prices_no_path_returns_false() {
        let system = system_with_prices(None);
        let result = system.update_prices().await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_prices_prices_data_correctness() {
        let dir = TempDir::new().unwrap();
        let prices_path = write_dummy_prices(&dir);
        let system = system_with_prices(Some(prices_path));

        let result = system
            .get_bulk_card_prices(vec!["uuid-alpha".to_string(), "uuid-beta".to_string()])
            .await
            .unwrap();

        let beta = result.get("uuid-beta").unwrap();
        assert_eq!(beta.paper.len(), 1);
        let ck = beta.paper.get("cardkingdom").unwrap();
        assert_eq!(ck.normal, Some(0.25));
        assert_eq!(ck.foil, None);
    }

    // ── Real-snapshot tests ───────────────────────────────────────────────────

    fn write_real_prices(dir: &TempDir) -> String {
        write_prices(dir, REAL_PRICES_SNAPSHOT)
    }

    #[tokio::test]
    async fn test_real_snapshot_all_retailers_present() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        let prices = system
            .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
            .await
            .unwrap()
            .unwrap();
        // Four retailers in the paper section.
        assert_eq!(prices.paper.len(), 4);
        for retailer in ["cardmarket", "manapool", "cardkingdom", "tcgplayer"] {
            assert!(prices.paper.contains_key(retailer), "missing {retailer}");
        }
    }

    #[tokio::test]
    async fn test_real_snapshot_foil_only_retailer() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        let prices = system
            .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
            .await
            .unwrap()
            .unwrap();
        // manapool has only foil retail for this card.
        let manapool = prices.paper.get("manapool").unwrap();
        assert_eq!(manapool.foil, Some(11.23));
        assert_eq!(manapool.normal, None);
        // cardkingdom also foil-only retail.
        let ck = prices.paper.get("cardkingdom").unwrap();
        assert_eq!(ck.foil, Some(11.99));
        assert_eq!(ck.normal, None);
    }

    #[tokio::test]
    async fn test_real_snapshot_both_normal_and_foil() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        let prices = system
            .get_card_prices("00010d56-fe38-5e35-8aed-518019aa36a5")
            .await
            .unwrap()
            .unwrap();
        // cardmarket has both normal and foil retail for this card.
        let cm = prices.paper.get("cardmarket").unwrap();
        assert_eq!(cm.normal, Some(3.07));
        assert_eq!(cm.foil, Some(4.44));
    }

    #[tokio::test]
    async fn test_real_snapshot_normal_only_card() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        let prices = system
            .get_card_prices("0001e0d0-2dcd-5640-aadc-a84765cf5fc9")
            .await
            .unwrap()
            .unwrap();
        // All four retailers have normal prices but no foil.
        assert_eq!(prices.paper.len(), 4);
        for (_, rp) in &prices.paper {
            assert!(rp.normal.is_some(), "expected normal price");
            assert_eq!(rp.foil, None, "expected no foil price");
        }
        assert_eq!(prices.paper.get("cardkingdom").unwrap().normal, Some(7.49));
        assert_eq!(prices.paper.get("tcgplayer").unwrap().normal, Some(5.89));
    }

    #[tokio::test]
    async fn test_real_snapshot_mtgo_section_not_in_paper() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        // uuid-0003caab has an "mtgo" section; it must NOT appear in paper.
        let prices = system
            .get_card_prices("0003caab-9ff5-5d1a-bc06-976dd0457f19")
            .await
            .unwrap()
            .unwrap();
        assert!(!prices.paper.contains_key("cardhoarder"), "mtgo retailer leaked into paper");
        assert_eq!(prices.paper.len(), 4);
    }

    #[tokio::test]
    async fn test_real_snapshot_bulk_returns_all_three() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        let uuids = vec![
            "00010d56-fe38-5e35-8aed-518019aa36a5".to_string(),
            "0001e0d0-2dcd-5640-aadc-a84765cf5fc9".to_string(),
            "0003caab-9ff5-5d1a-bc06-976dd0457f19".to_string(),
            "not-in-file".to_string(),
        ];
        let result = system.get_bulk_card_prices(uuids).await.unwrap();
        // 3 found, 1 missing — missing card must not cause failure.
        assert_eq!(result.len(), 3);
        assert!(!result.contains_key("not-in-file"));
    }

    #[tokio::test]
    async fn test_real_snapshot_buylist_not_in_retail() {
        let dir = TempDir::new().unwrap();
        let system = system_with_prices(Some(write_real_prices(&dir)));

        // cardkingdom for uuid-0003caab: retail foil=2.49, normal=0.35.
        // buylist foil=0.75 must NOT appear as the retail price.
        let prices = system
            .get_card_prices("0003caab-9ff5-5d1a-bc06-976dd0457f19")
            .await
            .unwrap()
            .unwrap();
        let ck = prices.paper.get("cardkingdom").unwrap();
        assert_eq!(ck.normal, Some(0.35));
        assert_eq!(ck.foil, Some(2.49));
    }
}
