pub mod decks_models;

use aide::axum::{ApiRouter, routing::get, routing::post};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use models::CollectionKind;
use persistence::{PersistenceSystem, PersistenceSystemTrait};

use crate::collections::{card_not_found, resolve_provider};
use crate::{bad_request, demo_err, demo_mode, storage_error};
use crate::{ApiError, ErrorPayload, GathersState};

use decks_models::{
    AvailabilityRequest, CardAvailability, DeckCard, DeckCardAdjustment, DeckCardMove, DeckMeta,
};

/// Confirms `id` names a deck, rejecting a missing collection or a plain one.
///
/// A deck's cards live in `deck_cards`, a collection's in `cards`, so pointing a deck endpoint at
/// a collection would read and write the wrong table.
pub(crate) async fn require_deck(
    storage: &PersistenceSystem,
    id: &String,
) -> Result<(), ApiError> {
    let kind = storage
        .get_collection_kind(id)
        .await
        .map_err(|e| storage_error("Failed to look up collection", e))?;

    match kind {
        Some(CollectionKind::Deck) => Ok(()),
        Some(CollectionKind::Collection) => Err(bad_request(format!(
            "'{id}' is a collection, not a deck"
        ))),
        // Spelled out rather than deferred to `storage_error`, which would call it a missing
        // collection; the caller asked about a deck
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorPayload {
                error: format!("Deck '{id}' not found"),
            }),
        )),
    }
}

/// Whether the provider guard refuses this change.
///
/// Adds only. Storing a card no configured system knows would leave a nameless row, but a card can
/// stop resolving after the fact (a plugin is disabled, a card DB swapped) and removal is then the
/// only way out; `cards_remove` skips the check for the same reason.
fn unresolvable_add(quantity: i32, provider: &str) -> bool {
    quantity > 0 && provider.is_empty()
}

/// Endpoints for a deck's *contents*: its cards and the boards they sit on.
///
/// A deck's *identity* (creating, renaming, deleting, listing) stays on `/api/collection`, since
/// a deck is a collection with `kind = "deck"` and shares that plumbing. Only the card list
/// differs: a board instead of a want quantity, a separate table, no purchase history.
///
/// Paths mirror `/api/collection`'s: `/cards/{id}/<action>`, not `/{id}/cards`.
pub fn deck_routes() -> ApiRouter<GathersState> {
    async fn cards_list(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<Vec<DeckCard>>, ApiError> {
        let storage = &state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let cards = storage
            .deck(&id)
            .cards()
            .await
            .map_err(|e| storage_error("Failed to get deck cards", e))?;

        Ok(Json(cards.into_iter().map(DeckCard::from).collect()))
    }

    /// Copies in the deck, summed across all boards.
    ///
    /// Counts copies where the collection endpoint counts distinct cards: a deck is described by
    /// how many cards are in it.
    async fn cards_count(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<i64>, ApiError> {
        let storage = &state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let count = storage
            .deck(&id)
            .card_count()
            .await
            .map_err(|e| storage_error("Failed to count deck cards", e))?;

        Ok(Json(count))
    }

    /// Changes one card's quantity on one board.
    ///
    /// Never touches any collection: putting a card in a deck doesn't deduct it from what is
    /// owned.
    async fn cards_adjust(
        State(state): State<GathersState>,
        Path(id): Path<String>,
        Json(input): Json<DeckCardAdjustment>,
    ) -> Result<Json<Option<DeckCard>>, ApiError> {
        // Adding is allowed in demo mode and removing isn't, matching `cards_add` and
        // `cards_remove`. This endpoint is one signed change rather than two routes, so the
        // split is on the sign instead of on the path
        if demo_mode() && input.quantity < 0 { return Err(demo_err()); }

        // Resolved before taking the storage lock: `resolve_provider` locks state itself, so doing
        // it while holding the lock would deadlock
        let provider = resolve_provider(&state, &input.id, input.provider.as_deref()).await;
        if unresolvable_add(input.quantity, &provider) {
            return Err(card_not_found(&input.id));
        }
        let now = chrono::Utc::now().to_rfc3339();

        let storage = &mut state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let card = storage
            .deck_mut(&id)
            .adjust_card(
                &input.id,
                &input.finish,
                &input.board,
                input.quantity,
                &now,
                &provider,
            )
            .await
            .map_err(|e| storage_error("Failed to adjust deck card", e))?;

        Ok(Json(card.map(DeckCard::from)))
    }

    /// Moves copies between two of the deck's boards, e.g. mainboard to sideboard.
    ///
    /// Atomic, so copies can't be duplicated or lost midway.
    async fn cards_move(
        State(state): State<GathersState>,
        Path(id): Path<String>,
        Json(input): Json<DeckCardMove>,
    ) -> Result<Json<Vec<DeckCard>>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let storage = &mut state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let mut deck = storage.deck_mut(&id);
        deck.move_card(
            &input.id,
            &input.finish,
            &input.from_board,
            &input.to_board,
            input.quantity,
        )
        .await
        .map_err(|e| storage_error("Failed to move deck card", e))?;

        let cards = deck
            .cards()
            .await
            .map_err(|e| storage_error("Failed to get deck cards", e))?;

        Ok(Json(cards.into_iter().map(DeckCard::from).collect()))
    }

    async fn meta_get(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<DeckMeta>, ApiError> {
        let storage = &state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let meta = storage
            .deck(&id)
            .meta()
            .await
            .map_err(|e| storage_error("Failed to get deck state", e))?;

        Ok(Json(meta.into()))
    }

    /// Records whether the deck is assembled and where it physically is.
    ///
    /// This is what makes its cards count as committed in `/availability`.
    async fn meta_set(
        State(state): State<GathersState>,
        Path(id): Path<String>,
        Json(input): Json<DeckMeta>,
    ) -> Result<Json<DeckMeta>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let storage = &mut state.1.lock().await.storage;
        require_deck(storage, &id).await?;

        let meta = storage
            .deck_mut(&id)
            .set_meta(
                input.built,
                input.location.as_deref(),
                input.notes.as_deref(),
            )
            .await
            .map_err(|e| storage_error("Failed to update deck state", e))?;

        Ok(Json(meta.into()))
    }

    ApiRouter::new()
        .api_route("/cards/{id}/list", get(cards_list))
        .api_route("/cards/{id}/count", get(cards_count))
        .api_route("/cards/{id}/adjust", post(cards_adjust))
        .api_route("/cards/{id}/move", post(cards_move))
        .api_route("/meta/{id}", get(meta_get).put(meta_set))
}

/// Card-scoped queries that span decks and collections.
///
/// Mounted apart from `/api/deck` because the question these answer is about a card ("where
/// are my copies of this?"), even though most of the answer comes from what decks have claimed.
/// A client asking it isn't holding a deck.
pub fn card_routes() -> ApiRouter<GathersState> {
    /// How many copies of each requested card are owned, how many are committed to decks that are
    /// currently built, and which decks those are.
    ///
    /// Takes a list so a client rendering a deck can ask once rather than once per row.
    async fn availability(
        State(state): State<GathersState>,
        Json(input): Json<AvailabilityRequest>,
    ) -> Result<Json<Vec<CardAvailability>>, ApiError> {
        let storage = &state.1.lock().await.storage;
        let availability = storage
            .card_availability(&input.ids)
            .await
            .map_err(|e| storage_error("Failed to compute card availability", e))?;

        Ok(Json(
            availability.into_iter().map(CardAvailability::from).collect(),
        ))
    }

    ApiRouter::new().api_route("/availability", post(availability))
}

#[cfg(test)]
mod deck_guard_tests {
    use super::*;
    use models::CollectionKind;
    use persistence::SQLitePersistenceSystem;

    async fn storage_with(name: &str, kind: CollectionKind) -> PersistenceSystem {
        let mut storage = PersistenceSystem::SQLitePersistenceSystem(
            SQLitePersistenceSystem::new(true, None).unwrap(),
        );
        storage.add_collection(name.to_string(), kind).await.unwrap();
        storage
    }

    #[tokio::test]
    async fn require_deck_accepts_a_deck() {
        let storage = storage_with("Burn", CollectionKind::Deck).await;
        assert!(require_deck(&storage, &"Burn".to_string()).await.is_ok());
    }

    /// Pointing a deck endpoint at a collection would read and write `cards` instead of
    /// `deck_cards`, so it's rejected rather than quietly empty.
    #[tokio::test]
    async fn require_deck_rejects_a_collection() {
        let storage = storage_with("Binder", CollectionKind::Collection).await;

        let (status, body) = require_deck(&storage, &"Binder".to_string())
            .await
            .expect_err("a collection is not a deck");

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("not a deck"), "{}", body.error);
    }

    #[tokio::test]
    async fn require_deck_rejects_an_unknown_name() {
        let storage = storage_with("Burn", CollectionKind::Deck).await;

        let (status, _) = require_deck(&storage, &"nope".to_string())
            .await
            .expect_err("no such deck");

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[cfg(test)]
mod deck_adjust_guard_tests {
    use super::unresolvable_add;

    /// The provider guard exists so an unhydratable card isn't *stored*; it must not also block
    /// getting rid of one.
    #[test]
    fn the_provider_guard_applies_to_adds_only() {
        assert!(unresolvable_add(1, ""), "adding an unresolvable card is refused");
        assert!(!unresolvable_add(-1, ""), "removing one is still allowed");
        assert!(!unresolvable_add(1, "MagicSQLite"), "a resolvable card is fine");
    }
}
