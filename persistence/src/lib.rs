mod csv_models;
mod import_export;
mod sqlite;

use enum_dispatch::enum_dispatch;
use models::CardID;
use models::CollectionCard;
use models::CollectionID;
use models::filters::SortOrder;

pub use crate::csv_models::{CsvField, CsvFieldMapping};
pub use crate::sqlite::{SQLitePersistenceSystem, default_price_history_path};

/// Errors caused by the request rather than by storage itself, so callers
/// can tell "you asked for something invalid" apart from a database failure
/// (e.g. to answer 4xx instead of 500). Returned inside an `eyre::Report`;
/// recover it with `report.downcast_ref::<PersistenceError>()`.
#[derive(Debug, Clone, PartialEq)]
pub enum PersistenceError {
    CollectionNotFound(CollectionID),
    CollectionExists(CollectionID),
    /// The collection is protected (the default collection) and can only be
    /// emptied into another collection, never deleted.
    CollectionNotRemovable(CollectionID),
    InvalidInput(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CollectionNotFound(name) => write!(f, "Collection '{name}' not found"),
            Self::CollectionExists(name) => write!(f, "Collection '{name}' already exists"),
            Self::CollectionNotRemovable(name) => write!(
                f,
                "Collection '{name}' is the default collection and can't be deleted; move its cards to another collection instead"
            ),
            Self::InvalidInput(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for PersistenceError {}

/// Largest per-unit price accepted for a purchase. Far above any real card,
/// but low enough that totals over many copies stay finite.
pub const MAX_UNIT_PRICE: f64 = 1_000_000_000.0;

/// Checks a per-unit purchase price is a real, non-negative amount no larger
/// than `MAX_UNIT_PRICE`.
pub fn validate_price(price: f64) -> Result<(), String> {
    if price.is_finite() && (0.0..=MAX_UNIT_PRICE).contains(&price) {
        Ok(())
    } else {
        Err(format!("Price must be between 0 and {MAX_UNIT_PRICE}"))
    }
}

/// Checks a currency is a 3-letter uppercase ISO 4217 code like "EUR".
pub fn validate_currency(currency: &str) -> Result<(), String> {
    if currency.len() == 3 && currency.chars().all(|c| c.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(format!("Currency must be a 3-letter ISO 4217 code like EUR or USD, got '{currency}'"))
    }
}

/// A collection's name plus whether it may be deleted.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionInfo {
    pub name: CollectionID,
    pub removable: bool,
}

#[derive(Debug, Default, Clone)]
pub enum CollectionSortField {
    #[default]
    TimeAdded,
    /// Sorts by each row's own `quantity` — since a row is now one
    /// specific `finish` of a card (see `models::CollectionCard`), this
    /// applies per finish rather than to a single fixed "normal" bucket.
    Quantity,
    WantQuantity,
    Provider,
}

#[derive(Debug, Default, Clone)]
pub struct CollectionCardsParams {
    pub offset: usize,
    pub limit: usize,
    pub sort_by: Option<CollectionSortField>,
    pub sort_order: Option<SortOrder>,
    /// Filter to exactly one provider.
    pub provider: Option<String>,
    /// Filter to any of these providers (ignored if `provider` is set).
    pub providers: Vec<String>,
    /// Hides rows from plugins that aren't currently enabled. `Some(list)`
    /// keeps only plugin rows (`plugin-*` providers) whose provider is in
    /// `list` — pass the enabled plugins' providers, so an empty list hides
    /// every plugin row. `None` applies no such restriction. Non-plugin
    /// rows are never affected.
    pub enabled_plugin_providers: Option<Vec<String>>,
}

impl CollectionCardsParams {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit,
            sort_by: None,
            sort_order: None,
            provider: None,
            providers: vec![],
            enabled_plugin_providers: None,
        }
    }
}

#[enum_dispatch]
#[derive(Debug, Clone)]
pub enum PersistenceSystem {
    SQLitePersistenceSystem,
}

#[enum_dispatch(PersistenceSystem)]
pub trait PersistenceSystemTrait {
    fn add_collection(
        &mut self,
        name: CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<String>>;

    /// Deletes a collection along with its cards, purchase history and share
    /// links — or, with `move_to`, first merges its cards and purchase
    /// history into that collection. The default collection can't be
    /// deleted: with `move_to` it's emptied into the target and kept,
    /// without it this fails with `PersistenceError::CollectionNotRemovable`
    /// and nothing changes.
    fn remove_collection(
        &mut self,
        name: &CollectionID,
        move_to: Option<CollectionID>,
    ) -> impl std::future::Future<Output = eyre::Result<CollectionID>>;

    /// Renames a collection, carrying its cards, purchase history and share
    /// links over. Fails with `CollectionNotFound` / `CollectionExists`.
    fn rename_collection(
        &mut self,
        old_name: &CollectionID,
        new_name: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<()>>;

    fn list_collections(
        &self,
        filter: Option<String>,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CollectionID>>>;

    fn list_collection_info(
        &self,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CollectionInfo>>>;

    fn get_cards_in_collection_count(
        &self,
        collection_id: CollectionID,
        providers: &[String],
        enabled_plugin_providers: Option<&[String]>,
    ) -> impl std::future::Future<Output = eyre::Result<usize>>;

    fn add_card_to_collection(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        finish: &str,
        quantity: i32,
        time_added: &str,
        provider: &str,
    ) -> impl std::future::Future<Output = eyre::Result<CollectionCard>>;

    fn add_cards_to_collection(
        &mut self,
        collection_id: &CollectionID,
        cards: &[CollectionCard],
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CollectionCard>>>;

    fn get_cards_in_collection_paginated(
        &self,
        collection_id: &CollectionID,
        params: CollectionCardsParams,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CollectionCard>>>;

    /// Adjusts (by delta, floored at 0) the quantity of a card the owner wants
    /// to acquire in a collection. Same delta model as `add_card_to_collection`,
    /// and works even if the card isn't owned yet (a wishlist entry).
    fn adjust_want_quantity(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        delta: i32,
        provider: &str,
    ) -> impl std::future::Future<Output = eyre::Result<CollectionCard>>;

    /// Moves each entry's `quantity`/`want_quantity` of that card+finish from
    /// its `collection` to `to_collection_id`. Only what the source actually
    /// holds is moved — a larger (or negative) request is clamped to it — so
    /// a move can never create cards. The destination must exist
    /// (`CollectionNotFound` otherwise).
    fn move_cards_between_collections(
        &mut self,
        cards: &[CollectionCard],
        to_collection_id: CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<()>>;

    fn record_purchase(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        finish: &str,
        quantity: i32,
        price_per_unit: Option<f64>,
        currency: &str,
        provider: &str,
        recorded_at: &str,
    ) -> impl std::future::Future<Output = eyre::Result<()>>;

    fn get_purchase_history(
        &self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<PurchaseHistoryEntry>>>;

    fn get_all_purchase_history(
        &self,
        collection_id: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<PurchaseHistoryEntry>>>;

    /// Keyed by `(card_uuid, finish)` — each finish of a card has its own
    /// cost basis, one summary per currency it was bought in.
    fn get_collection_purchase_totals(
        &self,
        collection_id: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<std::collections::HashMap<(CardID, String), Vec<PurchaseSummary>>>>;

    fn delete_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
    ) -> impl std::future::Future<Output = eyre::Result<bool>>;

    /// The entry's `finish` is fixed at creation and can't be changed here
    /// — only how many copies (of that same finish), at what price and, when
    /// `currency` is given, in which currency.
    fn update_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
        quantity: i32,
        price_per_unit: Option<f64>,
        currency: Option<&str>,
    ) -> impl std::future::Future<Output = eyre::Result<UpdateEntryResult>>;

    /// Explicitly grants read-only public access to a collection by minting
    /// a new, unguessable share token. This is the only way a collection
    /// becomes reachable through the public share endpoint.
    fn create_share_link(
        &mut self,
        collection_id: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<ShareLink>>;

    fn list_share_links(
        &self,
        collection_id: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<ShareLink>>>;

    /// Invalidates a share link. Returns `false` if the token didn't exist
    /// (or belonged to a different collection).
    fn revoke_share_link(
        &mut self,
        collection_id: &CollectionID,
        token: &str,
    ) -> impl std::future::Future<Output = eyre::Result<bool>>;

    /// Resolves a share token to its collection id, if the token is valid
    /// (exists and hasn't been revoked).
    fn resolve_share_link(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = eyre::Result<Option<CollectionID>>>;

    /// Whether price history is being kept (see
    /// `SQLitePersistenceSystem::enable_price_history`). When it isn't, the
    /// other price history methods do nothing and return nothing.
    fn price_history_enabled(&self) -> bool;

    /// Every distinct card, in any collection, stored under `provider` —
    /// owned or only wanted.
    fn tracked_card_uuids(
        &self,
        provider: &str,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CardID>>>;

    /// Whether any prices have been recorded for `provider` yet.
    fn has_price_history(
        &self,
        provider: &str,
    ) -> impl std::future::Future<Output = eyre::Result<bool>>;

    /// Stores `prices`, each under its own `recorded_on` day, replacing
    /// anything already recorded for the same card, retailer, finish and
    /// day. Non-finite and non-positive prices are skipped. Returns how many
    /// prices were written.
    fn record_prices(
        &mut self,
        provider: &str,
        prices: &[PricePoint],
    ) -> impl std::future::Future<Output = eyre::Result<usize>>;

    /// A card's recorded prices, oldest first.
    fn get_price_history(
        &self,
        provider: &str,
        card_uuid: &CardID,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<PriceHistoryEntry>>>;
}

/// One retailer's current price for one finish of a card, to be recorded
/// in price history.
#[derive(Debug, Clone, PartialEq)]
pub struct PricePoint {
    pub card_uuid: CardID,
    pub retailer: String,
    /// Same convention as `models::CollectionCard::finish`: `""` is the
    /// default finish, anything else is game-specific.
    pub finish: String,
    pub price: f64,
    /// ISO 4217 code `price` is in.
    pub currency: String,
    /// UTC day the price is as of.
    pub recorded_on: chrono::NaiveDate,
}

/// One retailer's price for one finish of a card on a given day.
#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
pub struct PriceHistoryEntry {
    pub retailer: String,
    /// Same convention as `models::CollectionCard::finish`: `""` is the
    /// default finish, anything else is game-specific.
    pub finish: String,
    pub price: f64,
    /// ISO 4217 code `price` is in.
    pub currency: String,
    /// UTC day the price is as of.
    pub recorded_on: chrono::NaiveDate,
}

/// A single shareable, read-only link granting public access to a collection.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ShareLink {
    pub token: String,
    pub collection_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEntryResult {
    Updated,
    NotFound,
    ValidationError(String),
}

#[derive(Debug, Clone)]
pub struct PurchaseSummary {
    /// ISO 4217 code `total_paid` is in.
    pub currency: String,
    pub total_paid: f64,
    pub quantity: i32,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct PurchaseHistoryEntry {
    pub id: i64,
    pub card_uuid: String,
    pub finish: String,
    pub quantity: i32,
    pub price_per_unit: Option<f64>,
    /// ISO 4217 code `price_per_unit` is in.
    pub currency: String,
    pub provider: String,
    pub recorded_at: String,
}
