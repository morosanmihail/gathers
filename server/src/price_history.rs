//! Keeps price history (see `ServerConfig::price_history_enabled`) for cards
//! tracked in collections. Everything here is best effort: failures are
//! logged and never reach the caller, so price history can't get in the way
//! of anything else the server does.

use std::sync::Arc;

use chrono::NaiveDate;
use models::CardPrices;
use persistence::{PersistenceSystem, PersistenceSystemTrait as _, PricePoint};
use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait as _};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{GathersState, RetrievalState, StorageState};

/// Cards looked up per price query, keeping SQL parameter lists small.
const CHUNK_SIZE: usize = 500;

/// Flattens a card's prices into one point per retailer and finish. A
/// retailer's `normal` price is the default (`""`) finish, matching how
/// collections name finishes (see `models::CollectionCard::finish`). Each
/// point is dated by the day its retailer's prices are as of, so a price
/// database that hasn't changed since yesterday re-records yesterday rather
/// than claiming the same prices for today; `fallback_date` is used when the
/// source has no date.
pub fn price_points(prices: &CardPrices, fallback_date: NaiveDate) -> Vec<PricePoint> {
    prices
        .paper
        .iter()
        .flat_map(|(retailer, retail)| {
            [("", retail.normal), ("foil", retail.foil)]
                .into_iter()
                .filter_map(move |(finish, price)| {
                    let recorded_on = retail.date.unwrap_or(fallback_date);
                    Some(PricePoint {
                        card_uuid: prices.uuid.clone(),
                        retailer: retailer.clone(),
                        finish: finish.to_string(),
                        price: price?,
                        currency: retail.currency.clone(),
                        recorded_on,
                    })
                })
        })
        .collect()
}

/// The storage to record into, cloned out of its lock — or `None` when
/// pricing or price history is off.
async fn recording_storage(
    retrieval: &Arc<Mutex<RetrievalState>>,
    storage: &Arc<Mutex<StorageState>>,
) -> Option<PersistenceSystem> {
    if !retrieval.lock().await.pricing_enabled {
        return None;
    }
    let storage = storage.lock().await.storage.clone();
    storage.price_history_enabled().then_some(storage)
}

/// Records the current prices of `uuids` from `system`.
async fn record(system: &RetrievalSystem, storage: &mut PersistenceSystem, uuids: Vec<String>) {
    let provider = system.name();
    let today = chrono::Utc::now().date_naive();
    let mut written = 0;
    for chunk in uuids.chunks(CHUNK_SIZE) {
        let prices = match system.get_bulk_card_prices(chunk.to_vec()).await {
            Ok(prices) => prices,
            Err(e) => {
                warn!(provider, error = %e, "Failed to look up prices for price history");
                return;
            }
        };
        let points: Vec<PricePoint> = prices.values().flat_map(|p| price_points(p, today)).collect();
        if points.is_empty() {
            continue;
        }
        match storage.record_prices(provider, &points).await {
            Ok(n) => written += n,
            Err(e) => {
                warn!(provider, error = %e, "Failed to record price history");
                return;
            }
        }
    }
    if written > 0 {
        info!(provider, cards = uuids.len(), prices = written, "Recorded price history");
    }
}

/// Records the current prices of every card collections track from `system`.
/// With `only_if_empty`, does nothing once anything has been recorded for
/// it — used to seed the history on startup.
pub async fn snapshot_tracked(
    retrieval: &Arc<Mutex<RetrievalState>>,
    storage: &Arc<Mutex<StorageState>>,
    system: &RetrievalSystem,
    only_if_empty: bool,
) {
    let Some(mut storage) = recording_storage(retrieval, storage).await else {
        return;
    };
    let provider = system.name();
    if only_if_empty {
        match storage.has_price_history(provider).await {
            Ok(false) => {}
            Ok(true) => return,
            Err(e) => {
                warn!(provider, error = %e, "Failed to check price history");
                return;
            }
        }
    }
    let uuids = match storage.tracked_card_uuids(provider).await {
        Ok(uuids) => uuids,
        Err(e) => {
            warn!(provider, error = %e, "Failed to list collection cards for price history");
            return;
        }
    };
    if !uuids.is_empty() {
        record(system, &mut storage, uuids).await;
    }
}

/// Seeds price history, in the background, for every active system that
/// has none yet.
pub fn spawn_startup_snapshot(retrieval: Arc<Mutex<RetrievalState>>, storage: Arc<Mutex<StorageState>>) {
    tokio::spawn(async move {
        let systems = {
            let ret = retrieval.lock().await;
            [ret.mtg.clone(), ret.pokemon.clone(), ret.riftbound.clone()]
        };
        for system in systems.into_iter().flatten() {
            snapshot_tracked(&retrieval, &storage, &system, true).await;
        }
    });
}

/// Records, in the background, the prices of every card collections track
/// from `system` — for after its price database was updated.
pub fn spawn_snapshot(retrieval: Arc<Mutex<RetrievalState>>, storage: Arc<Mutex<StorageState>>, system: RetrievalSystem) {
    tokio::spawn(async move {
        snapshot_tracked(&retrieval, &storage, &system, false).await;
    });
}

/// Records, in the background, the current prices of cards just added to a
/// collection under `provider`. Providers with no prices (plugins,
/// Riftbound) are skipped.
pub fn spawn_record_cards(state: &GathersState, provider: String, uuids: Vec<String>) {
    let (retrieval, storage) = state.clone();
    tokio::spawn(async move {
        let Some(mut storage) = recording_storage(&retrieval, &storage).await else {
            return;
        };
        let system = {
            let ret = retrieval.lock().await;
            [ret.mtg.clone(), ret.pokemon.clone(), ret.riftbound.clone()]
                .into_iter()
                .flatten()
                .find(|s| s.name() == provider)
        };
        if let Some(system) = system {
            record(&system, &mut storage, uuids).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::RetailerPrices;
    use std::collections::HashMap;

    #[test]
    fn price_points_use_collection_finish_names() {
        let prices = CardPrices {
            uuid: "c1".to_string(),
            paper: HashMap::from([(
                "tcg".to_string(),
                RetailerPrices { normal: Some(1.0), foil: Some(2.0), currency: "USD".to_string(), date: None },
            ), (
                "raw".to_string(),
                RetailerPrices { normal: Some(3.0), foil: None, currency: "EUR".to_string(), date: "2026-03-01".parse().ok() },
            )]),
        };
        let mut points = price_points(&prices, "2026-03-04".parse().unwrap());
        points.sort_by(|a, b| (&a.retailer, &a.finish).cmp(&(&b.retailer, &b.finish)));
        let summary: Vec<(&str, &str, f64, &str, String)> = points
            .iter()
            .map(|p| (p.retailer.as_str(), p.finish.as_str(), p.price, p.currency.as_str(), p.recorded_on.to_string()))
            .collect();
        // Dated by the source when it says, otherwise by the fallback.
        assert_eq!(
            summary,
            vec![
                ("raw", "", 3.0, "EUR", "2026-03-01".to_string()),
                ("tcg", "", 1.0, "USD", "2026-03-04".to_string()),
                ("tcg", "foil", 2.0, "USD", "2026-03-04".to_string()),
            ]
        );
        assert!(points.iter().all(|p| p.card_uuid == "c1"));
    }
}
