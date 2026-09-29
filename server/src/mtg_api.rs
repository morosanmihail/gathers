use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::http::StatusCode;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use models::{Card, CardPrices, Set};
use retrieval::RetrievalSystemTrait as _;
use schemars::JsonSchema;
use serde::Deserialize;
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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_mtg()?.clone();
        crate::check_unique_mode(&ret, input.unique.as_deref())?;

        ret.search_cards(input.into(), query.skip.into(), query.limit.min(crate::MAX_PAGE_SIZE).into())
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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_mtg()?.clone();

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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_mtg()?.clone();

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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_mtg()?.clone();

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
            let mut ret = state.0.lock().await;
            let system = ret.require_mtg()?.clone();
            ret.start_download("Sql")?;
            system
        };
        let retrieval = state.0.clone();
        tokio::spawn(async move {
            let result = mtg.update_backend().await;
            let mut ret = retrieval.lock().await;
            ret.finish_download("Sql");
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
            let mut ret = state.0.lock().await;
            let system = ret.require_mtg()?.clone();
            ret.start_download("Sql-prices")?;
            system
        };
        let retrieval = state.0.clone();
        tokio::spawn(async move {
            let result = mtg.update_prices().await;
            retrieval.lock().await.finish_download("Sql-prices");
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
        // Cloned out so the shared state isn't locked for the whole query.
        let mtg = state.0.lock().await.require_mtg()?.clone();
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
        .api_route("/update", post(update))
        .api_route("/prices", get(bulk_prices))
        .api_route("/prices/update", post(update_prices))
}
