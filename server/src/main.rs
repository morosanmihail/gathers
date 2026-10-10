use aide::axum::{ApiRouter, routing::get};
use aide::openapi::{Info, OpenApi};
use aide::swagger::Swagger;
use axum::http::StatusCode;
use axum::{Extension, Json, error_handling::HandleErrorLayer, extract::State};
use clap::{Parser, ValueEnum};
use persistence::{PersistenceSystem, PersistenceSystemTrait as _};
use retrieval::{DownloadProgress, NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};
use schemars::JsonSchema;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tower::{BoxError, ServiceBuilder};
use axum::http::HeaderValue;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::collections::{collection_routes, public_collection_routes};
use crate::collections::collections_models::APIUniqueMode;
use crate::mtg_api::mtg_routes;
use crate::plugin_api::plugin_routes;
use crate::pokemon_api::pokemon_routes;
use crate::riftbound_api::riftbound_routes;
use crate::settings_api::settings_routes;

mod auto_download;
mod collections;
mod mtg_api;
mod plugin_api;
mod pokemon_api;
mod price_history;
mod prices_api;
mod riftbound_api;
mod settings_api;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ErrorPayload {
    pub error: String,
}

/// Convenience alias for the standard API error response.
pub type ApiError = (StatusCode, Json<ErrorPayload>);

/// Rejects a `unique` mode the system doesn't offer up front, so a typo is a 400 naming the
/// valid modes rather than a generic search failure. A system that offers no modes ignores
/// `unique`, so nothing is rejected for it.
pub(crate) fn check_unique_mode(
    system: &RetrievalSystem,
    unique: Option<&str>,
) -> Result<(), ApiError> {
    retrieval::resolve_unique_mode(&system.unique_modes(), unique)
        .map(|_| ())
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload { error: e.to_string() }),
            )
        })
}

/// Maps a storage error to a response: request problems the persistence layer
/// reports (`persistence::PersistenceError`) become the matching 4xx, anything
/// else is a 500 prefixed with `context`.
pub(crate) fn storage_error(context: &str, e: eyre::Report) -> ApiError {
    use persistence::PersistenceError as E;
    let status = match e.downcast_ref::<E>() {
        Some(E::CollectionNotFound(_)) => StatusCode::NOT_FOUND,
        Some(E::CollectionExists(_)) => StatusCode::CONFLICT,
        Some(E::CollectionNotRemovable(_) | E::InvalidInput(_)) => StatusCode::BAD_REQUEST,
        None => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let error = if status == StatusCode::INTERNAL_SERVER_ERROR {
        format!("{context}. {e}")
    } else {
        e.to_string()
    };
    (status, Json(ErrorPayload { error }))
}

pub(crate) fn bad_request(error: impl Into<String>) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(ErrorPayload { error: error.into() }))
}

/// Most results a single search/list page returns; larger `limit`s are
/// capped to it. Keeps one request from dumping (and holding a connection
/// for) an entire card database.
pub(crate) const MAX_PAGE_SIZE: usize = 1000;

pub(crate) fn demo_mode() -> bool {
    std::env::var("DEMO_MODE").is_ok()
}

pub(crate) fn demo_err() -> ApiError {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorPayload {
            error: "Disabled in demo mode".to_string(),
        }),
    )
}

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
    /// Names of configured third-party plugins (see `PluginConfig`). Kept
    /// separate from `systems` — a plugin isn't a `provider` collections can
    /// store cards under, and doesn't support the same search filters.
    pub plugins: Vec<String>,
    /// The ways each system can collapse search results that share a card
    /// (see `APICardSearchFilters::unique`), keyed by system name; the first
    /// is the default. Systems with no modes are left out — they have
    /// nothing to toggle.
    pub unique_modes: HashMap<String, Vec<APIUniqueMode>>,
    /// Systems whose databases are currently being downloaded, with progress info.
    pub downloading: HashMap<String, DownloadProgressInfo>,
    /// Whether the server is running in demo mode (settings endpoints disabled).
    pub demo_mode: bool,
    /// Whether pricing support is enabled (market prices, purchase history, etc.).
    pub pricing_enabled: bool,
    /// Whether collection management is enabled.
    pub collections_enabled: bool,
    /// Whether daily prices of collection cards are being recorded (see
    /// `ServerConfig::price_history_enabled`). False when enabled but its
    /// database couldn't be opened.
    pub price_history_enabled: bool,
    /// ISO 4217 code of the currency card prices are taken in when a card is
    /// listed in several (see `ServerConfig::preferred_currency`).
    pub preferred_currency: String,
    /// Whether settings were saved that only take effect after a server
    /// restart. Stays set until the server restarts.
    pub restart_required: bool,
    /// Server version: the git tag it was built from (e.g. `v0.6.4`, or
    /// `v0.6.4-18-ga2de264` for a build past a tag).
    pub version: String,
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
    mtg_prices_path: Option<String>,
    riftbound_db_path: Option<String>,
    riftbound_prices_path: Option<String>,
    pokemon_db_path: Option<String>,
    pokemon_prices_path: Option<String>,
    /// Path to the server config file, for settings API.
    pub config_path: std::path::PathBuf,
    /// Progress trackers for in-progress downloads, keyed by system name.
    pub downloading: HashMap<String, Arc<Mutex<DownloadProgress>>>,
    pub pricing_enabled: bool,
    pub collections_enabled: bool,
    pub preferred_currency: String,
    /// Whether price history is actually being kept; set once storage is up.
    pub price_history_enabled: bool,
    /// Set when saved settings need a restart to apply; a restart clears it
    /// by starting from a fresh `RetrievalState`.
    pub restart_required: bool,
    /// Third-party plugin proxies, keyed by name. Set after construction —
    /// see `PluginConfig` and the wiring in `main()`.
    pub plugins: HashMap<String, retrieval::PluginRetrievalSystem>,
}

/// The `provider` string stored on collection cards that come from plugin
/// `name`. Always lowercase, whatever case the plugin was configured with
/// (`Books` -> `plugin-books`); the configured name is still what's shown to
/// users and what keys `RetrievalState::plugins`.
pub fn plugin_provider(name: &str) -> String {
    format!("plugin-{}", name.to_lowercase())
}

/// Looks a plugin up by name ignoring case, so a lowercase stored provider
/// (or a row stored before providers were lowercased) still resolves to a
/// plugin configured as `Books`. An exact match wins over a case-folded one.
pub fn find_plugin<'a, T>(plugins: &'a HashMap<String, T>, name: &str) -> Option<&'a T> {
    let lowered = name.to_lowercase();
    plugins.get(name).or_else(|| {
        plugins
            .iter()
            .find(|(configured, _)| configured.to_lowercase() == lowered)
            .map(|(_, plugin)| plugin)
    })
}

#[derive(Debug, Clone)]
pub struct StorageState {
    storage: PersistenceSystem,
    _storage_db_path: Option<String>,
}

impl RetrievalState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        systems: Vec<Systems>,
        mtg_db_path: Option<String>,
        mtg_prices_path: Option<String>,
        riftbound_db_path: Option<String>,
        riftbound_prices_path: Option<String>,
        pokemon_db_path: Option<String>,
        pokemon_prices_path: Option<String>,
        config_path: std::path::PathBuf,
        pricing_enabled: bool,
        collections_enabled: bool,
        preferred_currency: String,
    ) -> eyre::Result<RetrievalState> {
        let mut state = RetrievalState {
            mtg: None,
            riftbound: None,
            pokemon: None,
            mtg_system_type: None,
            mtg_db_path: mtg_db_path.clone(),
            mtg_prices_path: mtg_prices_path.clone(),
            riftbound_db_path: riftbound_db_path.clone(),
            riftbound_prices_path: riftbound_prices_path.clone(),
            pokemon_db_path: pokemon_db_path.clone(),
            pokemon_prices_path: pokemon_prices_path.clone(),
            config_path,
            downloading: HashMap::new(),
            pricing_enabled,
            collections_enabled,
            preferred_currency,
            price_history_enabled: false,
            restart_required: false,
            plugins: HashMap::new(),
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
            let prices_path = match system {
                Systems::Scryfall | Systems::Sql => mtg_prices_path.clone(),
                Systems::RiftboundSql => riftbound_prices_path.clone(),
                Systems::PokemonSql => pokemon_prices_path.clone(),
            };
            let retrieval = Self::new_retrieval(system, db_path, prices_path)?;
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
        prices_db_path: Option<String>,
    ) -> eyre::Result<RetrievalSystem> {
        Ok(match system {
            Systems::Scryfall => {
                RetrievalSystem::ScryfallRetrievalSystem(retrieval::ScryfallRetrievalSystem::new()?)
            }
            Systems::Sql => RetrievalSystem::MagicSQLiteRetrievalSystem(
                retrieval::MagicSQLiteRetrievalSystem::new(retrieval_db_path.clone(), prices_db_path)?,
            ),
            Systems::RiftboundSql => RetrievalSystem::RiftboundSQLiteRetrievalSystem(
                retrieval::RiftboundSQLiteRetrievalSystem::new(retrieval_db_path.clone(), prices_db_path)?,
            ),
            Systems::PokemonSql => RetrievalSystem::PokemonSQLiteRetrievalSystem(
                retrieval::PokemonSQLiteRetrievalSystem::new(retrieval_db_path.clone(), prices_db_path)?,
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
        let active: Vec<&RetrievalSystem> = [
            self.mtg.as_ref(),
            self.riftbound.as_ref(),
            self.pokemon.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let systems: Vec<String> = active.iter().map(|s| s.name().to_string()).collect();
        let unique_modes: HashMap<String, Vec<APIUniqueMode>> = active
            .iter()
            .map(|s| (s.name().to_string(), s.unique_modes()))
            .filter(|(_, modes)| !modes.is_empty())
            .map(|(name, modes)| (name, modes.into_iter().map(APIUniqueMode::from).collect()))
            .collect();
        let plugins: Vec<String> = self.plugins.keys().cloned().collect();
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
        let demo_mode = std::env::var("DEMO_MODE").is_ok();
        SystemInfo { system, systems, plugins, unique_modes, downloading, demo_mode, pricing_enabled: self.pricing_enabled, collections_enabled: self.collections_enabled, price_history_enabled: self.price_history_enabled, preferred_currency: self.preferred_currency.clone(), restart_required: self.restart_required, version: env!("GATHERS_VERSION").to_string() }
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

    pub fn require_plugin(&self, name: &str) -> Result<&retrieval::PluginRetrievalSystem, ApiError> {
        find_plugin(&self.plugins, name).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: format!("No plugin named '{name}'"),
                }),
            )
        })
    }

    /// Marks the database behind `key` (e.g. `Sql`, `PokemonSql-prices`) as
    /// being updated, so a second update can't start writing the same files
    /// concurrently. Fails with 409 if one is already running; pair with
    /// `finish_download`.
    pub fn start_download(&mut self, key: &str) -> Result<Arc<Mutex<DownloadProgress>>, ApiError> {
        if self.downloading.contains_key(key) {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorPayload {
                    error: format!("An update of {key} is already running"),
                }),
            ));
        }
        let progress = Arc::new(Mutex::new(DownloadProgress::default()));
        self.downloading.insert(key.to_string(), progress.clone());
        Ok(progress)
    }

    pub fn finish_download(&mut self, key: &str) {
        self.downloading.remove(key);
    }

    pub fn reload_mtg(&mut self) -> eyre::Result<()> {
        if let Some(system) = self.mtg_system_type {
            self.mtg = Some(Self::new_retrieval(system, self.mtg_db_path.clone(), self.mtg_prices_path.clone())?);
        }
        Ok(())
    }

    pub fn add_system(&mut self, system: Systems) -> eyre::Result<()> {
        let db_path = match system {
            Systems::Scryfall | Systems::Sql => self.mtg_db_path.clone(),
            Systems::RiftboundSql => self.riftbound_db_path.clone(),
            Systems::PokemonSql => self.pokemon_db_path.clone(),
        };
        let prices_path = match system {
            Systems::Scryfall | Systems::Sql => self.mtg_prices_path.clone(),
            Systems::RiftboundSql => self.riftbound_prices_path.clone(),
            Systems::PokemonSql => self.pokemon_prices_path.clone(),
        };
        let retrieval = Self::new_retrieval(system, db_path, prices_path)?;
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
                self.riftbound_prices_path.clone(),
            )?);
        }
        Ok(())
    }

    pub fn reload_pokemon(&mut self) -> eyre::Result<()> {
        if self.pokemon.is_some() {
            self.pokemon = Some(Self::new_retrieval(
                Systems::PokemonSql,
                self.pokemon_db_path.clone(),
                self.pokemon_prices_path.clone(),
            )?);
        }
        Ok(())
    }
}

impl StorageState {
    /// Opens the storage database and, when `price_history_db_path` is
    /// given, the price history one next to it. Price history is optional:
    /// if its database can't be opened, that's logged and storage carries
    /// on without it.
    pub fn new(storage_db_path: Option<String>, price_history_db_path: Option<String>) -> eyre::Result<StorageState> {
        let mut sqlite = persistence::SQLitePersistenceSystem::new(false, storage_db_path.clone())?;
        if let Some(path) = price_history_db_path {
            match sqlite.enable_price_history(false, Some(path.clone())) {
                Ok(()) => info!(path = %path, "Price history DB ready"),
                Err(e) => warn!(path = %path, error = %e, "Failed to open price history DB — price history disabled"),
            }
        }
        Ok(StorageState {
            storage: PersistenceSystem::SQLitePersistenceSystem(sqlite),
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

fn default_pricing_enabled() -> bool { true }
fn default_collections_enabled() -> bool { true }
fn default_preferred_currency() -> String { "EUR".to_string() }
fn default_plugin_enabled() -> bool { true }

/// A third-party retrieval plugin — a separate HTTP service implementing
/// the gathers plugin contract (see `retrieval::systems::plugin`). Any
/// number may be configured.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct PluginConfig {
    /// Unique name this plugin is addressed by, e.g. `/api/plugins/{name}/...`.
    pub name: String,
    /// Base URL the plugin's HTTP service is reachable at.
    pub base_url: String,
    #[serde(default = "default_plugin_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct ServerConfig {
    system: Vec<Systems>,
    port: usize,
    #[serde(default = "default_pricing_enabled")]
    pub pricing_enabled: bool,
    #[serde(default = "default_collections_enabled")]
    pub collections_enabled: bool,
    /// ISO 4217 code (e.g. "EUR", "USD"). When a card is listed by retailers
    /// in several currencies, prices in this one are used first — for shown
    /// prices and collection totals alike.
    #[serde(default = "default_preferred_currency")]
    pub preferred_currency: String,
    /// Periodically re-download card and price databases for all active systems.
    #[serde(default = "auto_download::default_enabled")]
    pub auto_download_enabled: bool,
    /// How often to run the auto-download, in hours. Takes effect on server restart.
    #[serde(default = "auto_download::default_interval_hours")]
    pub auto_download_interval_hours: u64,
    /// Record the daily prices of cards in collections, in a separate
    /// database (`price_history_db_path`). Recorded whenever a price
    /// database is updated, when cards are added, and on startup while
    /// nothing has been recorded yet. Takes effect on server restart.
    #[serde(default)]
    pub price_history_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtg_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtg_prices_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    riftbound_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    riftbound_prices_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pokemon_db_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pokemon_prices_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_db_path: Option<String>,
    /// Defaults to `storage.prices.db` next to the storage database.
    #[serde(skip_serializing_if = "Option::is_none")]
    price_history_db_path: Option<String>,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    /// Browser origins (e.g. `https://cards.example.com`) allowed to call
    /// this API from another origin. Empty by default: the bundled web UI
    /// is served from the same origin (behind its proxy), and allowing any
    /// origin would let every website a user visits read — and, as there's
    /// no authentication, change — their collections. Also settable as a
    /// comma-separated list in `GATHERS_CORS_ORIGINS`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cors_allowed_origins: Vec<String>,
}

impl ServerConfig {
    /// Rejects settings the server couldn't start with (or that would make
    /// it misbehave) before they're written to disk.
    fn validate(&self) -> Result<(), String> {
        if !(1..=65535).contains(&self.port) {
            return Err(format!("Port must be between 1 and 65535, got {}", self.port));
        }
        persistence::validate_currency(&self.preferred_currency)?;
        if self.system.is_empty() {
            return Err("At least one system must be enabled".to_string());
        }
        if self.system.contains(&Systems::Scryfall) && self.system.contains(&Systems::Sql) {
            return Err("Only one MTG system (Scryfall or Sql) can be enabled".to_string());
        }
        if !(1..=auto_download::MAX_INTERVAL_HOURS).contains(&self.auto_download_interval_hours) {
            return Err(format!(
                "Auto-download interval must be between 1 and {} hours",
                auto_download::MAX_INTERVAL_HOURS
            ));
        }
        let mut names = std::collections::HashSet::new();
        for plugin in &self.plugins {
            let name = plugin.name.trim();
            if name.is_empty() || name != plugin.name {
                return Err(format!("Invalid plugin name '{}'", plugin.name));
            }
            if !names.insert(name.to_lowercase()) {
                return Err(format!("Duplicate plugin name '{name}'"));
            }
            if !(plugin.base_url.starts_with("http://") || plugin.base_url.starts_with("https://")) {
                return Err(format!(
                    "Plugin '{name}' base URL must start with http:// or https://"
                ));
            }
        }
        for origin in &self.cors_allowed_origins {
            if origin.trim_end_matches('/') != origin
                || !(origin.starts_with("http://") || origin.starts_with("https://"))
            {
                return Err(format!(
                    "Invalid CORS origin '{origin}' (expected e.g. https://cards.example.com)"
                ));
            }
        }
        Ok(())
    }

    /// Whether going from `self` to `new` changes anything that is only read
    /// at startup. `pricing_enabled`, `collections_enabled` and
    /// `preferred_currency` are applied to the running server on save;
    /// everything else needs a restart.
    fn restart_needed(&self, new: &ServerConfig) -> bool {
        let mut live_applied = self.clone();
        live_applied.pricing_enabled = new.pricing_enabled;
        live_applied.collections_enabled = new.collections_enabled;
        live_applied.preferred_currency = new.preferred_currency.clone();
        toml::to_string(&live_applied).ok() != toml::to_string(new).ok()
    }
}

/// `server.toml` path keys and the env vars that override them.
pub const PATH_ENV_VARS: &[(&str, &str)] = &[
    ("mtg_db_path", "MTG_DB_PATH"),
    ("mtg_prices_path", "MTG_PRICES_PATH"),
    ("riftbound_db_path", "RIFTBOUND_DB_PATH"),
    ("riftbound_prices_path", "RIFTBOUND_PRICES_PATH"),
    ("pokemon_db_path", "POKEMON_DB_PATH"),
    ("pokemon_prices_path", "POKEMON_PRICES_PATH"),
    ("storage_db_path", "STORAGE_DB_PATH"),
    ("price_history_db_path", "PRICE_HISTORY_DB_PATH"),
];

/// Systems from `GATHERS_SYSTEMS` (comma-separated, e.g. "scryfall,riftbound-sql"),
/// or `None` when unset or nothing in it parses. Unknown entries are reported
/// on stderr when `warn` is set.
pub fn env_systems(warn: bool) -> Option<Vec<Systems>> {
    let val = std::env::var("GATHERS_SYSTEMS").ok()?;
    let parsed: Vec<Systems> = val
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            Systems::from_str(s, true).map_err(|e| {
                if warn {
                    eprintln!("warning: unknown system in GATHERS_SYSTEMS '{s}': {e}");
                }
            }).ok()
        })
        .collect();
    (!parsed.is_empty()).then_some(parsed)
}

/// `GATHERS_PRICE_HISTORY` (`true`/`1`/`false`/`0`, any case), overriding
/// `ServerConfig::price_history_enabled`. `None` when unset or unparseable.
pub fn env_price_history() -> Option<bool> {
    let val = std::env::var("GATHERS_PRICE_HISTORY").ok()?;
    match val.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Origins from `GATHERS_CORS_ORIGINS` (comma-separated), trailing slashes trimmed.
fn env_cors_origins() -> Vec<String> {
    std::env::var("GATHERS_CORS_ORIGINS")
        .map(|val| {
            val.split(',')
                .map(|o| o.trim().trim_end_matches('/').to_string())
                .filter(|o| !o.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Parser, Debug)]
#[command(version = env!("GATHERS_VERSION"), about)]
struct Args {
    /// Retrieval systems to enable. May be specified multiple times.
    /// Required when no config file exists. Supported values: scryfall, sql, riftbound-sql, pokemon-sql.
    #[clap(short, long, num_args = 1..)]
    system: Vec<Systems>,

    /// Port to listen on. Required when no config file exists.
    #[clap(short, long)]
    port: Option<usize>,

    /// Print the OpenAPI schema as JSON to stdout and exit, without starting the server.
    #[clap(long)]
    print_schema: bool,
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

fn openapi_doc() -> OpenApi {
    OpenApi {
        info: Info {
            title: "GatheRs API".to_string(),
            version: env!("GATHERS_VERSION").to_string(),
            ..Info::default()
        },
        ..OpenApi::default()
    }
}

fn api_router(api: &mut OpenApi) -> axum::Router<GathersState> {
    ApiRouter::new()
        .nest("/api/mtg", mtg_routes())
        .nest("/api/riftbound", riftbound_routes())
        .nest("/api/pokemon", pokemon_routes())
        .nest("/api/collection", collection_routes())
        .nest("/api/share", public_collection_routes())
        .nest("/api/settings", settings_routes())
        .nest("/api/plugins", plugin_routes())
        .api_route("/api/system", get(get_system_info))
        .route("/api.json", axum::routing::get(serve_api))
        .route("/swagger", Swagger::new("/api.json").axum_route())
        .finish_api(api)
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    if args.print_schema {
        let mut api = openapi_doc();
        let _ = api_router(&mut api);
        println!("{}", serde_json::to_string_pretty(&api)?);
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(version = env!("GATHERS_VERSION"), "GatheRs server starting");

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let gathers_dir = std::path::Path::new(&home).join(".local/share/gathers");
    let db_dir = gathers_dir.join("DB");
    let config_path = gathers_dir.join("server.toml");

    info!(config = %config_path.display(), "Loading config");

    // Load or create config file
    let mut config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let cfg = toml::from_str::<ServerConfig>(&content)
            .map_err(|e| eyre::eyre!("Failed to parse {}: {e}", config_path.display()))?;
        info!(config = %config_path.display(), "Config loaded");
        cfg
    } else {
        // Seed the new config from the same env vars that override it at
        // runtime, so the file on disk reflects what the server actually uses.
        let systems = env_systems(false)
            .or_else(|| (!args.system.is_empty()).then(|| args.system.clone()))
            .unwrap_or_else(|| vec![Systems::RiftboundSql]);
        let port = args.port.unwrap_or(5234);
        let path_or_default = |var: &str, file: &str| {
            Some(
                std::env::var(var)
                    .unwrap_or_else(|_| db_dir.join(file).to_string_lossy().into_owned()),
            )
        };
        let cfg = ServerConfig {
            system: systems,
            port,
            pricing_enabled: true,
            collections_enabled: true,
            preferred_currency: default_preferred_currency(),
            auto_download_enabled: false,
            auto_download_interval_hours: 24,
            price_history_enabled: env_price_history().unwrap_or(false),
            mtg_db_path: path_or_default("MTG_DB_PATH", "AllPrintings.db"),
            mtg_prices_path: path_or_default("MTG_PRICES_PATH", "AllPricesToday.sqlite"),
            riftbound_db_path: path_or_default("RIFTBOUND_DB_PATH", "riftbound.db"),
            riftbound_prices_path: path_or_default("RIFTBOUND_PRICES_PATH", retrieval::RIFTBOUND_PRICES_FILE),
            pokemon_db_path: path_or_default("POKEMON_DB_PATH", "pokemon.db"),
            pokemon_prices_path: path_or_default("POKEMON_PRICES_PATH", retrieval::POKEMON_PRICES_FILE),
            storage_db_path: path_or_default("STORAGE_DB_PATH", "storage.db"),
            price_history_db_path: None,
            plugins: Vec::new(),
            cors_allowed_origins: env_cors_origins(),
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
        info!(config = %config_path.display(), "Config created");
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
    info!(db_dir = %db_dir.display(), "Database directory ready");

    if let Err(e) = config.validate() {
        warn!(config = %config_path.display(), "Config problem: {e}");
    }

    // CLI args override config for this session
    if !args.system.is_empty() {
        config.system = args.system;
    }
    if let Some(port) = args.port {
        config.port = port;
    }

    // GATHERS_SYSTEMS env var overrides config (comma-separated, e.g. "scryfall,riftbound-sql")
    if let Some(systems) = env_systems(true) {
        config.system = systems;
    }
    if let Some(enabled) = env_price_history() {
        config.price_history_enabled = enabled;
    }

    // Env vars override config for DB paths
    let mtg_db_path = std::env::var("MTG_DB_PATH").ok().or(config.mtg_db_path);
    let mtg_prices_path = std::env::var("MTG_PRICES_PATH")
        .ok()
        .or(config.mtg_prices_path)
        .or_else(|| {
            // Derive default from MTG DB path when not explicitly configured
            // (handles old config files that predate this field).
            mtg_db_path.as_ref().map(|p| {
                std::path::Path::new(p)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join("AllPricesToday.sqlite")
                    .to_string_lossy()
                    .into_owned()
            })
        });
    let riftbound_db_path = std::env::var("RIFTBOUND_DB_PATH")
        .ok()
        .or(config.riftbound_db_path);
    let riftbound_prices_path = std::env::var("RIFTBOUND_PRICES_PATH")
        .ok()
        .or(config.riftbound_prices_path)
        .or_else(|| {
            riftbound_db_path.as_ref().map(|p| {
                std::path::Path::new(p)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(retrieval::RIFTBOUND_PRICES_FILE)
                    .to_string_lossy()
                    .into_owned()
            })
        });
    let pokemon_db_path = std::env::var("POKEMON_DB_PATH")
        .ok()
        .or(config.pokemon_db_path);
    let pokemon_prices_path = std::env::var("POKEMON_PRICES_PATH")
        .ok()
        .or(config.pokemon_prices_path)
        .or_else(|| {
            pokemon_db_path.as_ref().map(|p| {
                std::path::Path::new(p)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(retrieval::POKEMON_PRICES_FILE)
                    .to_string_lossy()
                    .into_owned()
            })
        });
    let storage_db_path = std::env::var("STORAGE_DB_PATH")
        .ok()
        .or(config.storage_db_path);
    let price_history_db_path = config.price_history_enabled.then(|| {
        std::env::var("PRICE_HISTORY_DB_PATH")
            .ok()
            .or(config.price_history_db_path.clone())
            .unwrap_or_else(|| {
                persistence::default_price_history_path(storage_db_path.as_deref().unwrap_or("storage.db"))
            })
    });

    let port = config.port;

    info!(systems = ?config.system, port, "Configuring systems");
    if let Some(ref p) = mtg_db_path { info!(path = %p, "MTG DB path"); }
    if let Some(ref p) = mtg_prices_path { info!(path = %p, "MTG prices path"); }
    if let Some(ref p) = riftbound_db_path { info!(path = %p, "Riftbound DB path"); }
    if let Some(ref p) = riftbound_prices_path { info!(path = %p, "Riftbound prices path"); }
    if let Some(ref p) = pokemon_db_path { info!(path = %p, "Pokemon DB path"); }
    if let Some(ref p) = storage_db_path { info!(path = %p, "Storage DB path"); }

    let mut retrieval_state = RetrievalState::new(
        config.system.clone(),
        mtg_db_path.clone(),
        mtg_prices_path.clone(),
        riftbound_db_path.clone(),
        riftbound_prices_path.clone(),
        pokemon_db_path.clone(),
        pokemon_prices_path.clone(),
        config_path.clone(),
        config.pricing_enabled,
        config.collections_enabled,
        config.preferred_currency.clone(),
    )?;

    {
        let mut seen = std::collections::HashSet::new();
        for p in config.plugins.iter().filter(|p| p.enabled) {
            // Names differing only by case share a provider string, so they
            // count as duplicates too.
            if !seen.insert(p.name.to_lowercase()) {
                warn!(name = %p.name, "Duplicate plugin name in config — only the last one will be used");
            }
        }
    }
    retrieval_state.plugins = config
        .plugins
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            (
                p.name.clone(),
                retrieval::PluginRetrievalSystem::new(p.name.clone(), p.base_url.clone()),
            )
        })
        .collect();
    if !retrieval_state.plugins.is_empty() {
        info!(
            plugins = ?retrieval_state.plugins.keys().collect::<Vec<_>>(),
            "Plugins configured"
        );
    }

    let retrieval = Arc::new(Mutex::new(retrieval_state));

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
                        info!(path = %path, "MTG DB missing — downloading in background");
                        tokio::spawn(async move {
                            match retrieval::download_mtg_db(&path, Some(progress)).await {
                                Ok(_) => {
                                    let mut state = retrieval.lock().await;
                                    if let Err(e) = state.add_system(Systems::Sql) {
                                        error!(error = %e, "Failed to init MTG system after download");
                                    } else {
                                        info!("MTG DB ready");
                                    }
                                }
                                Err(e) => {
                                    error!(error = %e, "Failed to download MTG DB");
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
                        info!(path = %path, "Riftbound DB missing — downloading in background");
                        tokio::spawn(async move {
                            match RetrievalState::new_retrieval(Systems::RiftboundSql, riftbound_db_path, None) {
                                Ok(temp) => match temp.update_backend().await {
                                    Ok(_) => {
                                        let mut state = retrieval.lock().await;
                                        if let Err(e) = state.add_system(Systems::RiftboundSql) {
                                            error!(error = %e, "Failed to init Riftbound system after download");
                                        } else {
                                            info!("Riftbound DB ready");
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to download Riftbound DB");
                                        retrieval.lock().await.downloading.remove("RiftboundSql");
                                    }
                                },
                                Err(e) => {
                                    error!(error = %e, "Failed to create Riftbound retrieval for download");
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
                        info!(path = %path, "Pokemon DB missing — running scraper in background");
                        tokio::spawn(async move {
                            match RetrievalState::new_retrieval(Systems::PokemonSql, Some(path.clone()), None) {
                                Ok(temp) => match temp.update_backend().await {
                                    Ok(_) => {
                                        let mut state = retrieval.lock().await;
                                        if let Err(e) = state.add_system(Systems::PokemonSql) {
                                            error!(error = %e, "Failed to init Pokemon system after scrape");
                                        } else {
                                            info!("Pokemon DB ready");
                                        }
                                        state.downloading.remove("PokemonSql");
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Failed to run pokedata scraper");
                                        retrieval.lock().await.downloading.remove("PokemonSql");
                                    }
                                },
                                Err(e) => {
                                    error!(error = %e, "Failed to create Pokemon retrieval for scrape");
                                    retrieval.lock().await.downloading.remove("PokemonSql");
                                }
                            }
                        });
                    }
                }
                _ => {
                    warn!(system = ?system, "Auto-update not implemented for this system");
                }
            }
        }
    }
    let storage = Arc::new(Mutex::new(StorageState::new(storage_db_path.clone(), price_history_db_path)?));
    info!(path = storage_db_path.as_deref().unwrap_or("(default)"), "Storage DB ready");
    if storage.lock().await.storage.price_history_enabled() {
        retrieval.lock().await.price_history_enabled = true;
        price_history::spawn_startup_snapshot(retrieval.clone(), storage.clone());
    }

    if config.auto_download_enabled {
        auto_download::spawn(retrieval.clone(), storage.clone(), gathers_dir.clone(), config.auto_download_interval_hours);
    }

    let mut api = openapi_doc();

    let mut allowed_origins = config.cors_allowed_origins.clone();
    for origin in env_cors_origins() {
        // A config created on first run already holds the env origins.
        if !allowed_origins.contains(&origin) {
            allowed_origins.push(origin);
        }
    }
    let allowed_origins = Arc::new(allowed_origins);
    let cors = {
        let origins: Vec<HeaderValue> = allowed_origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    };
    if !allowed_origins.is_empty() {
        info!(origins = ?allowed_origins, "Cross-origin requests allowed");
    }

    let app = api_router(&mut api)
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
        .layer(axum::middleware::from_fn_with_state(retrieval.clone(), feature_gate))
        .layer(axum::middleware::from_fn_with_state(allowed_origins, reject_cross_site))
        .layer(cors)
        .layer(Extension(api))
        .with_state((retrieval, storage));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!(port, "Listening on 0.0.0.0:{port}");

    // `POST /api/settings/restart` fires `RESTART`: stop accepting connections,
    // let in-flight requests (including the one that asked) finish, then
    // re-exec ourselves.
    let serve = axum::serve(listener, app).with_graceful_shutdown(RESTART.notified());
    tokio::select! {
        res = serve => res?,
        // Don't let a stuck connection hold the restart up forever.
        _ = async {
            RESTART.notified().await;
            tokio::time::sleep(RESTART_DRAIN_TIMEOUT).await;
        } => warn!("Timed out draining connections — restarting anyway"),
    }

    info!("Restarting");
    Err(reexec().into())
}

/// The host part of a `Host` header or origin authority, without the port.
fn host_without_port(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        // IPv6 literal, e.g. `[::1]:5173`.
        return rest.split(']').next().unwrap_or(rest);
    }
    authority.split(':').next().unwrap_or(authority)
}

/// Whether a request that changes state may proceed, given its `Origin` /
/// `Sec-Fetch-Site` headers. There is no authentication, so without this any
/// website a user visits could submit requests to their server (a plain
/// form POST or `<img src>` needs no CORS permission) and change or delete
/// their collections. Browsers always label such requests; non-browser
/// clients (curl, scripts) send neither header and are unaffected.
fn is_allowed_origin(
    method: &axum::http::Method,
    headers: &axum::http::HeaderMap,
    allowed: &[String],
) -> bool {
    use axum::http::Method;
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let origin = header("origin");
    if origin.is_some_and(|o| allowed.iter().any(|a| a == o)) {
        return true;
    }
    match header("sec-fetch-site") {
        Some("same-origin" | "none") => return true,
        Some("cross-site") => return false,
        // "same-site" (another port or subdomain) or an older browser: fall
        // back to comparing the origin's host with the one requested.
        _ => {}
    }
    match origin {
        None => true,
        Some(origin) => {
            let origin_host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
            let host = header("host").unwrap_or_default();
            host_without_port(origin_host).eq_ignore_ascii_case(host_without_port(host))
        }
    }
}

async fn reject_cross_site(
    State(allowed): State<Arc<Vec<String>>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    if is_allowed_origin(request.method(), request.headers(), &allowed) {
        next.run(request).await
    } else {
        warn!(method = %request.method(), path = %request.uri().path(), "Rejected cross-site request");
        (
            StatusCode::FORBIDDEN,
            Json(ErrorPayload {
                error: "Cross-site request rejected (add the origin to cors_allowed_origins to allow it)".to_string(),
            }),
        )
            .into_response()
    }
}

/// Which setting, if any, has to be on for `path` to be served.
fn required_feature(path: &str) -> Option<&'static str> {
    if path.starts_with("/api/collection/") || path.starts_with("/api/share/") {
        return Some("collections");
    }
    let pricing = ["/api/mtg/prices", "/api/pokemon/prices"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || path.contains("/purchase_history")
        || path.contains("/price_history")
        || path.ends_with("/value_breakdown")
        || path.ends_with("/value_history")
        || path.ends_with("/value_cards");
    pricing.then_some("pricing")
}

/// Enforces `collections_enabled` / `pricing_enabled` for the API, not just
/// the UI: a disabled feature's endpoints answer 403.
async fn feature_gate(
    State(retrieval): State<Arc<Mutex<RetrievalState>>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    if let Some(feature) = required_feature(request.uri().path()) {
        let enabled = {
            let state = retrieval.lock().await;
            match feature {
                "collections" => state.collections_enabled,
                _ => state.pricing_enabled,
            }
        };
        if !enabled {
            return (
                StatusCode::FORBIDDEN,
                Json(ErrorPayload {
                    error: format!("{} are disabled on this server", if feature == "collections" { "Collections" } else { "Prices" }),
                }),
            )
                .into_response();
        }
    }
    next.run(request).await
}

/// Fired (with `notify_waiters`, since both the graceful shutdown and its
/// drain timeout wait on it) by the restart endpoint to make `main` shut down
/// and re-exec.
pub(crate) static RESTART: tokio::sync::Notify = tokio::sync::Notify::const_new();

const RESTART_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Replaces this process with a fresh copy of itself (same args and
/// environment). On unix that's a real `exec`, so the PID stays the same and
/// works without a supervisor; only returns if it failed.
fn reexec() -> std::io::Error {
    let mut exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return e,
    };
    // Linux appends " (deleted)" when the binary was replaced while running
    // (e.g. after an upgrade) — restart into the new one at the same path.
    if let Some(live) = exe.to_str().and_then(|p| p.strip_suffix(" (deleted)")) {
        exe = live.into();
    }
    let mut cmd = std::process::Command::new(exe);
    cmd.args(std::env::args_os().skip(1));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.exec()
    }
    #[cfg(not(unix))]
    match cmd.spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => e,
    }
}

#[cfg(test)]
mod restart_needed_tests {
    use super::*;

    fn config() -> ServerConfig {
        toml::from_str("system = [\"RiftboundSql\"]\nport = 5234").unwrap()
    }

    #[test]
    fn unchanged_config_needs_no_restart() {
        assert!(!config().restart_needed(&config()));
    }

    #[test]
    fn live_applied_toggles_need_no_restart() {
        let mut new = config();
        new.pricing_enabled = !new.pricing_enabled;
        new.collections_enabled = !new.collections_enabled;
        new.preferred_currency = "USD".to_string();
        assert!(!config().restart_needed(&new));
    }

    #[test]
    fn startup_only_settings_need_a_restart() {
        let changes: [fn(&mut ServerConfig); 9] = [
            |c| c.port = 1234,
            |c| c.system.push(Systems::Sql),
            |c| c.auto_download_enabled = !c.auto_download_enabled,
            |c| c.auto_download_interval_hours += 1,
            |c| c.mtg_db_path = Some("/elsewhere.db".into()),
            |c| c.plugins.push(PluginConfig { name: "books".into(), base_url: "http://localhost:5236".into(), enabled: true }),
            |c| c.cors_allowed_origins.push("https://cards.example.com".into()),
            |c| c.price_history_enabled = !c.price_history_enabled,
            |c| c.price_history_db_path = Some("/elsewhere.prices.db".into()),
        ];
        for change in changes {
            let mut new = config();
            change(&mut new);
            assert!(config().restart_needed(&new));
        }
    }
}

#[cfg(test)]
mod plugin_name_tests {
    use super::*;

    #[test]
    fn plugin_provider_is_lowercase() {
        assert_eq!(plugin_provider("Books"), "plugin-books");
        assert_eq!(plugin_provider("books"), "plugin-books");
        assert_eq!(plugin_provider("Dummy-Books"), "plugin-dummy-books");
    }

    #[test]
    fn find_plugin_ignores_case_and_keeps_configured_name() {
        let plugins = HashMap::from([("Books".to_string(), 1)]);
        assert_eq!(find_plugin(&plugins, "books"), Some(&1));
        assert_eq!(find_plugin(&plugins, "Books"), Some(&1));
        assert_eq!(find_plugin(&plugins, "BOOKS"), Some(&1));
        assert_eq!(find_plugin(&plugins, "comics"), None);
    }

    #[test]
    fn find_plugin_prefers_exact_match() {
        let plugins = HashMap::from([("Books".to_string(), 1), ("books".to_string(), 2)]);
        assert_eq!(find_plugin(&plugins, "books"), Some(&2));
        assert_eq!(find_plugin(&plugins, "Books"), Some(&1));
    }
}

#[cfg(test)]
mod config_validation_tests {
    use super::*;

    fn config() -> ServerConfig {
        toml::from_str("system = [\"RiftboundSql\"]\nport = 5234").unwrap()
    }

    #[test]
    fn default_config_is_valid() {
        assert_eq!(config().validate(), Ok(()));
    }

    #[test]
    fn rejects_settings_the_server_cant_start_with() {
        let changes: [fn(&mut ServerConfig); 8] = [
            |c| c.port = 0,
            |c| c.port = 99999,
            |c| c.system.clear(),
            |c| c.system = vec![Systems::Scryfall, Systems::Sql],
            |c| c.auto_download_interval_hours = 0,
            |c| c.auto_download_interval_hours = u64::MAX,
            |c| c.plugins = vec![
                PluginConfig { name: "Books".into(), base_url: "http://a".into(), enabled: true },
                PluginConfig { name: "books".into(), base_url: "http://b".into(), enabled: true },
            ],
            |c| c.cors_allowed_origins.push("cards.example.com".into()),
        ];
        for (i, change) in changes.into_iter().enumerate() {
            let mut c = config();
            change(&mut c);
            assert!(c.validate().is_err(), "change #{i} should be rejected");
        }
    }
}

#[cfg(test)]
mod request_guard_tests {
    use super::*;
    use axum::http::{HeaderMap, Method};

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    #[test]
    fn safe_methods_and_non_browser_clients_pass() {
        let evil = headers(&[("origin", "https://evil.example"), ("sec-fetch-site", "cross-site")]);
        assert!(is_allowed_origin(&Method::GET, &evil, &[]));
        assert!(is_allowed_origin(&Method::POST, &HeaderMap::new(), &[]));
    }

    #[test]
    fn cross_site_writes_are_rejected_unless_allowed() {
        let evil = headers(&[("origin", "https://evil.example"), ("sec-fetch-site", "cross-site"), ("host", "gathers.local:5234")]);
        assert!(!is_allowed_origin(&Method::POST, &evil, &[]));
        assert!(!is_allowed_origin(&Method::DELETE, &evil, &[]));
        assert!(is_allowed_origin(&Method::POST, &evil, &["https://evil.example".to_string()]));
    }

    #[test]
    fn same_origin_writes_pass() {
        let ui = headers(&[("origin", "http://localhost:5173"), ("sec-fetch-site", "same-origin"), ("host", "localhost:5173")]);
        assert!(is_allowed_origin(&Method::POST, &ui, &[]));
        // A proxy that drops the port from Host (nginx's `$host`), in a
        // browser that doesn't send Sec-Fetch-Site.
        let proxied = headers(&[("origin", "http://nas:3001"), ("host", "nas")]);
        assert!(is_allowed_origin(&Method::POST, &proxied, &[]));
        let other_host = headers(&[("origin", "http://evil.example"), ("host", "nas")]);
        assert!(!is_allowed_origin(&Method::POST, &other_host, &[]));
        let ipv6 = headers(&[("origin", "http://[::1]:5173"), ("host", "[::1]:5234")]);
        assert!(is_allowed_origin(&Method::POST, &ipv6, &[]));
    }

    #[test]
    fn features_gate_their_endpoints() {
        assert_eq!(required_feature("/api/collection/list"), Some("collections"));
        assert_eq!(required_feature("/api/share/abc"), Some("collections"));
        assert_eq!(required_feature("/api/mtg/prices"), Some("pricing"));
        assert_eq!(required_feature("/api/pokemon/prices/update"), Some("pricing"));
        assert_eq!(required_feature("/api/mtg/cards/search"), None);
        assert_eq!(required_feature("/api/system"), None);
        assert_eq!(required_feature("/api/settings"), None);
    }

    #[test]
    fn storage_errors_map_to_status_codes() {
        use persistence::PersistenceError as E;
        let status = |e: E| storage_error("ctx", e.into()).0;
        assert_eq!(status(E::CollectionNotFound("x".into())), StatusCode::NOT_FOUND);
        assert_eq!(status(E::CollectionExists("x".into())), StatusCode::CONFLICT);
        assert_eq!(status(E::CollectionNotRemovable("x".into())), StatusCode::BAD_REQUEST);
        assert_eq!(status(E::InvalidInput("x".into())), StatusCode::BAD_REQUEST);
        let (status, Json(body)) = storage_error("Failed to do it", eyre::eyre!("disk on fire"));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Failed to do it. disk on fire");
    }
}
