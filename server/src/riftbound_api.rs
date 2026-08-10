use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::http::StatusCode;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use models::Card;
use retrieval::RetrievalSystemTrait;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    ApiError, ErrorPayload, GathersState, demo_mode, demo_err,
    collections::collections_models::APICardSearchFilters,
    riftbound_api::riftbound_api_models::APIRiftboundCard,
};
pub mod riftbound_api_models;

fn default_limit() -> usize {
    10
}

pub fn riftbound_routes() -> ApiRouter<GathersState> {
    #[derive(Deserialize, JsonSchema)]
    struct RiftboundSearchQuery {
        #[serde(default)]
        skip: usize,
        #[serde(default = "default_limit")]
        limit: usize,
    }

    async fn search_riftbound_cards(
        State(state): State<GathersState>,
        Query(query): Query<RiftboundSearchQuery>,
        Json(input): Json<APICardSearchFilters>,
    ) -> Result<Json<Vec<APIRiftboundCard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_riftbound()?;

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
                            Card::Riftbound(rb) => Some(rb.clone().into()),
                            _ => None,
                        })
                        .collect(),
                )
            })
    }

    #[derive(Deserialize, JsonSchema)]
    struct RiftboundRetrieveQuery {
        #[serde(default)]
        ids: Vec<String>,
    }

    async fn retrieve_riftbound_cards(
        State(state): State<GathersState>,
        Query(query): Query<RiftboundRetrieveQuery>,
    ) -> Result<Json<HashMap<String, APIRiftboundCard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_riftbound()?;

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
                        Card::Riftbound(rb) => Some((k, rb.into())),
                        _ => None,
                    })
                    .collect()
            })
            .map(Json)
    }

    async fn random_card(
        State(state): State<GathersState>,
    ) -> Result<Json<APIRiftboundCard>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_riftbound()?;

        let card = ret.get_random_card().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to get random card. {e}"),
                }),
            )
        })?;
        match card {
            Some(Card::Riftbound(rb)) => Ok(Json(rb.into())),
            _ => Err((
                StatusCode::NOT_FOUND,
                Json(ErrorPayload {
                    error: "No cards available".to_string(),
                }),
            )),
        }
    }

    async fn get_sets(State(state): State<GathersState>) -> Result<Json<Vec<String>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_riftbound()?;

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
            .map(|s| Json(s.iter().map(|s| s.code.clone()).collect()))
    }

    async fn update(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let mut ret = state.0.lock().await;
        let result = {
            let riftbound = ret.require_riftbound()?;
            riftbound.update_backend().await
        };
        match result.and_then(|_| ret.reload_riftbound()) {
            Ok(()) => Ok(Json("Update successful".to_string())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Oof. {e}"),
                }),
            )),
        }
    }

    ApiRouter::new()
        .api_route("/cards/search", post(search_riftbound_cards))
        .api_route("/cards/random", get(random_card))
        .api_route("/cards", get(retrieve_riftbound_cards))
        .api_route("/sets", get(get_sets))
        .api_route("/update", get(update))
}
