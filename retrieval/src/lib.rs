mod http;
pub mod mirror;
mod systems;

use std::collections::HashMap;

use enum_dispatch::enum_dispatch;
use models::{CardID, CardPrices, CollectorNumber, SetCode, filters::UniqueMode};
pub use http::DownloadProgress;
pub use systems::plugin::{
    PluginCard, PluginInfo, PluginRetrievalSystem, PluginSearchFilters, PluginSearchRequest,
    PluginUpdateResponse,
};
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

/// Picks the `UniqueMode` a search should run with: the one named by
/// `requested` (`None` or empty means the system default, its first mode).
/// `Ok(None)` when the system offers no modes at all, in which case
/// `requested` is ignored; an id the system doesn't offer is an error naming
/// the ones it does.
pub fn resolve_unique_mode<'a>(
    modes: &'a [UniqueMode],
    requested: Option<&str>,
) -> eyre::Result<Option<&'a UniqueMode>> {
    let Some(default) = modes.first() else {
        return Ok(None);
    };
    match requested.map(str::trim).filter(|r| !r.is_empty()) {
        None => Ok(Some(default)),
        Some(id) => modes.iter().find(|m| m.id == id).map(Some).ok_or_else(|| {
            eyre::eyre!(
                "Unsupported unique mode '{id}' (supported: {})",
                modes.iter().map(|m| m.id.as_str()).collect::<Vec<_>>().join(", ")
            )
        }),
    }
}

#[enum_dispatch(RetrievalSystem)]
#[allow(async_fn_in_trait)]
pub trait RetrievalSystemTrait {
    /// The ways this system can collapse results that share a card, selected
    /// with `CardSearchFilters::unique`. The first is the default. Empty
    /// (the default) means the system has one fixed behaviour and ignores
    /// `unique`, so there is nothing for a client to toggle.
    fn unique_modes(&self) -> Vec<UniqueMode> {
        Vec::new()
    }

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

    /// Returns a single uniformly-random card from this system's catalog, or
    /// `None` if the catalog is empty.
    async fn get_random_card(&self) -> eyre::Result<Option<models::Card>>;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn modes() -> Vec<UniqueMode> {
        vec![
            UniqueMode::new("prints", "Prints", ""),
            UniqueMode::new("cards", "Cards", ""),
        ]
    }

    #[test]
    fn test_resolve_unique_mode_defaults_to_the_first_mode() {
        let modes = modes();
        for requested in [None, Some(""), Some("  ")] {
            let mode = resolve_unique_mode(&modes, requested).unwrap().unwrap();
            assert_eq!(mode.id, "prints");
        }
    }

    #[test]
    fn test_resolve_unique_mode_finds_a_requested_mode() {
        let modes = modes();
        let mode = resolve_unique_mode(&modes, Some("cards")).unwrap().unwrap();
        assert_eq!(mode.id, "cards");
    }

    #[test]
    fn test_resolve_unique_mode_rejects_an_unknown_mode_naming_the_valid_ones() {
        let err = resolve_unique_mode(&modes(), Some("art")).unwrap_err().to_string();
        assert!(err.contains("'art'") && err.contains("prints, cards"), "{err}");
    }

    #[test]
    fn test_resolve_unique_mode_ignores_the_request_when_a_system_has_no_modes() {
        assert!(resolve_unique_mode(&[], Some("cards")).unwrap().is_none());
        assert!(resolve_unique_mode(&[], None).unwrap().is_none());
    }
}
