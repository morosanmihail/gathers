//! Routes for third-party plugins (see `retrieval::systems::plugin`).
//! Unlike mtg/pokemon/riftbound, which each get a dedicated module because
//! they're distinct, fixed Rust types, any number of plugins share this one
//! generic `/api/plugins/{name}/...` surface, dispatched by name at runtime.

use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use retrieval::{PluginCard, PluginSearchFilters};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ApiError, ErrorPayload, GathersState, demo_mode, demo_err};

#[derive(Debug, Serialize, JsonSchema)]
struct PluginSummary {
    name: String,
    base_url: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
struct SearchRequest {
    #[serde(default)]
    filters: PluginSearchFilters,
    #[serde(default)]
    skip: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

pub fn plugin_routes() -> ApiRouter<GathersState> {
    async fn list_plugins(State(state): State<GathersState>) -> Json<Vec<PluginSummary>> {
        let ret = state.0.lock().await;
        Json(
            ret.plugins
                .values()
                .map(|p| PluginSummary {
                    name: p.name.clone(),
                    base_url: p.base_url.clone(),
                })
                .collect(),
        )
    }

    async fn search(
        State(state): State<GathersState>,
        Path(name): Path<String>,
        Json(body): Json<SearchRequest>,
    ) -> Result<Json<Vec<PluginCard>>, ApiError> {
        let ret = state.0.lock().await;
        let plugin = ret.require_plugin(&name)?;
        plugin
            .search(body.filters, body.skip, body.limit)
            .await
            .map(Json)
            .map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorPayload {
                        error: format!("Plugin search failed: {e}"),
                    }),
                )
            })
    }

    async fn cards_by_ids(
        State(state): State<GathersState>,
        Path(name): Path<String>,
        Json(ids): Json<Vec<String>>,
    ) -> Result<Json<HashMap<String, PluginCard>>, ApiError> {
        let ret = state.0.lock().await;
        let plugin = ret.require_plugin(&name)?;
        plugin.cards_by_ids(ids).await.map(Json).map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorPayload {
                    error: format!("Plugin lookup failed: {e}"),
                }),
            )
        })
    }

    // Plugins are expected to background their own update work and respond
    // immediately (see the module doc on retrieval::systems::plugin), so —
    // unlike mtg/pokemon/riftbound's `update` — this doesn't need its own
    // tokio::spawn here; the plugin call itself should already be fast.
    async fn update(
        State(state): State<GathersState>,
        Path(name): Path<String>,
    ) -> Result<Json<String>, ApiError> {
        if demo_mode() {
            return Err(demo_err());
        }
        let ret = state.0.lock().await;
        let plugin = ret.require_plugin(&name)?;
        match plugin.update().await {
            Ok(true) => Ok(Json("Update started".to_string())),
            Ok(false) => Ok(Json("Plugin did not start an update".to_string())),
            Err(e) => Err((
                StatusCode::BAD_GATEWAY,
                Json(ErrorPayload {
                    error: format!("Plugin update failed: {e}"),
                }),
            )),
        }
    }

    ApiRouter::new()
        .api_route("/", get(list_plugins))
        .api_route("/{name}/search", post(search))
        .api_route("/{name}/cards/by-ids", post(cards_by_ids))
        .api_route("/{name}/update", post(update))
}
