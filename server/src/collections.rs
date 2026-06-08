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
    ApiError, ErrorPayload, GathersState,
    collections::collections_models::{
        APICardSearchFilters, CardIdentInner, CardToAdd, CollectionAddResponse, CollectionCard,
        CollectionCardsQuery, CollectionRemoveResponse, CollectionAllPurchaseHistoryResponse,
        CollectionPurchaseHistoryEntry, CollectionRenameRequest, CollectionValueBreakdown,
        CollectionsSearchQuery, PurchaseHistoryResponse, ResultCard, ResultCardInner,
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

fn matches_card_filters(card: &Card, filters: &APICardSearchFilters) -> bool {

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

fn card_name(card: &Card) -> &str {
    match card {
        Card::Magic(m) => &m.name,
        Card::Riftbound(r) => &r.name,
        Card::Pokemon(p) => &p.name,
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

fn sort_collection_cards(
    cards: &mut Vec<&models::CollectionCard>,
    card_data: &HashMap<String, Card>,
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
                let na = card_a.map(card_name).unwrap_or("");
                let nb = card_b.map(card_name).unwrap_or("");
                na.cmp(nb)
            }
            APISortField::SetCode => {
                let sa = card_a.map(|c| c.get_set()).unwrap_or_default();
                let sb = card_b.map(|c| c.get_set()).unwrap_or_default();
                sa.cmp(&sb)
            }
            APISortField::CollectorNumber => {
                let ca = card_a.map(|c| c.get_collector_number()).unwrap_or_default();
                let cb = card_b.map(|c| c.get_collector_number()).unwrap_or_default();
                ca.cmp(&cb)
            }
            APISortField::Rarity => {
                let ra = card_a.map(card_rarity_order).unwrap_or(0);
                let rb = card_b.map(card_rarity_order).unwrap_or(0);
                ra.cmp(&rb)
            }
            APISortField::Artist => {
                let aa = match card_a {
                    Some(Card::Magic(m)) => m.artist.as_str(),
                    _ => "",
                };
                let ab = match card_b {
                    Some(Card::Magic(m)) => m.artist.as_str(),
                    _ => "",
                };
                aa.cmp(ab)
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
        quantity: i32,
        foil_quantity: i32,
        provider: String,
    ) -> Result<Json<Vec<CollectionCard>>, ApiError> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        match storage
            .add_card_to_collection(
                &collection_id.to_string(),
                &uuid,
                quantity,
                foil_quantity,
                &now_str,
                &provider,
            )
            .await
        {
            Ok(card) => Ok(Json(vec![CollectionCard {
                id: card.uuid.to_string(),
                quantity: card.quantity,
                foil_quantity: card.foil_quantity,
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
        // Identify the provider by finding which configured system has this card.
        let systems = clone_retrieval_systems_by_name(&state).await;
        let card_ids = vec![input.id.clone()];
        let mut provider = String::new();
        for (name, system) in &systems {
            if let Ok(found) = system.get_cards_by_ids(card_ids.clone()).await
                && !found.is_empty()
            {
                provider = name.clone();
                break;
            }
        }

        let storage = &mut state.1.lock().await.storage;

        if let Err(e) = validate_collection(storage, &collection_id).await {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e));
        };

        let result = mutate_card_quantities(
            storage,
            &collection_id,
            input.id.clone(),
            input.quantity,
            input.foil_quantity,
            provider.clone(),
        )
        .await;

        // Record purchase history only when a positive price is supplied.
        if result.is_ok()
            && (input.quantity > 0 || input.foil_quantity > 0)
            && input.purchase_price.map_or(false, |p| p > 0.0)
        {
            let now = chrono::Utc::now().to_rfc3339();
            let normal_price = if input.quantity > 0 { input.purchase_price } else { None };
            let foil_price = if input.foil_quantity > 0 { input.purchase_price } else { None };
            let _ = storage
                .record_purchase(
                    &collection_id,
                    &input.id,
                    input.quantity.max(0),
                    input.foil_quantity.max(0),
                    normal_price,
                    foil_price,
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
        let neg_foil_quantity = input.foil_quantity.checked_neg().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: "Invalid foil quantity".to_string(),
                }),
            )
        })?;

        mutate_card_quantities(
            storage,
            &collection_id,
            input.id,
            neg_quantity,
            neg_foil_quantity,
            "".to_string(),
        )
        .await
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
                quantity: card.quantity,
                foil_quantity: card.foil_quantity,
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

    async fn export(
        State(state): State<GathersState>,
        Path(collection_id): Path<String>,
    ) -> Result<Response, ApiError> {
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
            .export_collection(&collection_id, &retrievals)
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
        let mut file_bytes: Option<Vec<u8>> = None;
        let mut collection_name: Option<String> = None;

        while let Some(field) = multipart.next_field().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorPayload {
                    error: format!("Failed to read multipart field: {e}"),
                }),
            )
        })? {
            match field.name() {
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
                _ => {}
            }
        }

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
            .import_csv(tmp_path, collection_name, &retrievals, None)
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
        let retrieval_systems = clone_retrieval_systems_by_name(&state).await;

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

        let mut card_data: HashMap<String, Card> = HashMap::new();
        for (provider, cards) in &by_provider {
            if let Some(retrieval) = retrieval_systems.get(provider) {
                let ids: Vec<String> = cards.iter().map(|c| c.uuid.clone()).collect();
                if let Ok(data) = retrieval.get_cards_by_ids(ids).await {
                    card_data.extend(data);
                }
            }
        }

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
                quantity: cc.quantity,
                foil_quantity: cc.foil_quantity,
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
        let retrieval_systems = clone_retrieval_systems_by_name(&state).await;

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

        let mut card_data: HashMap<String, Card> = HashMap::new();
        for (provider, cards) in &by_provider {
            if let Some(retrieval) = retrieval_systems.get(provider) {
                let ids: Vec<String> = cards.iter().map(|c| c.uuid.clone()).collect();
                if let Ok(data) = retrieval.get_cards_by_ids(ids).await {
                    card_data.extend(data);
                }
            }
        }

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

        // Group by provider for bulk price lookup.
        let mut by_provider: HashMap<String, Vec<models::CollectionCard>> = HashMap::new();
        for card in collection_cards {
            by_provider.entry(card.provider.clone()).or_default().push(card);
        }

        let total_count = by_provider.values().map(|v| v.len()).sum::<usize>();
        let mut total_value: f64 = 0.0;
        let mut profit: f64 = 0.0;
        let mut untracked_value: f64 = 0.0;
        let mut priced_count: usize = 0;

        for (provider, cards) in &by_provider {
            if let Some(retrieval) = retrieval_systems.get(provider) {
                let ids: Vec<String> = cards.iter().map(|c| c.uuid.clone()).collect();
                if let Ok(prices_map) = retrieval.get_bulk_card_prices(ids).await {
                    for card in cards {
                        if let Some(card_prices) = prices_map.get(&card.uuid) {
                            let (unit_normal, unit_foil) = preferred_unit_prices(card_prices);
                            let current = unit_normal * card.quantity as f64
                                + unit_foil * card.foil_quantity as f64;
                            if current <= 0.0 {
                                continue;
                            }
                            total_value += current;
                            priced_count += 1;

                            if let Some(summary) = purchase_totals.get(&card.uuid) {
                                let paid_normal = summary.quantity.min(card.quantity);
                                let paid_foil = summary.foil_quantity.min(card.foil_quantity);

                                let cost_normal = if summary.quantity > 0 {
                                    summary.total_normal_paid * paid_normal as f64
                                        / summary.quantity as f64
                                } else {
                                    0.0
                                };
                                let cost_foil = if summary.foil_quantity > 0 {
                                    summary.total_foil_paid * paid_foil as f64
                                        / summary.foil_quantity as f64
                                } else {
                                    0.0
                                };

                                let current_of_paid =
                                    unit_normal * paid_normal as f64 + unit_foil * paid_foil as f64;
                                profit += current_of_paid - (cost_normal + cost_foil);

                                let unpaid_normal = (card.quantity - paid_normal).max(0);
                                let unpaid_foil = (card.foil_quantity - paid_foil).max(0);
                                untracked_value += unit_normal * unpaid_normal as f64
                                    + unit_foil * unpaid_foil as f64;
                            } else {
                                untracked_value += current;
                            }
                        }
                    }
                }
            }
        }

        let round2 = |v: f64| (v * 100.0).round() / 100.0;
        Ok(Json(CollectionValueBreakdown {
            total_value: round2(total_value),
            profit: round2(profit),
            untracked_value: round2(untracked_value),
            priced_count,
            total_count,
        }))
    }

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct UpdatePurchaseEntryBody {
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
    }

    async fn delete_purchase_entry(
        State(state): State<GathersState>,
        Path((collection_id, entry_id)): Path<(String, i64)>,
    ) -> Result<StatusCode, ApiError> {
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
        let result = state
            .1
            .lock()
            .await
            .storage
            .update_purchase_entry(
                &collection_id,
                entry_id,
                body.quantity,
                body.foil_quantity,
                body.normal_price_per_unit,
                body.foil_price_per_unit,
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
        let retrieval_systems = clone_retrieval_systems_by_name(&state).await;

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

        let mut card_info: HashMap<String, models::Card> = HashMap::new();
        for (provider, uuids) in &uuids_by_provider {
            if let Some(retrieval) = retrieval_systems.get(provider) {
                if let Ok(data) = retrieval.get_cards_by_ids(uuids.clone()).await {
                    card_info.extend(data);
                }
            }
        }

        let result = entries
            .into_iter()
            .map(|e| {
                let card = card_info.get(&e.card_uuid);
                CollectionPurchaseHistoryEntry {
                    id: e.id,
                    card_name: card.map(|c| card_name(c).to_string()),
                    set_code: card.map(|c| c.get_set()),
                    card_uuid: e.card_uuid,
                    quantity: e.quantity,
                    foil_quantity: e.foil_quantity,
                    normal_price_per_unit: e.normal_price_per_unit,
                    foil_price_per_unit: e.foil_price_per_unit,
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
        .api_route("/cards/{id}/list", get(cards_get))
        .api_route("/cards/{id}/count", get(collection_cards_count))
        .api_route("/cards/{id}/search", post(collection_cards_search))
        .api_route("/cards/{id}/search/count", post(collection_cards_search_count))
        .api_route("/search", post(search_temp))
        .api_route("/cards/{id}/add", post(cards_add))
        .api_route("/cards/{id}/delete", post(cards_remove))
        .api_route("/cards/{id}/purchase_history/{card_uuid}", get(purchase_history))
        .api_route("/cards/{id}/purchase_history", get(all_purchase_history))
        .api_route("/cards/{id}/purchase_history_entry/{entry_id}", delete(delete_purchase_entry).patch(update_purchase_entry))
        .api_route("/cards/{id}/value_breakdown", get(collection_value_breakdown))
        .route("/import", axum::routing::post(import))
        .route("/export/{id}", axum::routing::get(export))
}
