pub mod decks_models;

use aide::axum::{ApiRouter, routing::get, routing::post};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use models::CollectionKind;
use retrieval::RetrievalSystemTrait;
use persistence::{PersistenceSystem, PersistenceSystemTrait};

use std::collections::HashMap;

use crate::collections::{
    card_id, card_name, card_not_found, clone_retrieval_systems_by_name, resolve_provider,
};
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

/// Whether two printings of the same card are interchangeable for this provider: whether owning
/// the Revised printing satisfies a deck that lists the M13 one.
///
/// True for Magic, where the rules treat cards sharing a name as the same card whatever the set.
/// Deliberately **not** true by default, because it isn't a universal property of card games: two
/// Pokemon cards can share a name across sets and have different HP and attacks, which makes them
/// different cards that happen to be called the same thing, and a plugin's domain is unknown to us
/// entirely. Anything not named here keeps its printings distinct, so a game added later doesn't
/// silently start merging cards that aren't the same.
fn printings_interchangeable(provider: &str) -> bool {
    matches!(provider, "MagicSQLite" | "Scryfall")
}

/// Whether resolving many ids at once is cheap enough for this provider to do on every availability
/// request.
///
/// Grouping printings needs card *names*, and the only way to get them is to look up the ids in
/// play. The SQLite backends answer that with one query. Scryfall issues one HTTPS request per
/// id (see `ScryfallRetrievalSystem::get_cards_by_ids`), so asking it about a whole collection
/// would mean thousands of serial round trips, and rate limiting, every time a deck page loads.
/// Cross-printing matching is switched off there rather than made unusably slow; availability still
/// answers, just per printing.
///
/// Separate from `printings_interchangeable` on purpose: that one is about the game's rules, this
/// one is about what the backend costs.
fn bulk_lookup_is_cheap(provider: &str) -> bool {
    !matches!(provider, "Scryfall")
}

/// Most bulk id lookups become one `uuid IN (?, ?, …)` query, and SQLite caps bound parameters
/// (32766 in recent builds). Chunk well under it so a large collection doesn't fail the whole
/// lookup.
const ID_LOOKUP_CHUNK: usize = 900;

/// Maps every id in play to the set of ids that mean the same card as it.
///
/// For a provider whose printings are interchangeable this is "every id sharing this card's name";
/// for every other provider an id is only itself. Names come from `retrieval` rather than storage,
/// which is why this lives in the server layer: `persistence` has ids and quantities but no idea
/// what a card is called.
async fn printing_groups(
    state: &GathersState,
    in_use: Vec<(String, String)>,
) -> HashMap<String, Vec<String>> {
    let mut by_provider: HashMap<String, Vec<String>> = HashMap::new();
    for (uuid, provider) in in_use {
        by_provider.entry(provider).or_default().push(uuid);
    }

    let systems = clone_retrieval_systems_by_name(state).await;
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for (provider, uuids) in by_provider {
        // Every printing stands alone, and no name lookup happens at all, when the game doesn't
        // treat same-named cards as the same card (Pokemon, Riftbound, any plugin) or when asking
        // the backend would be too expensive to do per request (Scryfall).
        let groupable = printings_interchangeable(&provider) && bulk_lookup_is_cheap(&provider);
        let Some(system) = systems.get(&provider).filter(|_| groupable) else {
            for uuid in uuids {
                groups.insert(uuid.clone(), vec![uuid]);
            }
            continue;
        };

        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
        let mut named: HashMap<String, String> = HashMap::new();
        for chunk in uuids.chunks(ID_LOOKUP_CHUNK) {
            let cards = match system.get_cards_by_ids(chunk.to_vec()).await {
                Ok(cards) => cards,
                // Don't swallow this: losing the names silently turns cross-printing matching off
                // and reports cards as missing that the owner has under another printing.
                Err(e) => {
                    tracing::warn!(
                        provider = %provider,
                        error = %e,
                        "Failed to resolve card names; availability falls back to per-printing matching"
                    );
                    continue;
                }
            };
            for card in cards.values() {
                let id = card_id(card).to_string();
                let name = card_name(card).to_string();
                by_name.entry(name.clone()).or_default().push(id.clone());
                named.insert(id, name);
            }
        }

        for uuid in uuids {
            match named.get(&uuid).and_then(|n| by_name.get(n)) {
                Some(siblings) => groups.insert(uuid, siblings.clone()),
                // A card the retrieval system no longer knows about (the DB was swapped, say) can't
                // be grouped, so it stands alone rather than being silently dropped from
                // availability.
                None => groups.insert(uuid.clone(), vec![uuid]),
            };
        }
    }

    groups
}

/// Folds every printing of one card into a single answer.
///
/// Quantities sum, and holdings collapse by `(deck, location, board, finish)` so a deck holding two
/// M13 copies and two Revised copies of the same card on the same board reads as one "4 in Burn"
/// rather than two "2 in Burn" entries a reader would have to add up. Finish is deliberately *not*
/// collapsed: a foil copy is a different physical object from a nonfoil one even when the printings
/// are interchangeable, and that distinction is worth surfacing.
fn merge_printings(
    id: &str,
    siblings: &[String],
    by_uuid: &HashMap<&str, &models::CardAvailability>,
) -> CardAvailability {
    let mut owned = 0;
    let mut committed = 0;
    let mut holdings: Vec<models::CardHolding> = vec![];

    for sibling in siblings {
        let Some(row) = by_uuid.get(sibling.as_str()) else { continue };
        owned += row.owned;
        committed += row.committed;
        holdings.extend(row.holdings.iter().cloned());
    }

    // Sorting first puts equal keys next to each other, so the merge is one pass over adjacent
    // pairs rather than a scan of everything collected so far per holding. The sort has to happen
    // regardless, since callers want a stable order, so collapsing costs nothing on top of it.
    //
    // The sort key is every field the merge compares, or duplicates wouldn't reliably land next to
    // each other.
    holdings.sort_by(|a, b| {
        (&a.deck, &a.location, &a.board, &a.finish)
            .cmp(&(&b.deck, &b.location, &b.board, &b.finish))
    });
    holdings.dedup_by(|dropped, kept| {
        if kept.deck == dropped.deck
            && kept.location == dropped.location
            && kept.board == dropped.board
            && kept.finish == dropped.finish
        {
            kept.quantity += dropped.quantity;
            true
        } else {
            false
        }
    });

    models::CardAvailability {
        uuid: id.to_string(),
        owned,
        committed,
        free: owned - committed,
        over_committed: committed > owned,
        holdings,
    }
    .into()
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
    /// Counts across printings where the game allows it (see `printings_interchangeable`): a
    /// deck listing the M13 Lightning Bolt is satisfied by a Revised one in the binder, and
    /// reporting that card as missing would be the wrong answer.
    ///
    /// Takes a list so a client rendering a deck can ask once rather than once per row.
    async fn availability(
        State(state): State<GathersState>,
        Json(input): Json<AvailabilityRequest>,
    ) -> Result<Json<Vec<CardAvailability>>, ApiError> {
        let in_use = {
            let storage = &state.1.lock().await.storage;
            storage
                .uuids_in_use()
                .await
                .map_err(|e| storage_error("Failed to list stored cards", e))?
        };

        // Resolved outside the storage lock: this reaches into retrieval, which takes the state
        // lock itself.
        let groups = printing_groups(&state, in_use).await;

        // One storage call for every printing of every requested card, rather than one per card.
        let expanded: Vec<String> = input
            .ids
            .iter()
            .flat_map(|id| groups.get(id).cloned().unwrap_or_else(|| vec![id.clone()]))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let rows = {
            let storage = &state.1.lock().await.storage;
            storage
                .card_availability(&expanded)
                .await
                .map_err(|e| storage_error("Failed to compute card availability", e))?
        };

        let by_uuid: HashMap<&str, &models::CardAvailability> =
            rows.iter().map(|r| (r.uuid.as_str(), r)).collect();

        Ok(Json(
            input
                .ids
                .iter()
                .map(|id| {
                    let siblings = groups
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| vec![id.clone()]);
                    merge_printings(id, &siblings, &by_uuid)
                })
                .collect(),
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

#[cfg(test)]
mod printing_merge_tests {
    use super::*;

    fn holding(deck: &str, board: &str, finish: &str, quantity: i32) -> models::CardHolding {
        models::CardHolding {
            deck: deck.to_string(),
            location: Some("red deck box".to_string()),
            board: board.to_string(),
            finish: finish.to_string(),
            quantity,
        }
    }

    fn row(uuid: &str, owned: i32, holdings: Vec<models::CardHolding>) -> models::CardAvailability {
        let committed = holdings.iter().map(|h| h.quantity).sum();
        models::CardAvailability {
            uuid: uuid.to_string(),
            owned,
            committed,
            free: owned - committed,
            over_committed: committed > owned,
            holdings,
        }
    }

    fn merge(id: &str, siblings: &[&str], rows: &[models::CardAvailability]) -> CardAvailability {
        let by_uuid: HashMap<&str, &models::CardAvailability> =
            rows.iter().map(|r| (r.uuid.as_str(), r)).collect();
        let siblings: Vec<String> = siblings.iter().map(|s| s.to_string()).collect();
        merge_printings(id, &siblings, &by_uuid)
    }

    /// Magic treats cards sharing a name as the same card, so owning the Revised printing has to
    /// satisfy a deck that lists the M13 one. That is the case that made the badge report a
    /// shortfall that wasn't real.
    #[test]
    fn owning_another_printing_satisfies_the_deck() {
        let rows = vec![
            row("m13", 0, vec![holding("Burn", "main", "", 4)]),
            row("revised", 4, vec![]),
        ];

        let a = merge("m13", &["m13", "revised"], &rows);

        assert_eq!(a.owned, 4, "copies of either printing count as owned");
        assert_eq!(a.committed, 4);
        assert_eq!(a.free, 0);
        assert!(!a.over_committed, "no shortfall when another printing covers it");
    }

    /// Mixed printings on one board are one holding, not two entries a reader has to add up
    /// themselves.
    #[test]
    fn mixed_printings_in_one_deck_collapse_to_one_holding() {
        let rows = vec![
            row("m13", 2, vec![holding("Burn", "main", "", 2)]),
            row("revised", 2, vec![holding("Burn", "main", "", 2)]),
        ];

        let a = merge("m13", &["m13", "revised"], &rows);

        assert_eq!(a.owned, 4);
        assert_eq!(a.holdings.len(), 1, "one deck, one board, one entry");
        assert_eq!(a.holdings[0].quantity, 4, "quantities add up");
        assert_eq!(a.holdings[0].deck, "Burn");
    }

    /// Finish is a real physical distinction even between interchangeable printings, so it stays
    /// split.
    #[test]
    fn finishes_stay_separate_when_printings_merge() {
        let rows = vec![
            row("m13", 1, vec![holding("Burn", "main", "", 1)]),
            row("revised", 1, vec![holding("Burn", "main", "foil", 1)]),
        ];

        let a = merge("m13", &["m13", "revised"], &rows);

        assert_eq!(a.holdings.len(), 2, "nonfoil and foil are different objects");
        assert_eq!(a.holdings.iter().map(|h| h.quantity).sum::<i32>(), 2);
    }

    /// Different boards of the same deck stay separate too: knowing a copy is in the sideboard
    /// rather than the mainboard is the point.
    #[test]
    fn boards_stay_separate() {
        let rows = vec![row(
            "m13",
            4,
            vec![holding("Burn", "main", "", 3), holding("Burn", "side", "", 1)],
        )];

        let a = merge("m13", &["m13"], &rows);

        assert_eq!(a.holdings.len(), 2);
    }

    /// A card with no sibling printings behaves exactly as before.
    #[test]
    fn a_lone_printing_is_unchanged() {
        let rows = vec![row("pikachu-set-a", 2, vec![holding("Deck", "", "", 3)])];

        let a = merge("pikachu-set-a", &["pikachu-set-a"], &rows);

        assert_eq!(a.owned, 2);
        assert_eq!(a.committed, 3);
        assert_eq!(a.free, -1);
        assert!(a.over_committed, "a genuine shortfall is still reported");
    }

    /// Three printings, not two. The merge walks sorted neighbours, so a sort key that didn't put
    /// every duplicate adjacent would leave some of them unmerged. With only two printings
    /// they're adjacent either way, so two can't catch it.
    #[test]
    fn three_printings_collapse_into_one_holding() {
        let rows = vec![
            row("m13", 1, vec![holding("Burn", "main", "", 1)]),
            row("revised", 1, vec![holding("Burn", "main", "", 2)]),
            row("m20", 1, vec![holding("Burn", "main", "", 3)]),
        ];

        let a = merge("m13", &["m13", "revised", "m20"], &rows);

        assert_eq!(a.holdings.len(), 1, "one deck, one board, one entry");
        assert_eq!(a.holdings[0].quantity, 6, "all three printings add up");
        assert_eq!(a.owned, 3);
    }

    /// Holdings come back in the same order whichever order the printings are visited in, so the
    /// badges don't reshuffle between requests.
    #[test]
    fn holdings_order_is_independent_of_sibling_order() {
        let rows = vec![
            row("a", 1, vec![holding("Storm", "side", "", 1)]),
            row("b", 1, vec![holding("Burn", "main", "foil", 1)]),
            row("c", 1, vec![holding("Burn", "main", "", 1)]),
        ];

        let labels = |x: &CardAvailability| {
            x.holdings
                .iter()
                .map(|h| format!("{}/{}/{}", h.deck, h.board, h.finish))
                .collect::<Vec<_>>()
        };

        let forward = merge("a", &["a", "b", "c"], &rows);
        let reverse = merge("a", &["c", "b", "a"], &rows);

        assert_eq!(labels(&forward), labels(&reverse));
        assert_eq!(
            labels(&forward),
            vec!["Burn/main/", "Burn/main/foil", "Storm/side/"]
        );
    }

    #[test]
    fn a_card_with_no_rows_at_all_is_zero() {
        let a = merge("unknown", &["unknown"], &[]);
        assert_eq!((a.owned, a.committed, a.free), (0, 0, 0));
        assert!(!a.over_committed);
    }
}

#[cfg(test)]
mod printing_interchange_tests {
    use super::printings_interchangeable;

    /// Magic's rules treat cards sharing a name as the same card whatever the set, so any printing
    /// satisfies a deck listing any other.
    #[test]
    fn magic_printings_are_interchangeable() {
        assert!(printings_interchangeable("MagicSQLite"));
        assert!(printings_interchangeable("Scryfall"));
    }

    /// Not a universal property of card games.
    ///
    /// Two Pokemon cards can share a name across sets and have different HP and attacks: they're
    /// different cards that happen to be called the same thing, so merging them would claim
    /// ownership of something that isn't owned.
    #[test]
    fn other_games_keep_their_printings_distinct() {
        assert!(!printings_interchangeable("PokemonSQLite"));
        assert!(!printings_interchangeable("RiftboundSQLite"));
    }

    /// A plugin's domain is unknown to us, and so is any game added later: both default to distinct
    /// rather than silently merging.
    #[test]
    fn unknown_providers_default_to_distinct() {
        assert!(!printings_interchangeable("plugin-books"));
        assert!(!printings_interchangeable("SomeFutureGame"));
        assert!(!printings_interchangeable(""));
    }
}
