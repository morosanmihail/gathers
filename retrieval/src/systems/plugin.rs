//! Client for the gathers plugin HTTP contract, letting a third-party
//! service supply its own card-shaped catalog without being compiled into
//! this binary.
//!
//! `models::Card` is a closed enum over Magic/Riftbound/Pokemon with
//! exhaustive matches throughout the server (collections, pricing, search)
//! — extending it for an arbitrary third-party domain (books, coins, ...)
//! would mean every one of those call sites has to know how to handle a
//! domain it was never designed for. So a plugin doesn't return
//! `models::Card` and isn't part of the `RetrievalSystem` enum_dispatch
//! set; instead it speaks its own small, deliberately generic wire format
//! (`PluginCard`) over HTTP, and gets its own parallel `/api/plugins/{name}`
//! routes on the server side rather than plugging into the MTG/Pokemon/
//! Riftbound-shaped trait.
//!
//! Wire contract a plugin implements, rooted at its configured `base_url`:
//!   `GET  /gathers-plugin/v1/info`          -> `PluginInfo`
//!   `POST /gathers-plugin/v1/search`        -> `PluginSearchRequest` -> `Vec<PluginCard>`
//!   `POST /gathers-plugin/v1/cards/by-ids`  -> `Vec<String>` -> `HashMap<String, PluginCard>`
//!   `POST /gathers-plugin/v1/update`        -> `PluginUpdateResponse`
//!
//! `update` is expected to background its own work and respond immediately
//! (mirrors gathers' own `/api/{mtg,pokemon,riftbound}/update`, which had to
//! be fixed to stop blocking inline under the server's global 10s request
//! timeout — a plugin blocking its own HTTP response the same way would hit
//! the identical failure mode).

use std::{collections::HashMap, time::Duration};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A card-shaped item returned by a plugin. Fields beyond `id`/`name` are
/// deliberately loose (empty string / `None` / empty map are all valid) so
/// a plugin author only has to fill in what makes sense for their domain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PluginCard {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub set_code: String,
    #[serde(default)]
    pub set_name: String,
    #[serde(default)]
    pub collector_number: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    /// Freeform, domain-specific fields (e.g. "author" for a books plugin),
    /// opaque to gathers itself.
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PluginSearchFilters {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub set_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PluginSearchRequest {
    #[serde(default)]
    pub filters: PluginSearchFilters,
    #[serde(default)]
    pub skip: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginUpdateResponse {
    pub started: bool,
}

/// A proxy to one configured plugin instance. Any number of these can be
/// registered (see `PluginConfig` in the server crate) — this type is
/// parameterized by `base_url`/`name` rather than needing a new Rust type
/// per third-party plugin.
#[derive(Debug, Clone)]
pub struct PluginRetrievalSystem {
    pub name: String,
    pub base_url: String,
    client: reqwest::Client,
}

/// Deliberately shorter than the server's global 10s request timeout
/// (`server/src/main.rs`'s `.timeout(Duration::from_secs(10))` layer, which
/// wraps every route including `/api/plugins/{name}/search`). If this were
/// longer, a slow or dead plugin would get its call silently cut by that
/// outer layer first — the caller would just see an opaque 408 with no
/// indication which plugin failed or why. Timing out here instead means the
/// failure comes back as a real `reqwest` error, which `plugin_api.rs`
/// turns into a specific "Plugin search failed: ..." message.
const PLUGIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

impl PluginRetrievalSystem {
    pub fn new(name: String, base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(PLUGIN_REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            name,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    pub async fn info(&self) -> eyre::Result<PluginInfo> {
        Ok(self
            .client
            .get(format!("{}/gathers-plugin/v1/info", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn search(
        &self,
        filters: PluginSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<PluginCard>> {
        Ok(self
            .client
            .post(format!("{}/gathers-plugin/v1/search", self.base_url))
            .json(&PluginSearchRequest {
                filters,
                skip,
                limit,
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn cards_by_ids(
        &self,
        ids: Vec<String>,
    ) -> eyre::Result<HashMap<String, PluginCard>> {
        Ok(self
            .client
            .post(format!("{}/gathers-plugin/v1/cards/by-ids", self.base_url))
            .json(&ids)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    /// Triggers the plugin's own backend refresh. Returns whether the
    /// plugin reported it actually started one.
    pub async fn update(&self) -> eyre::Result<bool> {
        let resp: PluginUpdateResponse = self
            .client
            .post(format!("{}/gathers-plugin/v1/update", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.started)
    }
}
