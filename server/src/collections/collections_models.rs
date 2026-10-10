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
    /// Collections only: each entry's current unit price for its finish, at
    /// the retailer it's valued at (see `CollectionValueBreakdown`). Prices
    /// in different currencies are compared as plain numbers; entries with no
    /// price come last in either order. Card searches sort by name instead.
    Price,
    /// Collections only: each entry's owned quantity. Card searches sort by
    /// name instead.
    Quantity,
    /// Collections only: each entry's wanted quantity. Card searches sort by
    /// name instead.
    WantQuantity,
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
            APISortField::Price | APISortField::Quantity | APISortField::WantQuantity => models::filters::SortField::Name,
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
    /// How to collapse results that share a card: one of the ids the system
    /// lists under `unique_modes` in `/api/system` (MTG: `prints`, `cards`,
    /// `art`). Omitted or empty means the system's default (`prints` for
    /// MTG). Systems that list no modes ignore it.
    pub unique: Option<String>,
}

/// One way a system can collapse search results that share a card; see
/// `APICardSearchFilters::unique`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct APIUniqueMode {
    /// Value to send as `unique`.
    pub id: String,
    /// Short name for a toggle.
    pub label: String,
    /// One-line explanation, for a tooltip.
    pub description: String,
}

impl From<models::filters::UniqueMode> for APIUniqueMode {
    fn from(value: models::filters::UniqueMode) -> Self {
        Self {
            id: value.id,
            label: value.label,
            description: value.description,
        }
    }
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
            unique: value.unique,
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Collection {
    pub id: String,
}

/// An entry of `/api/collection/list`.
#[derive(Serialize, JsonSchema)]
pub struct CollectionListEntry {
    pub id: String,
    /// False for the default collection, which can't be deleted (only
    /// emptied into another collection via `keepCardsInCollection`).
    pub removable: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct CollectionRemoveQuery {
    /// Move the collection's cards (and their purchase history) here instead
    /// of deleting them. Empty or omitted deletes them.
    #[serde(rename = "keepCardsInCollection", default)]
    pub keep_cards_in_collection: Option<String>,
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
    /// Which printing/finish this add/remove applies to — MTG's `""`
    /// (nonfoil) / `"foil"` / `"etched"`, a Pokemon variant like `"reverse
    /// holo"`, or just `""` for a game with a single version per collector
    /// number (Riftbound). Defaults to `""` (the primary/default finish).
    #[serde(default)]
    pub finish: String,
    pub quantity: i32,
    #[serde(rename = "purchasePrice", default)]
    pub purchase_price: Option<f64>,
    /// ISO 4217 code `purchasePrice` is in. Defaults to the server's
    /// `preferred_currency`.
    #[serde(rename = "purchaseCurrency", default)]
    pub purchase_currency: Option<String>,
    /// Which system/plugin this card came from, e.g. `RiftboundSQLite` or
    /// `plugin-dummy-books`. Optional so older callers keep working: when
    /// omitted, the server falls back to probing every configured system
    /// and plugin for the id — which is only safe when ids are unique
    /// across all of them. A client that knows the provider (the search UI
    /// always does, via whichever tab is active) should always send it.
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Deserialize, Debug, JsonSchema)]
pub struct AdjustWantQuantityRequest {
    pub id: String,
    pub delta: i32,
    /// See `CardToAdd::provider`.
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema, PartialEq)]
pub enum APICollectionSortField {
    #[default]
    TimeAdded,
    /// Sorts by each row's own quantity — since a row is now one specific
    /// finish of a card, this applies per finish, not to a single fixed
    /// "normal" bucket. There's no separate "foil quantity" sort field any
    /// more; filter to `finish == "foil"` client-side for that view.
    Quantity,
    WantQuantity,
    Provider,
}

impl From<APICollectionSortField> for persistence::CollectionSortField {
    fn from(value: APICollectionSortField) -> Self {
        match value {
            APICollectionSortField::TimeAdded => persistence::CollectionSortField::TimeAdded,
            APICollectionSortField::Quantity => persistence::CollectionSortField::Quantity,
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
    /// Which printing/finish this row tracks — see `CardToAdd::finish`.
    /// Distinct finishes of the same card `id` are separate rows; a client
    /// groups them back together by `id` to show "one card, several
    /// finishes" (e.g. nonfoil + foil).
    #[serde(default)]
    pub finish: String,
    pub quantity: i32,
    #[serde(rename = "wantQuantity", default)]
    pub want_quantity: i32,
    #[serde(rename = "collectionId")]
    pub collection_id: String,
    #[serde(rename = "timeAdded", default = "default_time_added")]
    #[schemars(default = "epoch")]
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
            finish: value.finish.clone(),
            quantity: value.quantity,
            want_quantity: value.want_quantity,
            collection: value.collection_id.to_string(),
            time_added: value.time_added.to_string(),
            provider: value.provider.clone(),
        }
    }
}

fn default_limit() -> usize {
    24
}

fn default_time_added() -> DateTime<Utc> {
    Utc::now()
}

/// Fixed placeholder used only for the schema's documented default (for determinism)
fn epoch() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
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
pub struct PriceHistoryResponse {
    /// Whether the server keeps price history at all. When it doesn't,
    /// `entries` is always empty.
    pub enabled: bool,
    /// Recorded prices, oldest first: one per retailer and finish per day.
    pub entries: Vec<persistence::PriceHistoryEntry>,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionValueBreakdown {
    /// Totals split by currency, largest total first. Every priced entry counts
    /// toward exactly one currency (that of its preferred retailer).
    pub currencies: Vec<CurrencyValue>,
    pub priced_count: usize,
    pub total_count: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CurrencyValue {
    /// ISO 4217 code, e.g. "USD", "EUR".
    pub currency: String,
    pub total_value: f64,
    pub profit: f64,
    pub untracked_value: f64,
    /// Total price of cards on the wishlist (`want_quantity`), independent of
    /// what's owned. Not included in `total_value`.
    pub wanted_value: f64,
    /// Owned entries priced in this currency.
    pub priced_count: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CollectionPurchaseHistoryEntry {
    pub id: i64,
    pub card_uuid: String,
    pub card_name: Option<String>,
    pub set_code: Option<String>,
    pub finish: String,
    pub quantity: i32,
    pub price_per_unit: Option<f64>,
    /// ISO 4217 code `price_per_unit` is in.
    pub currency: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct MoneyAmount {
    /// ISO 4217 code.
    pub currency: String,
    pub value: f64,
}

#[derive(Deserialize, JsonSchema)]
pub struct ValueHistoryQuery {
    /// Only days on or after this one (UTC, `YYYY-MM-DD`). Omitted means
    /// all recorded history.
    #[serde(default)]
    pub since: Option<chrono::NaiveDate>,
}

/// What the cards currently owned in a collection were worth on each day
/// price history has, at the prices they're valued at today (see
/// `CollectionValueBreakdown`): each card's history is read from the
/// retailer it's valued at now, so the last point lines up with the
/// collection's current total. Copies since sold or moved away are not
/// included — this is the value over time of what's held now.
#[derive(Serialize, JsonSchema)]
pub struct CollectionValueHistory {
    /// Whether the server keeps price history at all. When it doesn't,
    /// `currencies` is always empty.
    pub enabled: bool,
    /// Owned entries (one per card and finish) in the collection, priced or not.
    pub total_count: usize,
    /// One series per currency, largest latest value first. Every entry
    /// counts toward exactly one currency, that of its retailer.
    pub currencies: Vec<CurrencyValueHistory>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CurrencyValueHistory {
    /// ISO 4217 code.
    pub currency: String,
    /// Owned entries valued in this currency today.
    pub entry_count: usize,
    /// Oldest first, one per day any of those entries has a recorded price.
    pub points: Vec<ValueHistoryPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ValueHistoryPoint {
    /// UTC day.
    pub day: chrono::NaiveDate,
    /// Sum of quantity × unit price over the entries priced that day. A
    /// price carries forward to days with no new recording; an entry whose
    /// card has no price recorded yet contributes nothing.
    pub value: f64,
    /// Entries that contributed to `value`, out of `entry_count`.
    pub priced_count: usize,
}

#[derive(Deserialize, JsonSchema)]
pub struct ValueCardsQuery {
    /// How many days back `past_price` looks. Default 30.
    #[serde(default)]
    pub days: Option<u32>,
}

/// Every owned, priced entry of a collection with what it's worth, what it
/// cost and how its price moved — most valuable first.
#[derive(Serialize, JsonSchema)]
pub struct CollectionValueCards {
    /// Whether price history is kept; without it `first_price` and
    /// `past_price` are always missing.
    pub history_enabled: bool,
    /// The day `past_price` is as of (UTC).
    pub past_day: chrono::NaiveDate,
    /// Owned entries with no current price, so left out of `entries`.
    pub unpriced_count: usize,
    pub entries: Vec<ValueCardEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ValueCardEntry {
    pub card_uuid: String,
    pub provider: String,
    pub finish: String,
    /// `None` when the card's details couldn't be looked up.
    pub name: Option<String>,
    pub set_code: Option<String>,
    /// Direct image URL (Riftbound, Pokémon, plugins).
    pub image: Option<String>,
    /// MTG cards' image comes from Scryfall.
    pub scryfall_id: Option<String>,
    pub quantity: i32,
    /// ISO 4217 code of every price on this entry.
    pub currency: String,
    pub unit_price: f64,
    /// `unit_price` × `quantity`.
    pub total_value: f64,
    /// Owned copies that have a recorded purchase price.
    pub cost_quantity: i32,
    /// What those `cost_quantity` copies cost, per currency they were bought in.
    pub cost: Vec<MoneyAmount>,
    /// Current value of the `cost_quantity` copies minus `cost` — only when
    /// everything was bought in `currency`; otherwise convert `cost` and
    /// subtract it from `unit_price` × `cost_quantity`.
    pub profit: Option<f64>,
    /// Earliest recorded unit price.
    pub first_price: Option<DatedPrice>,
    /// Unit price as of `CollectionValueCards::past_day` (the latest
    /// recording on or before it). Missing when history starts later.
    pub past_price: Option<DatedPrice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct DatedPrice {
    /// UTC day it was recorded.
    pub day: chrono::NaiveDate,
    pub price: f64,
}
