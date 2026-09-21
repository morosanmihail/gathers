use std::collections::HashMap;
use std::sync::Arc;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::http::StatusCode;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use models::{Card, CardPrices, Set};
use retrieval::{DownloadProgress, RetrievalSystemTrait as _};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::{
    ApiError, ErrorPayload, GathersState, demo_mode, demo_err,
    collections::collections_models::APICardSearchFilters,
    mtg_api::mtg_api_models::APICard,
};
pub mod mtg_api_models;

fn default_limit() -> usize {
    10
}

pub fn mtg_routes() -> ApiRouter<GathersState> {
    #[derive(Deserialize, JsonSchema)]
    struct MagicSearchQuery {
        #[serde(default)]
        skip: usize,
        #[serde(default = "default_limit")]
        limit: usize,
    }

    async fn search_mtg_cards(
        State(state): State<GathersState>,
        Query(query): Query<MagicSearchQuery>,
        Json(input): Json<APICardSearchFilters>,
    ) -> Result<Json<Vec<APICard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_mtg()?;
        crate::check_unique_mode(ret, input.unique.as_deref())?;

        ret.search_cards(input.into(), query.skip.into(), query.limit.into())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to search cards. {e}"),
                    }),
                )
            })
            .map(|result| {
                Json(
                    result
                        .iter()
                        .filter_map(|c| match c {
                            Card::Magic(p) => Some(p.clone().into()),
                            _ => None,
                        })
                        .collect(),
                )
            })
    }

    #[derive(Deserialize, JsonSchema)]
    struct MagicRetrieveQuery {
        #[serde(default)]
        ids: Vec<String>,
    }

    async fn retrieve_cards(
        State(state): State<GathersState>,
        Query(query): Query<MagicRetrieveQuery>,
    ) -> Result<Json<HashMap<String, APICard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_mtg()?;

        ret.get_cards_by_ids(query.ids)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to retrieve cards. {e}"),
                    }),
                )
            })
            .map(|d| {
                d.into_iter()
                    .filter_map(|(k, v)| match v {
                        Card::Magic(m) => Some((k, m.into())),
                        _ => None,
                    })
                    .collect()
            })
            .map(Json)
    }

    async fn random_card(State(state): State<GathersState>) -> Result<Json<APICard>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_mtg()?;

        let card = ret.get_random_card().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to get random card. {e}"),
                }),
            )
        })?;
        match card {
            Some(Card::Magic(m)) => Ok(Json(m.into())),
            _ => Err((
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: "No cards available".to_string(),
                }),
            )),
        }
    }

    async fn get_sets(State(state): State<GathersState>) -> Result<Json<Vec<Set>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_mtg()?;

        ret.get_sets()
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to get sets. {e}"),
                    }),
                )
            })
            .map(Json)
    }

    // Backgrounded so a large/slow AllPrintings.db download isn't cancelled
    // by the server's global 10s request timeout (see the identical
    // comment on pokemon_api's `update`).
    async fn update(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let mtg = {
            let ret = state.0.lock().await;
            ret.require_mtg()?.clone()
        };
        let retrieval = state.0.clone();
        retrieval
            .lock()
            .await
            .downloading
            .insert("Sql".to_string(), Arc::new(Mutex::new(DownloadProgress::default())));
        tokio::spawn(async move {
            let result = mtg.update_backend().await;
            let mut ret = retrieval.lock().await;
            ret.downloading.remove("Sql");
            match result.and_then(|_| ret.reload_mtg()) {
                Ok(()) => info!("MTG DB updated"),
                Err(e) => error!(error = %e, "Failed to update MTG DB"),
            }
        });
        Ok(Json("Update started in background".to_string()))
    }

    // Backgrounded for the same reason as `update` above — a slow prices
    // download shouldn't be cancelled by the global 10s request timeout.
    async fn update_prices(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let mtg = {
            let ret = state.0.lock().await;
            ret.require_mtg()?.clone()
        };
        let retrieval = state.0.clone();
        retrieval.lock().await.downloading.insert(
            "Sql-prices".to_string(),
            Arc::new(Mutex::new(DownloadProgress::default())),
        );
        tokio::spawn(async move {
            let result = mtg.update_prices().await;
            retrieval.lock().await.downloading.remove("Sql-prices");
            match result {
                Ok(true) => info!("MTG prices updated"),
                Ok(false) => info!("No MTG price database configured"),
                Err(e) => error!(error = %e, "Failed to update MTG prices"),
            }
        });
        Ok(Json("Price update started".to_string()))
    }

    #[derive(Deserialize, JsonSchema)]
    struct BulkPricesQuery {
        #[serde(default)]
        ids: Vec<String>,
    }

    async fn bulk_prices(
        State(state): State<GathersState>,
        Query(query): Query<BulkPricesQuery>,
    ) -> Result<Json<HashMap<String, CardPrices>>, ApiError> {
        let guard = state.0.lock().await;
        let mtg = guard.require_mtg()?;
        mtg.get_bulk_card_prices(query.ids)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to retrieve prices. {e}"),
                    }),
                )
            })
            .map(Json)
    }

    ApiRouter::new()
        .api_route("/cards/search", post(search_mtg_cards))
        .api_route("/cards/random", get(random_card))
        .api_route("/cards", get(retrieve_cards))
        .api_route("/sets", get(get_sets))
        .api_route("/update", get(update))
        .api_route("/prices", get(bulk_prices))
        .api_route("/prices/update", get(update_prices))
}
