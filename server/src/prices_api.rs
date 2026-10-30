//! The `/prices` and `/prices/update` routes, shared by every system that
//! has a price database (MTG, Pokémon, Riftbound).

use std::{collections::HashMap, sync::Arc};

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
use tokio::sync::Mutex;
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

async fn live_system<S: PricedSystem>(retrieval: &Arc<Mutex<RetrievalState>>) -> Option<RetrievalSystem> {
    S::require(&*retrieval.lock().await).ok().cloned()
}

/// Makes the system currently serving `S` read its price database afresh,
/// and returns it. Reloading a system (after a card database update) replaces
/// it with a new instance holding its own prices, which a price update running
/// on the previous instance would otherwise leave stale.
async fn reset_live_prices<S: PricedSystem>(retrieval: &Arc<Mutex<RetrievalState>>) -> Option<RetrievalSystem> {
    let system = live_system::<S>(retrieval).await?;
    system.reset_prices().await;
    Some(system)
}

/// Updates `S`'s price database. Returns the system now serving the new
/// prices, or `None` if there's no price database to update.
pub async fn update_live_prices<S: PricedSystem>(
    retrieval: &Arc<Mutex<RetrievalState>>,
) -> eyre::Result<Option<RetrievalSystem>> {
    let Some(system) = live_system::<S>(retrieval).await else {
        return Ok(None);
    };
    if !system.update_prices().await? {
        return Ok(None);
    }
    drop(system);
    Ok(reset_live_prices::<S>(retrieval).await)
}

// Backgrounded like the systems' `/update` — a slow prices download
// shouldn't be cancelled by the global 10s request timeout.
async fn update_prices<S: PricedSystem>(State(state): State<GathersState>) -> Result<Json<String>, ApiError> {
    if demo_mode() { return Err(demo_err()); }
    {
        let mut ret = state.0.lock().await;
        S::require(&ret)?;
        ret.start_download(S::DOWNLOAD_KEY)?;
    }
    let (retrieval, storage) = state.clone();
    tokio::spawn(async move {
        let result = update_live_prices::<S>(&retrieval).await;
        retrieval.lock().await.finish_download(S::DOWNLOAD_KEY);
        match result {
            Ok(Some(system)) => {
                info!("{} prices updated", S::LABEL);
                crate::price_history::spawn_snapshot(retrieval, storage, system);
            }
            Ok(None) => info!("No {} price database configured", S::LABEL),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Systems;
    use tempfile::TempDir;

    /// Card in `data/testPrintings.db`.
    const CARD: &str = "0003caab-9ff5-5d1a-bc06-976dd0457f19";

    /// Puts a prices db holding just `CARD` at `price` in place, the way a
    /// download does: built beside `path` and renamed over it.
    fn write_prices(dir: &TempDir, path: &std::path::Path, price: f64) {
        let staging = dir.path().join("staging.sqlite");
        let conn = rusqlite::Connection::open(&staging).unwrap();
        conn.execute_batch(
            "CREATE TABLE prices (uuid TEXT, date TEXT, source TEXT, provider TEXT, priceType TEXT, finish TEXT, price REAL, currency TEXT)",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO prices VALUES (?1, '2026-05-22', 'paper', 'tcgplayer', 'retail', 'normal', ?2, 'USD')",
            rusqlite::params![CARD, price],
        )
        .unwrap();
        drop(conn);
        std::fs::rename(&staging, path).unwrap();
    }

    async fn normal_price(system: &RetrievalSystem) -> Option<f64> {
        system.get_card_prices(CARD).await.unwrap().unwrap().paper["tcgplayer"].normal
    }

    #[tokio::test]
    async fn reset_reaches_a_system_reloaded_during_a_price_update() {
        let dir = TempDir::new().unwrap();
        let cards = dir.path().join("cards.db");
        std::fs::copy("../data/testPrintings.db", &cards).unwrap();
        let prices = dir.path().join("prices.sqlite");
        write_prices(&dir, &prices, 1.0);

        let path = |p: &std::path::Path| Some(p.to_string_lossy().into_owned());
        let state = RetrievalState::new(
            vec![Systems::Sql],
            path(&cards),
            path(&prices),
            None,
            None,
            None,
            None,
            dir.path().join("server.toml"),
            true,
            true,
            "USD".to_string(),
        )
        .unwrap();
        let retrieval = Arc::new(Mutex::new(state));

        // A card update's reload swaps in a new instance, which loads the
        // prices that are on disk before the new ones arrive.
        let previous = live_system::<Mtg>(&retrieval).await.unwrap();
        retrieval.lock().await.reload_mtg().unwrap();
        write_prices(&dir, &prices, 2.0);
        previous.reset_prices().await;

        let live = live_system::<Mtg>(&retrieval).await.unwrap();
        assert_eq!(normal_price(&live).await, Some(1.0));

        let returned = reset_live_prices::<Mtg>(&retrieval).await.unwrap();
        assert_eq!(normal_price(&live).await, Some(2.0));
        assert_eq!(normal_price(&returned).await, Some(2.0));
    }
}
