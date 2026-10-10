//! The `/prices` and `/prices/update` routes, shared by every system that
//! has a price database (MTG, Pokémon, Riftbound).

use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{get, post},
};
use axum::http::StatusCode;
use axum::{Json, extract::State};
use axum_extra::extract::Query;
use models::CardPrices;
use retrieval::{RetrievalSystem, RetrievalSystemTrait};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::{error, info};

use crate::{ApiError, ErrorPayload, GathersState, RetrievalState, demo_err, demo_mode};

/// A system with a price database, as its routes see it.
pub trait PricedSystem: Send + Sync + 'static {
    /// Name used in log messages, e.g. "MTG".
    const LABEL: &'static str;
    /// Key a running price update is tracked under (see
    /// `RetrievalState::start_download`), e.g. `Sql-prices`.
    const DOWNLOAD_KEY: &'static str;
    fn require(state: &RetrievalState) -> Result<&RetrievalSystem, ApiError>;
}

pub struct Mtg;
impl PricedSystem for Mtg {
    const LABEL: &'static str = "MTG";
    const DOWNLOAD_KEY: &'static str = "Sql-prices";
    fn require(state: &RetrievalState) -> Result<&RetrievalSystem, ApiError> {
        state.require_mtg()
    }
}

pub struct Pokemon;
impl PricedSystem for Pokemon {
    const LABEL: &'static str = "Pokemon";
    const DOWNLOAD_KEY: &'static str = "PokemonSql-prices";
    fn require(state: &RetrievalState) -> Result<&RetrievalSystem, ApiError> {
        state.require_pokemon()
    }
}

pub struct Riftbound;
impl PricedSystem for Riftbound {
    const LABEL: &'static str = "Riftbound";
    const DOWNLOAD_KEY: &'static str = "RiftboundSql-prices";
    fn require(state: &RetrievalState) -> Result<&RetrievalSystem, ApiError> {
        state.require_riftbound()
    }
}

// Backgrounded like the systems' `/update` — a slow prices download
// shouldn't be cancelled by the global 10s request timeout.
async fn update_prices<S: PricedSystem>(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
    if demo_mode() { return Err(demo_err()); }
    let system = {
        let mut ret = state.0.lock().await;
        let system = S::require(&ret)?.clone();
        ret.start_download(S::DOWNLOAD_KEY)?;
        system
    };
    let (retrieval, storage) = state.clone();
    tokio::spawn(async move {
        let result = system.update_prices().await;
        retrieval.lock().await.finish_download(S::DOWNLOAD_KEY);
        match result {
            Ok(true) => {
                info!("{} prices updated", S::LABEL);
                crate::price_history::spawn_snapshot(retrieval, storage, system);
            }
            Ok(false) => info!("No {} price database configured", S::LABEL),
            Err(e) => error!(error = %e, "Failed to update {} prices", S::LABEL),
        }
    });
    Ok(Json("Price update started".to_string()))
}

#[derive(Deserialize, JsonSchema)]
struct BulkPricesQuery {
    #[serde(default)]
    ids: Vec<String>,
}

async fn bulk_prices<S: PricedSystem>(
    State(state): State<GathersState>,
    Query(query): Query<BulkPricesQuery>,
) -> Result<Json<HashMap<String, CardPrices>>, ApiError> {
    // Cloned out so the shared state isn't locked for the whole query.
    let system = S::require(&*state.0.lock().await)?.clone();
    system
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

/// `/prices` (current prices of the cards in `ids`) and `/prices/update`,
/// for merging into system `S`'s router.
pub fn price_routes<S: PricedSystem>() -> ApiRouter<GathersState> {
    ApiRouter::new()
        .api_route("/prices", get(bulk_prices::<S>))
        .api_route("/prices/update", post(update_prices::<S>))
}
