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

use models::CardPrices;

use crate::{
    ApiError, ErrorPayload, GathersState, collections::collections_models::APICardSearchFilters,
    pokemon_api::pokemon_api_models::APIPokemonCard,
};
pub mod pokemon_api_models;

fn default_limit() -> usize {
    24
}

pub fn pokemon_routes() -> ApiRouter<GathersState> {
    #[derive(Deserialize, JsonSchema)]
    struct PokemonSearchQuery {
        #[serde(default)]
        skip: usize,
        #[serde(default = "default_limit")]
        limit: usize,
    }

    async fn search_pokemon_cards(
        State(state): State<GathersState>,
        Query(query): Query<PokemonSearchQuery>,
        Json(input): Json<APICardSearchFilters>,
    ) -> Result<Json<Vec<APIPokemonCard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_pokemon()?;

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
                            Card::Pokemon(p) => Some(p.clone().into()),
                            _ => None,
                        })
                        .collect(),
                )
            })
    }

    #[derive(Deserialize, JsonSchema)]
    struct PokemonRetrieveQuery {
        #[serde(default)]
        ids: Vec<String>,
    }

    async fn retrieve_pokemon_cards(
        State(state): State<GathersState>,
        Query(query): Query<PokemonRetrieveQuery>,
    ) -> Result<Json<HashMap<String, APIPokemonCard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_pokemon()?;

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
                        Card::Pokemon(p) => Some((k, p.into())),
                        _ => None,
                    })
                    .collect()
            })
            .map(Json)
    }

    async fn get_sets(State(state): State<GathersState>) -> Result<Json<Vec<String>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_pokemon()?;

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
        let mut ret = state.0.lock().await;
        let result = {
            let pokemon = ret.require_pokemon()?;
            pokemon.update_backend().await
        };
        match result.and_then(|_| ret.reload_pokemon()) {
            Ok(()) => Ok(Json("Update successful".to_string())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to update Pokemon DB. {e}"),
                }),
            )),
        }
    }

    async fn update_prices(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        let guard = state.0.lock().await;
        let pokemon = guard.require_pokemon()?;
        match pokemon.update_prices().await {
            Ok(true) => Ok(Json("Price update started".to_string())),
            Ok(false) => Ok(Json("No price database configured".to_string())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to update prices. {e}"),
                }),
            )),
        }
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
        let pokemon = guard.require_pokemon()?;
        pokemon
            .get_bulk_card_prices(query.ids)
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
        .api_route("/cards/search", post(search_pokemon_cards))
        .api_route("/cards", get(retrieve_pokemon_cards))
        .api_route("/sets", get(get_sets))
        .api_route("/update", get(update))
        .api_route("/prices", get(bulk_prices))
        .api_route("/prices/update", get(update_prices))
}
