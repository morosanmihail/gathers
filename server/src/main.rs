use aide::axum::{ApiRouter, routing::get};
use aide::openapi::{Info, OpenApi};
use aide::swagger::Swagger;
use axum::http::StatusCode;
use axum::{Extension, Json, error_handling::HandleErrorLayer, extract::State};
use clap::{Parser, ValueEnum};
use persistence::PersistenceSystem;
use retrieval::{DownloadProgress, NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait};
use schemars::JsonSchema;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tower::{BoxError, ServiceBuilder};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::debug;

use crate::collections::collection_routes;
use crate::mtg_api::mtg_routes;
use crate::pokemon_api::pokemon_routes;
use crate::riftbound_api::riftbound_routes;

mod collections;
mod mtg_api;
mod pokemon_api;
mod riftbound_api;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ErrorPayload {
    pub error: String,
}

/// Convenience alias for the standard API error response.
pub type ApiError = (StatusCode, Json<ErrorPayload>);

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct DownloadProgressInfo {
    pub downloaded: u64,
    pub total: u64,
    pub phase: String,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct SystemInfo {
    /// Primary active system, identified by NamedRetrievalSystem::name().
    pub system: String,
    /// All active systems, identified by NamedRetrievalSystem::name().
    /// These strings also match the `provider` field stored on collection cards.
    pub systems: Vec<String>,
    /// Systems whose databases are currently being downloaded, with progress info.
    pub downloading: HashMap<String, DownloadProgressInfo>,
}

type GathersState = (Arc<Mutex<RetrievalState>>, Arc<Mutex<StorageState>>);

#[derive(Debug, Clone)]
pub struct RetrievalState {
    pub mtg: Option<RetrievalSystem>,
    pub riftbound: Option<RetrievalSystem>,
    pub pokemon: Option<RetrievalSystem>,
    /// Which MTG system variant is active (Scryfall or Sql), for reload support.
    mtg_system_type: Option<Systems>,
    mtg_db_path: Option<String>,
    riftbound_db_path: Option<String>,
    pokemon_db_path: Option<String>,
    /// Progress trackers for in-progress downloads, keyed by system name.
    pub downloading: HashMap<String, Arc<Mutex<DownloadProgress>>>,
}

#[derive(Debug, Clone)]
pub struct StorageState {
    storage: PersistenceSystem,
    _storage_db_path: Option<String>,
}

impl RetrievalState {
    pub fn new(
        systems: Vec<Systems>,
        mtg_db_path: Option<String>,
        riftbound_db_path: Option<String>,
        pokemon_db_path: Option<String>,
    ) -> eyre::Result<RetrievalState> {
        let mut state = RetrievalState {
            mtg: None,
            riftbound: None,
            pokemon: None,
            mtg_system_type: None,
            mtg_db_path: mtg_db_path.clone(),
            riftbound_db_path: riftbound_db_path.clone(),
            pokemon_db_path: pokemon_db_path.clone(),
            downloading: HashMap::new(),
        };

        for system in systems {
            let db_path = match system {
                Systems::Scryfall | Systems::Sql => mtg_db_path.clone(),
                Systems::RiftboundSql => riftbound_db_path.clone(),
                Systems::PokemonSql => pokemon_db_path.clone(),
            };
            // Skip file-based systems whose DB doesn't exist yet (downloading in background).
            let needs_file = matches!(system, Systems::Sql | Systems::RiftboundSql | Systems::PokemonSql);
            if needs_file
                && let Some(ref path) = db_path
                    && !std::path::Path::new(path).exists() {
                        continue;
                    }
            let retrieval = Self::new_retrieval(system, db_path)?;
            match system {
                Systems::Scryfall | Systems::Sql => {
                    state.mtg = Some(retrieval);
                    state.mtg_system_type = Some(system);
                }
                Systems::RiftboundSql => state.riftbound = Some(retrieval),
                Systems::PokemonSql => state.pokemon = Some(retrieval),
            }
        }

        Ok(state)
    }

    pub fn new_retrieval(
        system: Systems,
        retrieval_db_path: Option<String>,
    ) -> eyre::Result<RetrievalSystem> {
        Ok(match system {
            Systems::Scryfall => {
                RetrievalSystem::ScryfallRetrievalSystem(retrieval::ScryfallRetrievalSystem::new()?)
            }
            Systems::Sql => RetrievalSystem::MagicSQLiteRetrievalSystem(
                retrieval::MagicSQLiteRetrievalSystem::new(retrieval_db_path.clone())?,
            ),
            Systems::RiftboundSql => RetrievalSystem::RiftboundSQLiteRetrievalSystem(
                retrieval::RiftboundSQLiteRetrievalSystem::new(retrieval_db_path.clone())?,
            ),
            Systems::PokemonSql => RetrievalSystem::PokemonSQLiteRetrievalSystem(
                retrieval::PokemonSQLiteRetrievalSystem::new(retrieval_db_path.clone())?,
            ),
        })
    }

    pub fn active_systems(&self) -> Vec<Systems> {
        let mut systems = Vec::new();
        if let Some(s) = self.mtg_system_type {
            systems.push(s);
        }
        if self.riftbound.is_some() {
            systems.push(Systems::RiftboundSql);
        }
        if self.pokemon.is_some() {
            systems.push(Systems::PokemonSql);
        }
        systems
    }

    /// Returns the primary system for webui compatibility.
    /// Prefers MTG, then Riftbound, then Pokemon.
    pub fn primary_system(&self) -> Systems {
        if let Some(s) = self.mtg_system_type {
            s
        } else if self.riftbound.is_some() {
            Systems::RiftboundSql
        } else {
            Systems::PokemonSql
        }
    }

    pub async fn get_system_info(&self) -> SystemInfo {
        let systems: Vec<String> = [
            self.mtg.as_ref(),
            self.riftbound.as_ref(),
            self.pokemon.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|s| s.name().to_string())
        .collect();
        let system = systems.first().cloned().unwrap_or_default();
        let mut downloading = HashMap::new();
        for (key, progress) in &self.downloading {
            let p = progress.lock().await;
            downloading.insert(key.clone(), DownloadProgressInfo {
                downloaded: p.downloaded,
                total: p.total,
                phase: p.phase.clone(),
            });
        }
        SystemInfo { system, systems, downloading }
    }

    pub fn require_mtg(&self) -> Result<&RetrievalSystem, ApiError> {
        self.mtg.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: "MTG system not configured".into(),
                }),
            )
        })
    }

    pub fn require_riftbound(&self) -> Result<&RetrievalSystem, ApiError> {
        self.riftbound.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: "Riftbound system not configured".into(),
                }),
            )
        })
    }

    pub fn require_pokemon(&self) -> Result<&RetrievalSystem, ApiError> {
        self.pokemon.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: "Pokemon system not configured".into(),
                }),
            )
        })
    }

    pub fn reload_mtg(&mut self) -> eyre::Result<()> {
        if let Some(system) = self.mtg_system_type {
            self.mtg = Some(Self::new_retrieval(system, self.mtg_db_path.clone())?);
        }
        Ok(())
    }

    pub fn add_system(&mut self, system: Systems) -> eyre::Result<()> {
        let db_path = match system {
            Systems::Scryfall | Systems::Sql => self.mtg_db_path.clone(),
            Systems::RiftboundSql => self.riftbound_db_path.clone(),
            Systems::PokemonSql => self.pokemon_db_path.clone(),
        };
        let retrieval = Self::new_retrieval(system, db_path)?;
        match system {
            Systems::Scryfall | Systems::Sql => {
                self.mtg = Some(retrieval);
                self.mtg_system_type = Some(system);
            }
            Systems::RiftboundSql => self.riftbound = Some(retrieval),
            Systems::PokemonSql => self.pokemon = Some(retrieval),
        }
        self.downloading.remove(&format!("{system:?}"));
        Ok(())
    }

    pub fn reload_riftbound(&mut self) -> eyre::Result<()> {
        if self.riftbound.is_some() {
            self.riftbound = Some(Self::new_retrieval(
                Systems::RiftboundSql,
                self.riftbound_db_path.clone(),
            )?);
        }
        Ok(())
    }

    pub fn reload_pokemon(&mut self) -> eyre::Result<()> {
        if self.pokemon.is_some() {
            self.pokemon = Some(Self::new_retrieval(
                Systems::PokemonSql,
                self.pokemon_db_path.clone(),
            )?);
        }
        Ok(())
    }
}

impl StorageState {
    pub fn new(storage_db_path: Option<String>) -> eyre::Result<StorageState> {
        Ok(StorageState {
            storage: PersistenceSystem::SQLitePersistenceSystem(
                persistence::SQLitePersistenceSystem::new(false, storage_db_path.clone())?,
            ),
            _storage_db_path: storage_db_path,
        })
    }
}

#[derive(
    Copy,
    Clone,
    ValueEnum,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    JsonSchema,
)]
pub enum Systems {
    Scryfall,
    Sql,
    RiftboundSql,
    PokemonSql,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ServerConfig {
    system: Vec<Systems>,
    port: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtg_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    riftbound_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pokemon_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_db_path: Option<String>,
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Retrieval systems to enable. May be specified multiple times.
    /// Required when no config file exists. Supported values: scryfall, sql, riftbound-sql, pokemon-sql.
    #[clap(short, long, num_args = 1..)]
    system: Vec<Systems>,

    /// Port to listen on. Required when no config file exists.
    #[clap(short, long)]
    port: Option<usize>,
}

async fn get_system_info(
    State(state): State<GathersState>,
) -> Result<Json<SystemInfo>, (axum::http::StatusCode, Json<String>)> {
    let ret = state.0.lock().await;
    Ok(Json(ret.get_system_info().await))
}

async fn serve_api(Extension(api): Extension<OpenApi>) -> impl axum::response::IntoResponse {
    Json(api)
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let gathers_dir = std::path::Path::new(&home).join(".local/share/gathers");
    let db_dir = gathers_dir.join("DB");
    let config_path = gathers_dir.join("server.toml");

    // Load or create config file
    let mut config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        toml::from_str::<ServerConfig>(&content)
            .map_err(|e| eyre::eyre!("Failed to parse {}: {e}", config_path.display()))?
    } else {
        let systems = if args.system.is_empty() {
            vec![Systems::RiftboundSql]
        } else {
            args.system.clone()
        };
        let port = args.port.unwrap_or(5234);
        let cfg = ServerConfig {
            system: systems,
            port,
            mtg_db_path: Some(
                db_dir
                    .join("AllPrintings.db")
                    .to_string_lossy()
                    .into_owned(),
            ),
            riftbound_db_path: Some(db_dir.join("riftbound.db").to_string_lossy().into_owned()),
            pokemon_db_path: Some(db_dir.join("pokemon.db").to_string_lossy().into_owned()),
            storage_db_path: Some(db_dir.join("storage.db").to_string_lossy().into_owned()),
        };
        if let Err(e) = std::fs::create_dir_all(&gathers_dir) {
            eprintln!(
                "error: cannot create config directory {}: {e}\n  check permissions on {}",
                gathers_dir.display(),
                gathers_dir.parent().map(|p| p.display().to_string()).unwrap_or_default()
            );
            std::process::exit(1);
        }
        if let Err(e) = std::fs::write(&config_path, toml::to_string_pretty(&cfg)?) {
            eprintln!(
                "error: cannot write config file {}: {e}\n  check permissions on {}",
                config_path.display(),
                gathers_dir.display()
            );
            std::process::exit(1);
        }
        println!("Created config file at {}", config_path.display());
        cfg
    };

    if let Err(e) = std::fs::create_dir_all(&db_dir) {
        eprintln!(
            "error: cannot create database directory {}: {e}\n  check permissions on {}",
            db_dir.display(),
            gathers_dir.display()
        );
        std::process::exit(1);
    }

    // CLI args override config for this session
    if !args.system.is_empty() {
        config.system = args.system;
    }
    if let Some(port) = args.port {
        config.port = port;
    }

    // GATHERS_SYSTEMS env var overrides config (comma-separated, e.g. "scryfall,riftbound-sql")
    if let Ok(val) = std::env::var("GATHERS_SYSTEMS") {
        let parsed: Vec<Systems> = val
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| {
                Systems::from_str(s, true).map_err(|e| {
                    eprintln!("warning: unknown system in GATHERS_SYSTEMS '{s}': {e}");
                }).ok()
            })
            .collect();
        if !parsed.is_empty() {
            config.system = parsed;
        }
    }

    // Env vars override config for DB paths
    let mtg_db_path = std::env::var("MTG_DB_PATH").ok().or(config.mtg_db_path);
    let riftbound_db_path = std::env::var("RIFTBOUND_DB_PATH")
        .ok()
        .or(config.riftbound_db_path);
    let pokemon_db_path = std::env::var("POKEMON_DB_PATH")
        .ok()
        .or(config.pokemon_db_path);
    let storage_db_path = std::env::var("STORAGE_DB_PATH")
        .ok()
        .or(config.storage_db_path);

    let port = config.port;

    let retrieval = Arc::new(Mutex::new(RetrievalState::new(
        config.system.clone(),
        mtg_db_path.clone(),
        riftbound_db_path.clone(),
        pokemon_db_path.clone(),
    )?));

    if std::env::var("GATHERS_NO_AUTO_UPDATE").is_err() {
        for system in &config.system {
            match system {
                Systems::Sql => {
                    if let Some(ref path) = mtg_db_path
                        && !std::path::Path::new(path).exists()
                    {
                        let path = path.clone();
                        let retrieval = retrieval.clone();
                        let progress = Arc::new(Mutex::new(DownloadProgress::default()));
                        retrieval.lock().await.downloading.insert("Sql".to_string(), progress.clone());
                        tokio::spawn(async move {
                            println!("Downloading MTG DB in background...");
                            match retrieval::download_mtg_db(&path, Some(progress)).await {
                                Ok(_) => {
                                    let mut state = retrieval.lock().await;
                                    if let Err(e) = state.add_system(Systems::Sql) {
                                        eprintln!("Failed to init MTG system after download: {e}");
                                    } else {
                                        println!("MTG DB ready.");
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to download MTG DB: {e}");
                                    retrieval.lock().await.downloading.remove("Sql");
                                }
                            }
                        });
                    }
                }
                Systems::RiftboundSql => {
                    if let Some(ref path) = riftbound_db_path
                        && !std::path::Path::new(path).exists()
                    {
                        let retrieval = retrieval.clone();
                        let riftbound_db_path = riftbound_db_path.clone();
                        retrieval.lock().await.downloading.insert("RiftboundSql".to_string(), Arc::new(Mutex::new(DownloadProgress::default())));
                        tokio::spawn(async move {
                            println!("Downloading Riftbound DB in background...");
                            match RetrievalState::new_retrieval(Systems::RiftboundSql, riftbound_db_path) {
                                Ok(temp) => match temp.update_backend().await {
                                    Ok(_) => {
                                        let mut state = retrieval.lock().await;
                                        if let Err(e) = state.add_system(Systems::RiftboundSql) {
                                            eprintln!("Failed to init Riftbound system after download: {e}");
                                        } else {
                                            println!("Riftbound DB ready.");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to download Riftbound DB: {e}");
                                        retrieval.lock().await.downloading.remove("RiftboundSql");
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to create Riftbound retrieval for download: {e}");
                                    retrieval.lock().await.downloading.remove("RiftboundSql");
                                }
                            }
                        });
                    }
                }
                Systems::PokemonSql => {
                    if let Some(ref path) = pokemon_db_path
                        && !std::path::Path::new(path).exists()
                    {
                        let path = path.clone();
                        let retrieval = retrieval.clone();
                        retrieval.lock().await.downloading.insert("PokemonSql".to_string(), Arc::new(Mutex::new(DownloadProgress::default())));
                        tokio::spawn(async move {
                            println!("Running pokedata scraper in background...");
                            match RetrievalState::new_retrieval(Systems::PokemonSql, Some(path.clone())) {
                                Ok(temp) => match temp.update_backend().await {
                                    Ok(_) => {
                                        let mut state = retrieval.lock().await;
                                        if let Err(e) = state.add_system(Systems::PokemonSql) {
                                            eprintln!("Failed to init Pokemon system after scrape: {e}");
                                        } else {
                                            println!("Pokemon DB ready.");
                                        }
                                        state.downloading.remove("PokemonSql");
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to run pokedata scraper: {e}");
                                        retrieval.lock().await.downloading.remove("PokemonSql");
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to create Pokemon retrieval for scrape: {e}");
                                    retrieval.lock().await.downloading.remove("PokemonSql");
                                }
                            }
                        });
                    }
                }
                _ => {
                    println!("Downloading updates not implemented for {system:?}");
                }
            }
        }
    }
    let storage = Arc::new(Mutex::new(StorageState::new(storage_db_path)?));

    let mut api = OpenApi {
        info: Info {
            title: "GatheRs API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let cors = CorsLayer::permissive();
    let app = ApiRouter::new()
        .nest("/mtg", mtg_routes())
        .nest("/riftbound", riftbound_routes())
        .nest("/pokemon", pokemon_routes())
        .nest("/collection", collection_routes())
        .api_route("/system", get(get_system_info))
        .route("/api.json", axum::routing::get(serve_api))
        .route("/swagger", Swagger::new("/api.json").axum_route())
        .finish_api(&mut api)
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(axum::http::StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {error}"),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .layer(cors)
        .layer(Extension(api))
        .with_state((retrieval, storage));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    debug!(port = ?port, "Started server" );

    axum::serve(listener, app).await?;

    Ok(())
}
