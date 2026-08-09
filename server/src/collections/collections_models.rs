use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::mtg_api::mtg_api_models::{APICardColour, APIRarity};
use crate::pokemon_api::pokemon_api_models::APIEnergyType;
use crate::riftbound_api::riftbound_api_models::APICardDomain;
use persistence;

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema, PartialEq)]
pub enum APISortField {
    #[default]
    Name,
    Rarity,
    SetCode,
    CollectorNumber,
    Artist,
    /// Pokemon-only: card release date.
    ReleaseDate,
}

impl From<APISortField> for models::filters::SortField {
    fn from(value: APISortField) -> Self {
        match value {
            APISortField::Name => models::filters::SortField::Name,
            APISortField::Rarity => models::filters::SortField::Rarity,
            APISortField::SetCode => models::filters::SortField::SetCode,
            APISortField::CollectorNumber => models::filters::SortField::CollectorNumber,
            APISortField::Artist => models::filters::SortField::Artist,
            APISortField::ReleaseDate => models::filters::SortField::ReleaseDate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema, PartialEq)]
pub enum APISortOrder {
    #[default]
    Asc,
    Desc,
}

impl From<APISortOrder> for models::filters::SortOrder {
    fn from(value: APISortOrder) -> Self {
        match value {
            APISortOrder::Asc => models::filters::SortOrder::Asc,
            APISortOrder::Desc => models::filters::SortOrder::Desc,
        }
    }
}

/// Server-local version of `models::filters::CardSearchFilters` with `JsonSchema`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct APICardSearchFilters {
    pub name: Option<String>,
    #[serde(alias = "colorIdentities")]
    pub color_identities: Option<Vec<APICardColour>>,
    #[serde(alias = "setCode")]
    pub set_code: Option<String>,
    #[serde(alias = "collectorNumber")]
    pub collector_number: Option<String>,
    pub artist: Option<String>,
    pub text: Option<String>,
    #[serde(default, deserialize_with = "empty_string_to_none")]
    pub rarity: Option<APIRarity>,
    pub subtypes: Option<Vec<String>>,
    pub supertypes: Option<String>,
    pub types: Option<Vec<String>>,
    #[serde(alias = "manaValueMin")]
    pub mana_value_min: Option<f64>,
    #[serde(alias = "manaValueMax")]
    pub mana_value_max: Option<f64>,
    pub colors: Option<Vec<APICardColour>>,
    pub keywords: Option<Vec<String>>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    #[serde(alias = "isReserved")]
    pub is_reserved: Option<bool>,
    #[serde(alias = "isPromo")]
    pub is_promo: Option<bool>,
    #[serde(alias = "isReprint")]
    pub is_reprint: Option<bool>,
    #[serde(alias = "isFullArt")]
    pub is_full_art: Option<bool>,
    #[serde(alias = "borderColor")]
    pub border_color: Option<String>,
    #[serde(alias = "legalIn")]
    pub legal_in: Option<String>,
    pub domains: Option<Vec<APICardDomain>>,
    #[serde(alias = "energyTypes")]
    pub energy_types: Option<Vec<APIEnergyType>>,
    pub pokedex: Option<i64>,
    #[serde(alias = "sortBy")]
    pub sort_by: Option<APISortField>,
    #[serde(alias = "sortOrder")]
    pub sort_order: Option<APISortOrder>,
}

impl From<APICardSearchFilters> for models::filters::CardSearchFilters {
    fn from(value: APICardSearchFilters) -> Self {
        models::filters::CardSearchFilters {
            name: value.name,
            color_identities: value
                .color_identities
                .map(|v| v.into_iter().map(models::CardColour::from).collect()),
            set_code: value.set_code,
            collector_number: value.collector_number,
            artist: value.artist,
            text: value.text,
            rarity: value.rarity.map(models::Rarity::from),
            subtypes: value.subtypes,
            supertypes: value.supertypes,
            types: value.types,
            mana_value_min: value.mana_value_min,
            mana_value_max: value.mana_value_max,
            colors: value
                .colors
                .map(|v| v.into_iter().map(models::CardColour::from).collect()),
            keywords: value.keywords,
            power: value.power,
            toughness: value.toughness,
            loyalty: value.loyalty,
            defense: value.defense,
            is_reserved: value.is_reserved,
            is_promo: value.is_promo,
            is_reprint: value.is_reprint,
            is_full_art: value.is_full_art,
            border_color: value.border_color,
            legal_in: value.legal_in,
            domains: value.domains.map(|v| {
                v.into_iter()
                    .map(models::riftbound::CardDomain::from)
                    .collect()
            }),
            energy_types: value.energy_types.map(|v| {
                v.into_iter()
                    .map(models::pokemon::EnergyType::from)
                    .collect()
            }),
            pokedex: value.pokedex,
            sort_by: value.sort_by.map(models::filters::SortField::from),
            sort_order: value.sort_order.map(models::filters::SortOrder::from),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Collection {
    pub id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionAddResponse {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionRemoveResponse {
    pub message: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CollectionRenameRequest {
    pub new_id: String,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct CardToAdd {
    pub id: String,
    pub quantity: i32,
    #[serde(rename = "foilQuantity")]
    pub foil_quantity: i32,
    #[serde(rename = "purchasePrice", default)]
    pub purchase_price: Option<f64>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct SetWantQuantityRequest {
    pub id: String,
    #[serde(rename = "wantQuantity")]
    pub want_quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema, PartialEq)]
pub enum APICollectionSortField {
    #[default]
    TimeAdded,
    Quantity,
    FoilQuantity,
    WantQuantity,
    Provider,
}

impl From<APICollectionSortField> for persistence::CollectionSortField {
    fn from(value: APICollectionSortField) -> Self {
        match value {
            APICollectionSortField::TimeAdded => persistence::CollectionSortField::TimeAdded,
            APICollectionSortField::Quantity => persistence::CollectionSortField::Quantity,
            APICollectionSortField::FoilQuantity => persistence::CollectionSortField::FoilQuantity,
            APICollectionSortField::WantQuantity => persistence::CollectionSortField::WantQuantity,
            APICollectionSortField::Provider => persistence::CollectionSortField::Provider,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CollectionCardsQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub sort_by: Option<APICollectionSortField>,
    pub sort_order: Option<APISortOrder>,
    /// Single provider inclusion filter (legacy, takes precedence over `providers`).
    pub provider: Option<String>,
    /// Multiple provider inclusion filter — comma-separated: `?providers=X,Y`.
    #[serde(default)]
    pub providers: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CollectionCard {
    pub id: String,
    pub quantity: i32,
    #[serde(rename = "foilQuantity")]
    pub foil_quantity: i32,
    #[serde(rename = "wantQuantity", default)]
    pub want_quantity: i32,
    #[serde(rename = "collectionId")]
    pub collection_id: String,
    #[serde(rename = "timeAdded")]
    pub time_added: DateTime<Utc>,
    #[serde(default)]
    pub provider: String,
}

/// A page of full card data for a shareable, read-only collection view.
/// Each entry in `cards` merges the collection entry (quantity, provider, ...)
/// with the full card details, so the client needs a single request per page
/// instead of a separate card-detail lookup.
#[derive(Serialize, JsonSchema)]
pub struct PublicCollectionPage {
    pub cards: Vec<serde_json::Value>,
    pub total: usize,
}

/// A shareable, read-only link granting public access to a collection via
/// its opaque `token`. Owner-managed: created and revoked explicitly through
/// the `/collection/share/{id}` endpoints.
#[derive(Serialize, JsonSchema)]
pub struct ShareLinkResponse {
    pub token: String,
    #[serde(rename = "collectionId")]
    pub collection_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

impl From<persistence::ShareLink> for ShareLinkResponse {
    fn from(value: persistence::ShareLink) -> Self {
        Self {
            token: value.token,
            collection_id: value.collection_id,
            created_at: value.created_at,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct ShareLinkRevokeResponse {
    pub revoked: bool,
}

impl From<&CollectionCard> for models::CollectionCard {
    fn from(value: &CollectionCard) -> Self {
        models::CollectionCard {
            uuid: value.id.to_string(),
            quantity: value.quantity,
            foil_quantity: value.foil_quantity,
            want_quantity: value.want_quantity,
            collection: value.collection_id.to_string(),
            time_added: value.time_added.to_string(),
            provider: "".to_string(),
        }
    }
}

fn default_limit() -> usize {
    24
}

#[derive(Deserialize, JsonSchema)]
pub struct CollectionsSearchQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub page_size: usize,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct CardIdentInner {
    #[serde(rename = "scryfallId")]
    pub scryfall_id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ResultCardInner {
    pub id: String,
    pub name: String,
    #[serde(rename = "setCode")]
    pub set_code: String,
    #[serde(rename = "cardIdentifiers")]
    pub card_identifiers: CardIdentInner,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ResultCard {
    #[serde(rename = "mtGCard")]
    pub mtg_card: ResultCardInner,
}

#[derive(Serialize, JsonSchema)]
pub struct PurchaseHistoryResponse {
    pub entries: Vec<persistence::PurchaseHistoryEntry>,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionValueBreakdown {
    pub total_value: f64,
    pub profit: f64,
    pub untracked_value: f64,
    pub priced_count: usize,
    pub total_count: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionPurchaseHistoryEntry {
    pub id: i64,
    pub card_uuid: String,
    pub card_name: Option<String>,
    pub set_code: Option<String>,
    pub quantity: i32,
    pub foil_quantity: i32,
    pub normal_price_per_unit: Option<f64>,
    pub foil_price_per_unit: Option<f64>,
    pub provider: String,
    pub recorded_at: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionAllPurchaseHistoryResponse {
    pub entries: Vec<CollectionPurchaseHistoryEntry>,
}

fn empty_string_to_none<'de, D>(deserializer: D) -> Result<Option<APIRarity>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(match opt.as_deref() {
        Some("") | None => None,
        Some(s) => {
            Some(serde_json::from_str(&format!("\"{}\"", s)).map_err(serde::de::Error::custom)?)
        }
    })
}
