use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::http::StatusCode;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use models::{Card, Set};
use retrieval::RetrievalSystemTrait;
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::{error, info};

use models::CardPrices;

use crate::{
    ApiError, ErrorPayload, GathersState, demo_mode, demo_err,
    collections::collections_models::APICardSearchFilters,
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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_pokemon()?.clone();
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
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_pokemon()?.clone();

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

    async fn random_card(
        State(state): State<GathersState>,
    ) -> Result<Json<APIPokemonCard>, ApiError> {
        // Cloned out so the shared state isn't locked for the whole query.
        let ret = state.0.lock().await.require_pokemon()?.clone();

        let card = ret.get_random_card().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to get random card. {e}"),
                }),
            )
        })?;
        match card {
            Some(Card::Pokemon(p)) => Ok(Json(p.into())),
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
        let ret = state.0.lock().await.require_pokemon()?.clone();

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

    // Runs in the background instead of blocking on `update_backend()`
    // inline: the Pokemon scraper walks every set sequentially (with
    // deliberate rate-limit sleeps) and routinely takes far longer than the
    // server's global 10s request timeout, which would otherwise cancel
    // the scrape after just a few sets — every single call — leaving a
    // near-empty db that still reports "Update successful".
    async fn update(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let pokemon = {
            let mut ret = state.0.lock().await;
            let system = ret.require_pokemon()?.clone();
            ret.start_download("PokemonSql")?;
            system
        };
        let retrieval = state.0.clone();
        tokio::spawn(async move {
            let result = pokemon.update_backend().await;
            let mut ret = retrieval.lock().await;
            ret.finish_download("PokemonSql");
            match result.and_then(|_| ret.reload_pokemon()) {
                Ok(()) => info!("Pokemon DB updated"),
                Err(e) => error!(error = %e, "Failed to update Pokemon DB"),
            }
        });
        Ok(Json("Update started in background".to_string()))
    }

    // Backgrounded for the same reason as `update` above — a slow prices
    // download shouldn't be cancelled by the global 10s request timeout.
    async fn update_prices(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let pokemon = {
            let mut ret = state.0.lock().await;
            let system = ret.require_pokemon()?.clone();
            ret.start_download("PokemonSql-prices")?;
            system
        };
        let retrieval = state.0.clone();
        tokio::spawn(async move {
            let result = pokemon.update_prices().await;
            retrieval.lock().await.finish_download("PokemonSql-prices");
            match result {
                Ok(true) => info!("Pokemon prices updated"),
                Ok(false) => info!("No Pokemon price database configured"),
                Err(e) => error!(error = %e, "Failed to update Pokemon prices"),
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
        let pokemon = state.0.lock().await.require_pokemon()?.clone();
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
        .api_route("/cards/random", get(random_card))
        .api_route("/cards", get(retrieve_pokemon_cards))
        .api_route("/sets", get(get_sets))
        .api_route("/update", post(update))
        .api_route("/prices", get(bulk_prices))
        .api_route("/prices/update", post(update_prices))
}
