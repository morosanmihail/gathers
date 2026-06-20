mod http;
pub mod mirror;
mod systems;

use std::collections::HashMap;

use enum_dispatch::enum_dispatch;
use models::{CardID, CardPrices, CollectorNumber, SetCode};
pub use http::DownloadProgress;
pub use systems::pokemon::{PokemonSQLiteRetrievalSystem, download_pokemon_prices};
pub use systems::riftsqlite::RiftboundSQLiteRetrievalSystem;
pub use systems::scryfall::ScryfallRetrievalSystem;
pub use systems::sqlite::{MagicSQLiteRetrievalSystem, download_mtg_db, download_prices};

#[enum_dispatch]
#[derive(Debug, Clone)]
pub enum RetrievalSystem {
    ScryfallRetrievalSystem,
    MagicSQLiteRetrievalSystem,
    RiftboundSQLiteRetrievalSystem,
    PokemonSQLiteRetrievalSystem,
}

#[enum_dispatch(RetrievalSystem)]
#[allow(async_fn_in_trait)]
pub trait RetrievalSystemTrait {
    async fn search_cards(
        &self,
        filters: models::filters::CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<models::Card>>;

    async fn get_cards_by_ids(
        &self,
        ids: Vec<String>,
    ) -> eyre::Result<HashMap<String, models::Card>>;

    async fn get_sets(&self) -> eyre::Result<Vec<models::Set>>;
    async fn bulk_search_cards(
        &self,
        cards: Vec<(SetCode, CollectorNumber)>,
    ) -> eyre::Result<Vec<(SetCode, CollectorNumber, CardID)>>;
    async fn update_backend(&self) -> eyre::Result<bool>;

    async fn get_card_prices(&self, _uuid: &str) -> eyre::Result<Option<CardPrices>> {
        Ok(None)
    }

    async fn get_bulk_card_prices(
        &self,
        _uuids: Vec<String>,
    ) -> eyre::Result<HashMap<String, CardPrices>> {
        Ok(HashMap::new())
    }

    async fn update_prices(&self) -> eyre::Result<bool> {
        Ok(false)
    }
}

#[enum_dispatch(RetrievalSystem)]
pub trait NamedRetrievalSystem {
    fn name(&self) -> &str;
}
