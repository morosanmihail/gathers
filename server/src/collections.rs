use std::collections::HashMap;

use aide::axum::{
    ApiRouter,
    routing::{delete, get, post},
};
use axum::extract::Multipart;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use models::Card;
use persistence::{CollectionCardsParams, PersistenceSystem, PersistenceSystemTrait, UpdateEntryResult};
use retrieval::{NamedRetrievalSystem as _, RetrievalSystem, RetrievalSystemTrait};

use crate::{
    ApiError, ErrorPayload, GathersState, demo_mode, demo_err,
    collections::collections_models::{
        APICardSearchFilters, AdjustWantQuantityRequest, CardIdentInner, CardToAdd,
        CollectionAddResponse, CollectionCard, CollectionCardsQuery, CollectionRemoveResponse,
        CollectionAllPurchaseHistoryResponse, CollectionPurchaseHistoryEntry,
        CollectionRenameRequest, CollectionValueBreakdown, CollectionsSearchQuery,
        PublicCollectionPage, PurchaseHistoryResponse, ResultCard, ResultCardInner,
        ShareLinkResponse, ShareLinkRevokeResponse,
    },
};
use models::CardTrait as _;
pub mod collections_models;

use crate::collections::collections_models::Collection;

/// Pick cardmarket retailer if present, else first available. Returns (normal_price, foil_price).
fn preferred_unit_prices(prices: &models::CardPrices) -> (f64, f64) {
    let rp = prices
        .paper
        .get("raw")
        .or_else(|| prices.paper.iter().find(|(k, _)| k.to_lowercase() == "cardmarket").map(|(_, v)| v))
        .or_else(|| prices.paper.values().next());
    let Some(rp) = rp else { return (0.0, 0.0) };
    let normal = rp.normal.or(rp.foil).unwrap_or(0.0);
    let foil = rp.foil.or(rp.normal).unwrap_or(0.0);
    (normal, foil)
}

/// Pure computation of a collection's value breakdown, given already-resolved
/// unit prices (normal, foil) per card uuid. Kept free of I/O so it can be
/// unit tested without a live retrieval system or database.
///
/// `cards` is a flat list of (uuid, finish) rows now (see
/// `models::CollectionCard::finish`), so `total_count`/`priced_count` count
/// rows, not distinct cards — a card owned in two finishes (e.g. nonfoil +
/// foil) contributes two rows. `purchase_totals` is keyed the same way:
/// `(card_uuid, finish)`.
fn compute_value_breakdown(
    cards: &[models::CollectionCard],
    unit_prices: &HashMap<String, (f64, f64)>,
    purchase_totals: &HashMap<(String, String), persistence::PurchaseSummary>,
) -> collections_models::CollectionValueBreakdown {
    // Wanted-only entries (nothing owned yet) aren't part of the collection's
    // owned value — exclude them so price totals and the priced/total ratio
    // only ever reflect quantities actually owned.
    let total_count = cards.iter().filter(|c| c.quantity > 0).count();

    let mut total_value: f64 = 0.0;
    let mut profit: f64 = 0.0;
    let mut untracked_value: f64 = 0.0;
    let mut priced_count: usize = 0;
    let mut wanted_value: f64 = 0.0;

    for card in cards {
        let Some(&(unit_normal, unit_foil)) = unit_prices.get(&card.uuid) else { continue };
        // Pricing only distinguishes normal/foil (see `models::CardPrices`)
        // — any other finish is priced as "normal" for lack of anything
        // better, same as an unpriced card would fall back to 0.
        let unit_price = if card.finish == "foil" { unit_foil } else { unit_normal };

        // want_quantity is only ever tracked on the "" (default) finish row.
        if card.want_quantity > 0 {
            wanted_value += unit_normal * card.want_quantity as f64;
        }

        if card.quantity <= 0 {
            continue;
        }

        let current = unit_price * card.quantity as f64;
        if current <= 0.0 {
            continue;
        }
        total_value += current;
        priced_count += 1;

        if let Some(summary) = purchase_totals.get(&(card.uuid.clone(), card.finish.clone())) {
            let paid = summary.quantity.min(card.quantity);
            let cost = if summary.quantity > 0 {
                summary.total_paid * paid as f64 / summary.quantity as f64
            } else {
                0.0
            };

            let current_of_paid = unit_price * paid as f64;
            profit += current_of_paid - cost;

            let unpaid = (card.quantity - paid).max(0);
            untracked_value += unit_price * unpaid as f64;
        } else {
            untracked_value += current;
        }
    }

    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    collections_models::CollectionValueBreakdown {
        total_value: round2(total_value),
        profit: round2(profit),
        untracked_value: round2(untracked_value),
        priced_count,
        total_count,
        wanted_value: round2(wanted_value),
    }
}

/// Returns all configured retrieval systems, cloned out of the state lock,
/// keyed by their provider name.
async fn clone_retrieval_systems_by_name(state: &GathersState) -> HashMap<String, RetrievalSystem> {
    let guard = state.0.lock().await;
    [
        guard.mtg.clone(),
        guard.riftbound.clone(),
        guard.pokemon.clone(),
    ]
    .into_iter()
    .flatten()
    .map(|s| (s.name().to_string(), s))
    .collect()
}

async fn clone_plugins_by_name(state: &GathersState) -> HashMap<String, retrieval::PluginRetrievalSystem> {
    state.0.lock().await.plugins.clone()
}

/// Checks whether `provider` (a real system's name, or `plugin-{name}`)
/// actually has `card_id`. Used both to trust-but-verify a client-supplied
/// provider and, when none is given, as part of the probe-everything
/// fallback below.
async fn provider_has_card(
    systems: &HashMap<String, RetrievalSystem>,
    plugins: &HashMap<String, retrieval::PluginRetrievalSystem>,
    provider: &str,
    card_id: &str,
) -> bool {
    let ids = vec![card_id.to_string()];
    if let Some(name) = provider.strip_prefix("plugin-") {
        let Some(plugin) = plugins.get(name) else {
            return false;
        };
        matches!(plugin.cards_by_ids(ids).await, Ok(found) if !found.is_empty())
    } else {
        let Some(system) = systems.get(provider) else {
            return false;
        };
        matches!(system.get_cards_by_ids(ids).await, Ok(found) if !found.is_empty())
    }
}

/// Determines which configured system or plugin owns `card_id`.
///
/// A client that knows the provider (the search UI always does — it's
/// whichever tab is active) should send it via `explicit`; it's verified
/// against that provider specifically rather than trusted blindly. Without
/// one — an older client, or a caller that genuinely doesn't know — this
/// falls back to probing every configured system and plugin in turn and
/// taking the first match, which is only correct when ids are unique
/// across all of them (two plugins both minting an id like `book-1` would
/// otherwise let the wrong one "win" the card).
async fn resolve_provider(state: &GathersState, card_id: &str, explicit: Option<&str>) -> String {
    let systems = clone_retrieval_systems_by_name(state).await;
    let plugins = clone_plugins_by_name(state).await;

    if let Some(explicit) = explicit.filter(|p| !p.is_empty())
        && provider_has_card(&systems, &plugins, explicit, card_id).await
    {
        return explicit.to_string();
    }

    for (name, system) in &systems {
        if let Ok(found) = system.get_cards_by_ids(vec![card_id.to_string()]).await
            && !found.is_empty()
        {
            return name.clone();
        }
    }
    for (name, plugin) in &plugins {
        if let Ok(found) = plugin.cards_by_ids(vec![card_id.to_string()]).await
            && !found.is_empty()
        {
            return format!("plugin-{name}");
        }
    }
    String::new()
}

/// A stored collection entry's provider is either a real system's
/// `NamedRetrievalSystem::name()`, or `plugin-{name}` for a third-party
/// plugin — see `cards_add`. This is the corresponding hydrated card data,
/// used everywhere collections needs to display/filter/sort by card detail
/// (`models::Card` can't represent a plugin card — see
/// `retrieval::systems::plugin`'s module doc for why).
#[allow(clippy::large_enum_variant)]
enum AnyCollectible {
    Known(Card),
    Plugin(retrieval::PluginCard),
}

/// Fetches card detail for every uuid in `by_provider`, dispatching each
/// provider group to the matching real system or, for a `plugin-{name}`
/// provider, to that plugin. Providers that don't resolve to anything
/// configured are silently skipped, same as the pre-plugin behavior for an
/// unrecognized provider.
async fn hydrate_collectibles(
    state: &GathersState,
    by_provider: &HashMap<String, Vec<String>>,
) -> HashMap<String, AnyCollectible> {
    let retrieval_systems = clone_retrieval_systems_by_name(state).await;
    let plugins = clone_plugins_by_name(state).await;
    let mut out = HashMap::new();
    for (provider, ids) in by_provider {
        if let Some(name) = provider.strip_prefix("plugin-") {
            if let Some(plugin) = plugins.get(name)
                && let Ok(data) = plugin.cards_by_ids(ids.clone()).await
            {
                out.extend(data.into_iter().map(|(k, v)| (k, AnyCollectible::Plugin(v))));
            }
        } else if let Some(retrieval) = retrieval_systems.get(provider)
            && let Ok(data) = retrieval.get_cards_by_ids(ids.clone()).await
        {
            out.extend(data.into_iter().map(|(k, v)| (k, AnyCollectible::Known(v))));
        }
    }
    out
}

fn matches_card_filters(card: &AnyCollectible, filters: &APICardSearchFilters) -> bool {
    match card {
        AnyCollectible::Known(card) => matches_known_card_filters(card, filters),
        AnyCollectible::Plugin(card) => matches_plugin_card_filters(card, filters),
    }
}

/// Plugin cards don't carry rarity/colors/mana/etc — a filter expressing one
/// of those constraints is treated as "not applicable" rather than
/// excluding the card. Only the fields `PluginCard` actually has
/// (name/set_code/collector_number/text) are checked.
fn matches_plugin_card_filters(card: &retrieval::PluginCard, filters: &APICardSearchFilters) -> bool {
    if let Some(ref v) = filters.name
        && !v.is_empty() && !card.name.to_lowercase().contains(&v.to_lowercase()) {
            return false;
        }
    if let Some(ref v) = filters.set_code
        && !v.is_empty() && !card.set_code.to_lowercase().contains(&v.to_lowercase()) {
            return false;
        }
    if let Some(ref v) = filters.collector_number
        && !v.is_empty() && card.collector_number != *v {
            return false;
        }
    if let Some(ref v) = filters.text
        && !v.is_empty() {
            let needle = v.to_lowercase();
            let in_name = card.name.to_lowercase().contains(&needle);
            let in_description = card
                .description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle);
            if !in_name && !in_description {
                return false;
            }
        }
    true
}

fn matches_known_card_filters(card: &Card, filters: &APICardSearchFilters) -> bool {

    let name_lower: String;
    let set_lower: String;
    let cn: String;

    match card {
        Card::Magic(m) => {
            name_lower = m.name.to_lowercase();
            set_lower = m.set_code.to_lowercase();
            cn = m.collector_number.clone();
        }
        Card::Riftbound(r) => {
            name_lower = r.name.to_lowercase();
            set_lower = r.set_code.to_lowercase();
            cn = r.collector_number.clone();
        }
        Card::Pokemon(p) => {
            name_lower = p.name.to_lowercase();
            set_lower = p.set_code.to_lowercase();
            cn = p.collector_number.clone();
        }
    }

    if let Some(ref v) = filters.name
        && !v.is_empty() && !name_lower.contains(&v.to_lowercase()) {
            return false;
        }
    if let Some(ref v) = filters.set_code
        && !v.is_empty() && !set_lower.contains(&v.to_lowercase()) {
            return false;
        }
    if let Some(ref v) = filters.collector_number
        && !v.is_empty() && cn != *v {
            return false;
        }

    match card {
        Card::Magic(m) => {
            if let Some(ref v) = filters.artist
                && !v.is_empty() && !m.artist.to_lowercase().contains(&v.to_lowercase()) {
                    return false;
                }
            if let Some(ref v) = filters.text
                && !v.is_empty() && !m.text.to_lowercase().contains(&v.to_lowercase()) {
                    return false;
                }
            if let Some(ref rarity) = filters.rarity {
                let filter_rarity = models::Rarity::from(rarity.clone());
                if m.rarity != filter_rarity {
                    return false;
                }
            }
            if let Some(ref colors) = filters.color_identities
                && !colors.is_empty() {
                    let filter_colors: Vec<models::CardColour> =
                        colors.iter().map(|c| models::CardColour::from(c.clone())).collect();
                    if !filter_colors.iter().all(|c| m.color_identity.contains(c)) {
                        return false;
                    }
                }
        }
        Card::Riftbound(r) => {
            if let Some(ref v) = filters.artist
                && !v.is_empty()
                    && !r.artists.iter().any(|a| a.to_lowercase().contains(&v.to_lowercase()))
                {
                    return false;
                }
            if let Some(ref v) = filters.text
                && !v.is_empty() && !r.text.to_lowercase().contains(&v.to_lowercase()) {
                    return false;
                }
            if let Some(ref domains) = filters.domains
                && !domains.is_empty() {
                    let filter_domains: Vec<models::riftbound::CardDomain> =
                        domains.iter().map(|d| models::riftbound::CardDomain::from(d.clone())).collect();
                    if !filter_domains.iter().all(|d| r.domains.contains(d)) {
                        return false;
                    }
                }
        }
        Card::Pokemon(p) => {
            if let Some(ref energy) = filters.energy_types
                && !energy.is_empty() {
                    let filter_energy: Vec<models::pokemon::EnergyType> =
                        energy.iter().map(|e| models::pokemon::EnergyType::from(e.clone())).collect();
                    if !filter_energy.iter().all(|e| p.energy_types.contains(e)) {
                        return false;
                    }
                }
        }
    }

    true
}

/// Serializes a card's full detail (whichever provider it belongs to) into a
/// flat JSON object matching the shape the webui2 client already expects for
/// a merged "collection card" (see `PublicCollectionPage`).
fn card_to_public_json(card: &AnyCollectible) -> serde_json::Map<String, serde_json::Value> {
    let value = match card {
        AnyCollectible::Known(Card::Magic(m)) => {
            serde_json::to_value(crate::mtg_api::mtg_api_models::APICard::from(m.clone()))
        }
        AnyCollectible::Known(Card::Riftbound(r)) => serde_json::to_value(
            crate::riftbound_api::riftbound_api_models::APIRiftboundCard::from(r.clone()),
        ),
        AnyCollectible::Known(Card::Pokemon(p)) => serde_json::to_value(
            crate::pokemon_api::pokemon_api_models::APIPokemonCard::from(p.clone()),
        ),
        AnyCollectible::Plugin(p) => Ok(serde_json::Value::Object(plugin_card_to_public_json(p))),
    };
    match value {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

/// `PluginCard`'s own (snake_case) `Serialize` isn't used here — the merged
/// public collection JSON is camelCase everywhere else, and the webui2
/// client's generic card rendering expects an `image` field, not
/// `image_url` (matching the convention `AnyCard`/`cardImageUrl` already use
/// for Riftbound/Pokemon).
fn plugin_card_to_public_json(card: &retrieval::PluginCard) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert("name".to_string(), serde_json::Value::String(card.name.clone()));
    map.insert("setCode".to_string(), serde_json::Value::String(card.set_code.clone()));
    map.insert("setName".to_string(), serde_json::Value::String(card.set_name.clone()));
    map.insert(
        "collectorNumber".to_string(),
        serde_json::Value::String(card.collector_number.clone()),
    );
    if let Some(ref d) = card.description {
        map.insert("description".to_string(), serde_json::Value::String(d.clone()));
    }
    if let Some(ref img) = card.image_url {
        map.insert("image".to_string(), serde_json::Value::String(img.clone()));
    }
    map
}

fn card_name(card: &Card) -> &str {
    match card {
        Card::Magic(m) => &m.name,
        Card::Riftbound(r) => &r.name,
        Card::Pokemon(p) => &p.name,
    }
}

fn collectible_name(card: &AnyCollectible) -> &str {
    match card {
        AnyCollectible::Known(c) => card_name(c),
        AnyCollectible::Plugin(p) => &p.name,
    }
}

fn collectible_set(card: &AnyCollectible) -> String {
    match card {
        AnyCollectible::Known(c) => c.get_set(),
        AnyCollectible::Plugin(p) => p.set_code.clone(),
    }
}

fn collectible_collector_number(card: &AnyCollectible) -> String {
    match card {
        AnyCollectible::Known(c) => c.get_collector_number(),
        AnyCollectible::Plugin(p) => p.collector_number.clone(),
    }
}

fn card_rarity_order(card: &Card) -> u8 {
    match card {
        Card::Magic(m) => match m.rarity {
            models::Rarity::Common => 0,
            models::Rarity::Uncommon => 1,
            models::Rarity::Rare => 2,
            models::Rarity::Mythic => 3,
            _ => 4,
        },
        Card::Riftbound(r) => match r.rarity {
            models::riftbound::RBRarity::Common => 0,
            models::riftbound::RBRarity::Uncommon => 1,
            models::riftbound::RBRarity::Rare => 2,
            models::riftbound::RBRarity::Epic => 3,
            _ => 4,
        },
        Card::Pokemon(p) => match p.rarity {
            models::pokemon::PokemonRarity::Common => 0,
            models::pokemon::PokemonRarity::Uncommon => 1,
            models::pokemon::PokemonRarity::Rare => 2,
            _ => 3,
        },
    }
}

fn collectible_rarity_order(card: &AnyCollectible) -> u8 {
    match card {
        AnyCollectible::Known(c) => card_rarity_order(c),
        // No rarity concept for a plugin card — sort it to the bottom.
        AnyCollectible::Plugin(_) => 5,
    }
}

fn sort_collection_cards(
    cards: &mut Vec<&models::CollectionCard>,
    card_data: &HashMap<String, AnyCollectible>,
    sort_by: &Option<crate::collections::collections_models::APISortField>,
    sort_order: &Option<crate::collections::collections_models::APISortOrder>,
) {
    use crate::collections::collections_models::{APISortField, APISortOrder};
    let desc = matches!(sort_order, Some(APISortOrder::Desc));

    cards.sort_by(|a, b| {
        let card_a = card_data.get(&a.uuid);
        let card_b = card_data.get(&b.uuid);
        let ord = match sort_by.as_ref().unwrap_or(&APISortField::Name) {
            APISortField::Name => {
                let na = card_a.map(collectible_name).unwrap_or("");
                let nb = card_b.map(collectible_name).unwrap_or("");
                na.cmp(nb)
            }
            APISortField::SetCode => {
                let sa = card_a.map(collectible_set).unwrap_or_default();
                let sb = card_b.map(collectible_set).unwrap_or_default();
                sa.cmp(&sb)
            }
            APISortField::CollectorNumber => {
                let ca = card_a.map(collectible_collector_number).unwrap_or_default();
                let cb = card_b.map(collectible_collector_number).unwrap_or_default();
                ca.cmp(&cb)
            }
            APISortField::Rarity => {
                let ra = card_a.map(collectible_rarity_order).unwrap_or(0);
                let rb = card_b.map(collectible_rarity_order).unwrap_or(0);
                ra.cmp(&rb)
            }
            APISortField::Artist => {
                let aa = match card_a {
                    Some(AnyCollectible::Known(Card::Magic(m))) => m.artist.as_str(),
                    _ => "",
                };
                let ab = match card_b {
                    Some(AnyCollectible::Known(Card::Magic(m))) => m.artist.as_str(),
                    _ => "",
                };
                aa.cmp(ab)
            }
            APISortField::ReleaseDate => {
                let da = match card_a {
                    Some(AnyCollectible::Known(Card::Pokemon(p))) => p.release_date.as_deref().unwrap_or(""),
                    _ => "",
                };
                let db = match card_b {
                    Some(AnyCollectible::Known(Card::Pokemon(p))) => p.release_date.as_deref().unwrap_or(""),
                    _ => "",
                };
                da.cmp(db)
            }
        };
        if desc { ord.reverse() } else { ord }
    });
}

pub fn collection_routes() -> ApiRouter<GathersState> {
    async fn list(State(state): State<GathersState>) -> Result<Json<Vec<Collection>>, ApiError> {
        let storage = &state.1.lock().await.storage;

        match storage.list_collections(None).await {
            Ok(collections) => Ok(Json(
                collections
                    .iter()
                    .map(|c| Collection { id: c.clone() })
                    .collect(),
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to list collections. {e}"),
                }),
            )),
        }
    }

    async fn add(
        State(state): State<GathersState>,
        Json(input): Json<Collection>,
    ) -> Result<Json<CollectionAddResponse>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        validate_collection_name(&input.id)?;

        let storage = &mut state.1.lock().await.storage;

        match storage.add_collection(input.id.clone()).await {
            Ok(collection_id) => Ok(Json(CollectionAddResponse {
                id: collection_id,
                name: input.id,
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to add collection. {e}"),
                }),
            )),
        }
    }

    async fn remove(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<CollectionRemoveResponse>, ApiError> {
        if demo_mode() { return Err(demo_err()); }

        let storage = &mut state.1.lock().await.storage;

        // TODO: allow setting the "move to collection" instead of None
        match storage.remove_collection(&id, None).await {
            Ok(message) => Ok(Json(CollectionRemoveResponse { message })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to remove collection. {e}"),
                }),
            )),
        }
    }

    async fn rename(
        State(state): State<GathersState>,
        Path(id): Path<String>,
        Json(input): Json<CollectionRenameRequest>,
    ) -> Result<Json<Collection>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        validate_collection_name(&input.new_id)?;

        let storage = &mut state.1.lock().await.storage;

        match storage.rename_collection(&id, &input.new_id).await {
            Ok(()) => Ok(Json(Collection { id: input.new_id })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to rename collection. {e}"),
                }),
            )),
        }
    }

    #[axum::debug_handler]
    async fn move_to(
        State(state): State<GathersState>,
        Path(to_collection_id): Path<String>,
        Json(input): Json<Vec<CollectionCard>>,
    ) -> Result<Json<()>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let storage = &mut state.1.lock().await.storage;

        let cards: Vec<models::CollectionCard> = input.iter().map(|card| card.into()).collect();
        match storage
            .move_cards_between_collections(&cards, to_collection_id)
            .await
        {
            Ok(_) => Ok(Json(())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to move cards. {e}"),
                }),
            )),
        }
    }

    async fn share_list(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<Vec<ShareLinkResponse>>, ApiError> {
        let storage = &state.1.lock().await.storage;

        match storage.list_share_links(&id).await {
            Ok(links) => Ok(Json(links.into_iter().map(ShareLinkResponse::from).collect())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to list share links. {e}"),
                }),
            )),
        }
    }

    async fn share_create(
        State(state): State<GathersState>,
        Path(id): Path<String>,
    ) -> Result<Json<ShareLinkResponse>, ApiError> {
        if demo_mode() { return Err(demo_err()); }

        let storage = &mut state.1.lock().await.storage;

        match storage.create_share_link(&id).await {
            Ok(link) => Ok(Json(link.into())),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to create share link. {e}"),
                }),
            )),
        }
    }

    async fn share_revoke(
        State(state): State<GathersState>,
        Path((id, token)): Path<(String, String)>,
    ) -> Result<Json<ShareLinkRevokeResponse>, ApiError> {
        if demo_mode() { return Err(demo_err()); }

        let storage = &mut state.1.lock().await.storage;

        match storage.revoke_share_link(&id, &token).await {
            Ok(revoked) => Ok(Json(ShareLinkRevokeResponse { revoked })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to revoke share link. {e}"),
                }),
            )),
        }
    }

    fn validate_collection_name(name: &str) -> Result<(), ApiError> {
        if name.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "Collection name cannot be empty".to_string(),
                }),
            ));
        }
        if name.len() > 255 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "Collection name too long (max 255 characters)".to_string(),
                }),
            ));
        }
        if name.chars().any(|c| c.is_control()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "Collection name contains invalid characters".to_string(),
                }),
            ));
        }
        Ok(())
    }

    async fn validate_collection(
        storage: &mut PersistenceSystem,
        collection_id: &String,
    ) -> Result<(), Json<ErrorPayload>> {
        let collections = match storage.list_collections(None).await {
            Ok(collections) => collections,
            Err(e) => {
                return Err(Json(ErrorPayload {
                    error: format!("Failed to verify collection. {e}"),
                }));
            }
        };

        if !collections.contains(collection_id) {
            return Err(Json(ErrorPayload {
                error: "Collection not found".to_string(),
            }));
        }
        Ok(())
    }

    async fn mutate_card_quantities(
        storage: &mut PersistenceSystem,
        collection_id: &str,
        uuid: String,
        finish: &str,
        quantity: i32,
        provider: String,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        match storage
            .add_card_to_collection(
                &collection_id.to_string(),
                &uuid,
                finish,
                quantity,
                &now_str,
                &provider,
            )
            .await
        {
            Ok(card) => Ok(Json(vec![CollectionCard {
                id: card.uuid.to_string(),
                finish: card.finish,
                quantity: card.quantity,
                want_quantity: card.want_quantity,
                collection_id: collection_id.to_string(),
                time_added: DateTime::parse_from_rfc3339(&card.time_added)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                provider: card.provider,
            }])),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to change card quantity in collection. {e}"),
                }),
            )),
        }
    }

    async fn cards_add(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Json(input): Json<CardToAdd>,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        let provider = resolve_provider(&state, &input.id, input.provider.as_deref()).await;

        let storage = &mut state.1.lock().await.storage;

        if let Err(e) = validate_collection(storage, &collection_id).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        };

        let result = mutate_card_quantities(
            storage,
            &collection_id,
            input.id.clone(),
            &input.finish,
            input.quantity,
            provider.clone(),
        )
        .await;

        // Record purchase history only when a positive price is supplied.
        if result.is_ok() && input.quantity > 0 && input.purchase_price.is_some_and(|p| p > 0.0) {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = storage
                .record_purchase(
                    &collection_id,
                    &input.id,
                    &input.finish,
                    input.quantity,
                    input.purchase_price,
                    &provider,
                    &now,
                )
                .await;
        }

        result
    }

    async fn cards_remove(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Json(input): Json<CardToAdd>,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let storage = &mut state.1.lock().await.storage;

        if let Err(e) = validate_collection(storage, &collection_id).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        };

        let neg_quantity = input.quantity.checked_neg().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "Invalid quantity".to_string(),
                }),
            )
        })?;

        mutate_card_quantities(
            storage,
            &collection_id,
            input.id,
            &input.finish,
            neg_quantity,
            "".to_string(),
        )
        .await
    }

    async fn cards_want_adjust(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Json(input): Json<AdjustWantQuantityRequest>,
    ) -> Result<Json<CollectionCard>, ApiError> {
        // Only used if the card doesn't already have a row in the collection;
        // an existing row keeps its own provider regardless.
        let provider = resolve_provider(&state, &input.id, input.provider.as_deref()).await;

        let storage = &mut state.1.lock().await.storage;

        if let Err(e) = validate_collection(storage, &collection_id).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        };

        match storage
            .adjust_want_quantity(&collection_id, &input.id, input.delta, &provider)
            .await
        {
            Ok(card) => Ok(Json(CollectionCard {
                id: card.uuid,
                finish: card.finish,
                quantity: card.quantity,
                want_quantity: card.want_quantity,
                collection_id: collection_id.clone(),
                time_added: DateTime::parse_from_rfc3339(&card.time_added)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                provider: card.provider,
            })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to adjust want quantity. {e}"),
                }),
            )),
        }
    }

    async fn cards_get(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Query(query): Query<CollectionCardsQuery>,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        let collection_params = CollectionCardsParams {
            offset: query.offset,
            limit: query.limit.min(1000),
            sort_by: query.sort_by.map(persistence::CollectionSortField::from),
            sort_order: query.sort_order.map(models::filters::SortOrder::from),
            provider: query.provider,
            providers: query.providers
                .as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        };
        let cards = state
            .1
            .lock()
            .await
            .storage
            .get_cards_in_collection_paginated(&collection_id, collection_params)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to get cards from collection. {e}"),
                    }),
                )
            })?;

        let response_cards = cards
            .into_iter()
            .map(|card| CollectionCard {
                id: card.uuid,
                finish: card.finish,
                quantity: card.quantity,
                want_quantity: card.want_quantity,
                collection_id: collection_id.clone(),
                time_added: DateTime::parse_from_rfc3339(&card.time_added)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                provider: card.provider,
            })
            .collect();

        Ok(Json(response_cards))
    }

    async fn collection_cards_count(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Query(query): Query<CollectionCardsQuery>,
    ) -> Result<Json<usize>, ApiError> {
        let storage = &mut state.1.lock().await.storage;
        let providers: Vec<String> = if let Some(p) = query.provider {
            vec![p]
        } else {
            query.providers
                .as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default()
        };

        match storage.get_cards_in_collection_count(collection_id, &providers).await {
            Ok(count) => Ok(Json(count)),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to get card count for collection. {e}"),
                }),
            )),
        }
    }

    async fn search_temp(
        State(state): State<GathersState>,
        Query(query): Query<CollectionsSearchQuery>,
        Json(input): Json<APICardSearchFilters>,
    ) -> Result<Json<Vec<ResultCard>>, ApiError> {
        let guard = state.0.lock().await;
        let ret = guard.require_mtg()?;

        match ret
            .search_cards(input.into(), query.offset.into(), query.page_size.min(1000).into())
            .await
        {
            Ok(result) => Ok(Json(
                result
                    .iter()
                    .filter_map(|c| match c {
                        Card::Magic(m) => Some(m),
                        _ => None,
                    })
                    .map(|c| ResultCard {
                        mtg_card: ResultCardInner {
                            id: c.id.clone(),
                            name: c.name.clone(),
                            set_code: c.set_code.clone(),
                            card_identifiers: CardIdentInner {
                                scryfall_id: c.card_identifiers.scryfall_id.clone(),
                            },
                        },
                    })
                    .collect(),
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to search cards. {e}"),
                }),
            )),
        }
    }

    /// Optional CSV column-header overrides for import/export, shared by
    /// both endpoints (as query params on export, matching-named multipart
    /// text fields on import — see `build_csv_mapping`). `preset` names one
    /// of `persistence::CsvFieldMapping`'s built-in third-party formats
    /// (e.g. `tcgplayer`); any of the five `*_field` overrides given on top
    /// of it replace just that one field, preset or not. Defaults (nothing
    /// given at all) reproduce gathers' own original fixed CSV format.
    #[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
    struct CsvMappingParams {
        #[serde(default)]
        preset: Option<String>,
        #[serde(default)]
        set_code_field: Option<String>,
        #[serde(default)]
        collector_number_field: Option<String>,
        #[serde(default)]
        quantity_field: Option<String>,
        #[serde(default)]
        foil_quantity_field: Option<String>,
        #[serde(default)]
        provider_field: Option<String>,
    }

    fn build_csv_mapping(params: &CsvMappingParams) -> Result<persistence::CsvFieldMapping, ApiError> {
        let mut mapping = match params.preset.as_deref() {
            None | Some("") => persistence::CsvFieldMapping::default(),
            Some(name) => persistence::CsvFieldMapping::preset(name).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorPayload {
                        error: format!(
                            "Unknown CSV mapping preset '{name}' (known presets: tcgplayer, cardkingdom, deckbox, mtggoldfish)"
                        ),
                    }),
                )
            })?,
        };
        for (field, value) in [
            (persistence::CsvField::SetCode, &params.set_code_field),
            (persistence::CsvField::CollectorNumber, &params.collector_number_field),
            (persistence::CsvField::Quantity, &params.quantity_field),
            (persistence::CsvField::FoilQuantity, &params.foil_quantity_field),
            (persistence::CsvField::Provider, &params.provider_field),
        ] {
            if let Some(header) = value {
                mapping.set(field, Some(header.clone()));
            }
        }
        Ok(mapping)
    }

    async fn export(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Query(mapping_params): Query<CsvMappingParams>,
    ) -> Result<Response, ApiError> {
        let mapping = build_csv_mapping(&mapping_params)?;

        let retrievals: Vec<RetrievalSystem> = {
            let guard = state.0.lock().await;
            [guard.mtg.clone(), guard.riftbound.clone(), guard.pokemon.clone()]
                .into_iter()
                .flatten()
                .collect()
        };

        let csv = state
            .1
            .lock()
            .await
            .storage
            .export_collection(&collection_id, &retrievals, &mapping)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Export failed: {e}"),
                    }),
                )
            })?;

        let safe_filename: String = collection_id
            .chars()
            .filter(|c| *c != '"' && *c != '\\' && !c.is_control())
            .collect();
        let filename = format!("attachment; filename=\"{safe_filename}.csv\"");
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("text/csv"))
            .header(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&filename).unwrap_or_else(|_| {
                    HeaderValue::from_static("attachment; filename=\"collection.csv\"")
                }),
            )
            .body(axum::body::Body::from(csv))
            .unwrap())
    }

    async fn import(
        State(state): State<GathersState>,
        mut multipart: Multipart,
    ) -> Result<Json<()>, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let mut file_bytes: Option<Vec<u8>> = None;
        let mut collection_name: Option<String> = None;
        let mut mapping_params = CsvMappingParams::default();

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: format!("Failed to read multipart field: {e}"),
                }),
            )
        })? {
            let name = field.name().map(str::to_string);
            match name.as_deref() {
                Some("file") => {
                    file_bytes = Some(field.bytes().await.map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorPayload {
                                error: format!("Failed to read file bytes: {e}"),
                            }),
                        )
                    })?.to_vec());
                }
                Some("collection") => {
                    collection_name = Some(field.text().await.map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorPayload {
                                error: format!("Failed to read collection field: {e}"),
                            }),
                        )
                    })?);
                }
                // Optional CSV field-mapping overrides — see `CsvMappingParams`.
                Some(field_name @ ("preset" | "set_code_field" | "collector_number_field"
                    | "quantity_field" | "foil_quantity_field" | "provider_field")) => {
                    let value = field.text().await.map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorPayload {
                                error: format!("Failed to read '{field_name}' field: {e}"),
                            }),
                        )
                    })?;
                    let target = match field_name {
                        "preset" => &mut mapping_params.preset,
                        "set_code_field" => &mut mapping_params.set_code_field,
                        "collector_number_field" => &mut mapping_params.collector_number_field,
                        "quantity_field" => &mut mapping_params.quantity_field,
                        "foil_quantity_field" => &mut mapping_params.foil_quantity_field,
                        "provider_field" => &mut mapping_params.provider_field,
                        _ => unreachable!(),
                    };
                    *target = Some(value);
                }
                _ => {}
            }
        }

        let mapping = build_csv_mapping(&mapping_params)?;

        let bytes = file_bytes.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "No file provided".to_string(),
                }),
            )
        })?;

        let collection_name = collection_name.unwrap_or_else(|| "New Collection".to_string());
        validate_collection_name(&collection_name)?;

        let mut tmp = tempfile::NamedTempFile::new().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to create temp file: {e}"),
                }),
            )
        })?;
        std::io::Write::write_all(&mut tmp, &bytes).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to write temp file: {e}"),
                }),
            )
        })?;
        let tmp_path = tmp.path().to_string_lossy().to_string();

        let retrievals: Vec<RetrievalSystem> = {
            let guard = state.0.lock().await;
            [guard.mtg.clone(), guard.riftbound.clone(), guard.pokemon.clone()]
                .into_iter()
                .flatten()
                .collect()
        };

        state
            .1
            .lock()
            .await
            .storage
            .import_csv(tmp_path, collection_name, &retrievals, None, &mapping)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Import failed: {e}"),
                    }),
                )
            })?;

        Ok(Json(()))
    }

    async fn collection_cards_search(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Query(query): Query<CollectionCardsQuery>,
        Json(filters): Json<APICardSearchFilters>,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        let all_params = CollectionCardsParams {
            offset: 0,
            limit: i64::MAX as usize,
            sort_by: None,
            sort_order: None,
            provider: query.provider.clone(),
            providers: query.providers.as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        };

        let collection_cards = state
            .1
            .lock()
            .await
            .storage
            .get_cards_in_collection_paginated(&collection_id, all_params)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload { error: format!("Failed to get cards from collection. {e}") }),
                )
            })?;

        let mut by_provider: HashMap<String, Vec<models::CollectionCard>> = HashMap::new();
        for card in collection_cards {
            by_provider.entry(card.provider.clone()).or_default().push(card);
        }

        let ids_by_provider: HashMap<String, Vec<String>> = by_provider
            .iter()
            .map(|(provider, cards)| (provider.clone(), cards.iter().map(|c| c.uuid.clone()).collect()))
            .collect();
        let card_data = hydrate_collectibles(&state, &ids_by_provider).await;

        let mut matched: Vec<&models::CollectionCard> = by_provider
            .values()
            .flatten()
            .filter(|cc| {
                card_data
                    .get(&cc.uuid)
                    .map(|card| matches_card_filters(card, &filters))
                    .unwrap_or(false)
            })
            .collect();

        sort_collection_cards(&mut matched, &card_data, &filters.sort_by, &filters.sort_order);

        let page: Vec<CollectionCard> = matched
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .map(|cc| CollectionCard {
                id: cc.uuid.clone(),
                finish: cc.finish.clone(),
                quantity: cc.quantity,
                want_quantity: cc.want_quantity,
                collection_id: collection_id.clone(),
                time_added: DateTime::parse_from_rfc3339(&cc.time_added)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                provider: cc.provider.clone(),
            })
            .collect();

        Ok(Json(page))
    }

    async fn collection_cards_search_count(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
        Query(query): Query<CollectionCardsQuery>,
        Json(filters): Json<APICardSearchFilters>,
    ) -> Result<Json<usize>, ApiError> {
        let all_params = CollectionCardsParams {
            offset: 0,
            limit: i64::MAX as usize,
            sort_by: None,
            sort_order: None,
            provider: query.provider.clone(),
            providers: query.providers.as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        };

        let collection_cards = state
            .1
            .lock()
            .await
            .storage
            .get_cards_in_collection_paginated(&collection_id, all_params)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload { error: format!("Failed to get cards from collection. {e}") }),
                )
            })?;

        let mut by_provider: HashMap<String, Vec<models::CollectionCard>> = HashMap::new();
        for card in collection_cards {
            by_provider.entry(card.provider.clone()).or_default().push(card);
        }

        let ids_by_provider: HashMap<String, Vec<String>> = by_provider
            .iter()
            .map(|(provider, cards)| (provider.clone(), cards.iter().map(|c| c.uuid.clone()).collect()))
            .collect();
        let card_data = hydrate_collectibles(&state, &ids_by_provider).await;

        let count = by_provider
            .values()
            .flatten()
            .filter(|cc| {
                card_data
                    .get(&cc.uuid)
                    .map(|card| matches_card_filters(card, &filters))
                    .unwrap_or(false)
            })
            .count();

        Ok(Json(count))
    }

    async fn purchase_history(
        State(state): State<GathersState>,
        Path((collection_id, card_uuid)): Path<(String, String)>,
    ) -> Result<Json<PurchaseHistoryResponse>, ApiError> {
        let storage = &state.1.lock().await.storage;
        match storage.get_purchase_history(&collection_id, &card_uuid).await {
            Ok(entries) => Ok(Json(PurchaseHistoryResponse { entries })),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    error: format!("Failed to get purchase history. {e}"),
                }),
            )),
        }
    }

    async fn collection_value_breakdown(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
    ) -> Result<Json<CollectionValueBreakdown>, ApiError> {
        let retrieval_systems = clone_retrieval_systems_by_name(&state).await;

        let storage_guard = state.1.lock().await;

        let collection_cards = storage_guard
            .storage
            .get_cards_in_collection_paginated(
                &collection_id,
                CollectionCardsParams {
                    offset: 0,
                    limit: 100_000,
                    sort_by: None,
                    sort_order: None,
                    provider: None,
                    providers: vec![],
                },
            )
            .await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload { error: format!("Failed to get cards. {e}") }),
            ))?;

        let purchase_totals = storage_guard
            .storage
            .get_collection_purchase_totals(&collection_id)
            .await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload { error: format!("Failed to get purchase history. {e}") }),
            ))?;

        drop(storage_guard);

        // Group by provider for bulk price lookup. Wanted-only entries (nothing
        // owned yet) are kept here too, so their wishlist price is still fetched.
        let mut by_provider: HashMap<String, Vec<models::CollectionCard>> = HashMap::new();
        for card in collection_cards {
            by_provider.entry(card.provider.clone()).or_default().push(card);
        }

        let mut unit_prices: HashMap<String, (f64, f64)> = HashMap::new();
        for (provider, cards) in &by_provider {
            if let Some(retrieval) = retrieval_systems.get(provider) {
                let ids: Vec<String> = cards.iter().map(|c| c.uuid.clone()).collect();
                if let Ok(prices_map) = retrieval.get_bulk_card_prices(ids).await {
                    for card in cards {
                        if let Some(card_prices) = prices_map.get(&card.uuid) {
                            unit_prices.insert(card.uuid.clone(), preferred_unit_prices(card_prices));
                        }
                    }
                }
            }
        }

        let all_cards: Vec<models::CollectionCard> = by_provider.into_values().flatten().collect();
        Ok(Json(compute_value_breakdown(&all_cards, &unit_prices, &purchase_totals)))
    }

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct UpdatePurchaseEntryBody {
        quantity: i32,
        price_per_unit: Option<f64>,
    }

    async fn delete_purchase_entry(
        State(state): State<GathersState>,
        Path((collection_id, entry_id)): Path<(String, i64)>,
    ) -> Result<StatusCode, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let found = state
            .1
            .lock()
            .await
            .storage
            .delete_purchase_entry(&collection_id, entry_id)
            .await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload { error: format!("Failed to delete entry. {e}") }),
            ))?;
        if found { Ok(StatusCode::NO_CONTENT) } else { Ok(StatusCode::NOT_FOUND) }
    }

    async fn update_purchase_entry(
        State(state): State<GathersState>,
        Path((collection_id, entry_id)): Path<(String, i64)>,
        Json(body): Json<UpdatePurchaseEntryBody>,
    ) -> Result<StatusCode, ApiError> {
        if demo_mode() { return Err(demo_err()); }
        let result = state
            .1
            .lock()
            .await
            .storage
            .update_purchase_entry(
                &collection_id,
                entry_id,
                body.quantity,
                body.price_per_unit,
            )
            .await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload { error: format!("Failed to update entry. {e}") }),
            ))?;
        match result {
            UpdateEntryResult::Updated => Ok(StatusCode::NO_CONTENT),
            UpdateEntryResult::NotFound => Ok(StatusCode::NOT_FOUND),
            UpdateEntryResult::ValidationError(msg) => Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload { error: msg }),
            )),
        }
    }

    async fn all_purchase_history(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
    ) -> Result<Json<CollectionAllPurchaseHistoryResponse>, ApiError> {
        let entries = state
            .1
            .lock()
            .await
            .storage
            .get_all_purchase_history(&collection_id)
            .await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload { error: format!("Failed to get purchase history. {e}") }),
            ))?;

        let mut uuids_by_provider: HashMap<String, Vec<String>> = HashMap::new();
        for entry in &entries {
            uuids_by_provider
                .entry(entry.provider.clone())
                .or_default()
                .push(entry.card_uuid.clone());
        }
        for ids in uuids_by_provider.values_mut() {
            ids.sort();
            ids.dedup();
        }

        let card_info = hydrate_collectibles(&state, &uuids_by_provider).await;

        let result = entries
            .into_iter()
            .map(|e| {
                let card = card_info.get(&e.card_uuid);
                CollectionPurchaseHistoryEntry {
                    id: e.id,
                    card_name: card.map(|c| collectible_name(c).to_string()),
                    set_code: card.map(collectible_set),
                    card_uuid: e.card_uuid,
                    finish: e.finish,
                    quantity: e.quantity,
                    price_per_unit: e.price_per_unit,
                    provider: e.provider,
                    recorded_at: e.recorded_at,
                }
            })
            .collect();

        Ok(Json(CollectionAllPurchaseHistoryResponse { entries: result }))
    }

    ApiRouter::new()
        .api_route("/list", get(list))
        .api_route("/add", post(add))
        .api_route("/remove/{id}", post(remove))
        .api_route("/rename/{id}", post(rename))
        .api_route("/move/{id}", post(move_to))
        .api_route("/share/{id}", get(share_list).post(share_create))
        .api_route("/share/{id}/{token}", delete(share_revoke))
        .api_route("/cards/{id}/list", get(cards_get))
        .api_route("/cards/{id}/count", get(collection_cards_count))
        .api_route("/cards/{id}/search", post(collection_cards_search))
        .api_route("/cards/{id}/search/count", post(collection_cards_search_count))
        .api_route("/search", post(search_temp))
        .api_route("/cards/{id}/add", post(cards_add))
        .api_route("/cards/{id}/delete", post(cards_remove))
        .api_route("/cards/{id}/want", post(cards_want_adjust))
        .api_route("/cards/{id}/purchase_history/{card_uuid}", get(purchase_history))
        .api_route("/cards/{id}/purchase_history", get(all_purchase_history))
        .api_route("/cards/{id}/purchase_history_entry/{entry_id}", delete(delete_purchase_entry).patch(update_purchase_entry))
        .api_route("/cards/{id}/value_breakdown", get(collection_value_breakdown))
        .route("/import", axum::routing::post(import))
        .route("/export/{id}", axum::routing::get(export))
}

/// Routes for read-only, shareable collection links. Distinct from
/// `collection_routes` (mounted separately, under `/api/share`) so that a
/// reverse proxy can carve out an auth exception for exactly this prefix
/// (plus the matching `/share` webui2 page) without exposing any of the
/// collection-editing endpoints.
///
/// A collection is reachable here only via an opaque share `token` that the
/// owner explicitly minted (see the `/share` management endpoints on
/// `collection_routes`) and can revoke at any time — knowing the collection's
/// name grants no access.
pub fn public_collection_routes() -> ApiRouter<GathersState> {
    /// One page of full card data for a collection, merging entry metadata
    /// (quantity, provider, ...) with full card details in a single response
    /// so a shared link doesn't need per-card follow-up requests.
    async fn get_public_cards(
        State(state): State<GathersState>,
        Path(token): Path<String>,
        Query(query): Query<CollectionCardsQuery>,
    ) -> Result<Json<PublicCollectionPage>, ApiError> {
        let collection_id = state
            .1
            .lock()
            .await
            .storage
            .resolve_share_link(&token)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to resolve share link. {e}"),
                    }),
                )
            })?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorPayload {
                        error: "This share link is invalid or has been revoked.".to_string(),
                    }),
                )
            })?;

        let collection_params = CollectionCardsParams {
            offset: query.offset,
            limit: query.limit.min(1000),
            sort_by: query.sort_by.map(persistence::CollectionSortField::from),
            sort_order: query.sort_order.map(models::filters::SortOrder::from),
            provider: query.provider.clone(),
            providers: query.providers
                .as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        };

        let entries = state
            .1
            .lock()
            .await
            .storage
            .get_cards_in_collection_paginated(&collection_id, collection_params)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to get cards from collection. {e}"),
                    }),
                )
            })?;

        let providers: Vec<String> = if let Some(p) = query.provider.clone() {
            vec![p]
        } else {
            query.providers
                .as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default()
        };
        let total = state
            .1
            .lock()
            .await
            .storage
            .get_cards_in_collection_count(collection_id.clone(), &providers)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorPayload {
                        error: format!("Failed to get card count for collection. {e}"),
                    }),
                )
            })?;

        let mut ids_by_provider: HashMap<String, Vec<String>> = HashMap::new();
        for entry in &entries {
            ids_by_provider.entry(entry.provider.clone()).or_default().push(entry.uuid.clone());
        }

        let card_data = hydrate_collectibles(&state, &ids_by_provider).await;

        // A plugin provider that's since been disabled/removed can't be
        // hydrated — such a card shouldn't show up on a shared page any
        // more than in the owner's own collection view.
        let cards = entries
            .into_iter()
            .filter(|entry| {
                !entry.provider.starts_with("plugin-") || card_data.contains_key(&entry.uuid)
            })
            .map(|entry| {
                let mut obj = card_data
                    .get(&entry.uuid)
                    .map(card_to_public_json)
                    .unwrap_or_default();
                let time_added = DateTime::parse_from_rfc3339(&entry.time_added)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                obj.insert("id".to_string(), serde_json::Value::String(entry.uuid));
                obj.insert("finish".to_string(), serde_json::Value::String(entry.finish));
                obj.insert("quantity".to_string(), serde_json::Value::from(entry.quantity));
                obj.insert("wantQuantity".to_string(), serde_json::Value::from(entry.want_quantity));
                obj.insert(
                    "collectionId".to_string(),
                    serde_json::Value::String(collection_id.clone()),
                );
                obj.insert(
                    "timeAdded".to_string(),
                    serde_json::Value::String(time_added.to_rfc3339()),
                );
                obj.insert("provider".to_string(), serde_json::Value::String(entry.provider));
                serde_json::Value::Object(obj)
            })
            .collect();

        Ok(Json(PublicCollectionPage { cards, total }))
    }

    ApiRouter::new().api_route("/{token}", get(get_public_cards))
}

#[cfg(test)]
mod value_breakdown_tests {
    use super::*;

    fn card(uuid: &str, finish: &str, quantity: i32, want_quantity: i32) -> models::CollectionCard {
        models::CollectionCard {
            uuid: uuid.to_string(),
            finish: finish.to_string(),
            quantity,
            want_quantity,
            time_added: "2024-01-01T00:00:00Z".to_string(),
            collection: "c1".to_string(),
            provider: "mtg".to_string(),
        }
    }

    #[test]
    fn wanted_value_sums_price_times_want_quantity() {
        let cards = vec![card("a", "", 0, 3)];
        let unit_prices = HashMap::from([("a".to_string(), (2.5, 4.0))]);
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        assert_eq!(breakdown.wanted_value, 7.5);
        // Wanted-only entries aren't owned, so they don't affect owned totals.
        assert_eq!(breakdown.total_value, 0.0);
        assert_eq!(breakdown.total_count, 0);
        assert_eq!(breakdown.priced_count, 0);
    }

    #[test]
    fn wanted_value_excluded_from_owned_totals_when_also_owned() {
        // Owned 2 normal, wants 5 more on top of what's owned.
        let cards = vec![card("a", "", 2, 5)];
        let unit_prices = HashMap::from([("a".to_string(), (1.0, 1.0))]);
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        assert_eq!(breakdown.total_value, 2.0);
        assert_eq!(breakdown.wanted_value, 5.0);
        assert_eq!(breakdown.total_count, 1);
        assert_eq!(breakdown.priced_count, 1);
    }

    #[test]
    fn zero_want_quantity_contributes_nothing() {
        let cards = vec![card("a", "", 1, 0)];
        let unit_prices = HashMap::from([("a".to_string(), (3.0, 3.0))]);
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        assert_eq!(breakdown.wanted_value, 0.0);
    }

    #[test]
    fn missing_price_excludes_card_from_wanted_value() {
        let cards = vec![card("a", "", 0, 4)];
        let unit_prices = HashMap::new();
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        assert_eq!(breakdown.wanted_value, 0.0);
    }

    #[test]
    fn multiple_wanted_cards_sum_together() {
        let cards = vec![card("a", "", 0, 2), card("b", "", 1, 1)];
        let unit_prices = HashMap::from([
            ("a".to_string(), (10.0, 10.0)),
            ("b".to_string(), (5.0, 5.0)),
        ]);
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        // a: 2 * 10.0 = 20.0, b: 1 * 5.0 = 5.0
        assert_eq!(breakdown.wanted_value, 25.0);
    }

    #[test]
    fn foil_finish_uses_foil_unit_price() {
        let cards = vec![card("a", "foil", 2, 0)];
        let unit_prices = HashMap::from([("a".to_string(), (1.0, 9.0))]);
        let purchase_totals = HashMap::new();

        let breakdown = compute_value_breakdown(&cards, &unit_prices, &purchase_totals);

        assert_eq!(breakdown.total_value, 18.0);
    }
}
