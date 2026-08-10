mod csv_models;
mod import_export;
mod sqlite;

use enum_dispatch::enum_dispatch;
use models::CardID;
use models::CollectionCard;
use models::CollectionID;
use models::filters::SortOrder;

pub use crate::sqlite::SQLitePersistenceSystem;

#[derive(Debug, Default, Clone)]
pub enum CollectionSortField {
    #[default]
    TimeAdded,
    Quantity,
    FoilQuantity,
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

    fn remove_collection(
        &mut self,
        name: &CollectionID,
        move_to: Option<CollectionID>,
    ) -> impl std::future::Future<Output = eyre::Result<CollectionID>>;

    fn rename_collection(
        &mut self,
        old_name: &CollectionID,
        new_name: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<()>>;

    fn list_collections(
        &self,
        filter: Option<String>,
    ) -> impl std::future::Future<Output = eyre::Result<Vec<CollectionID>>>;

    fn get_cards_in_collection_count(
        &self,
        collection_id: CollectionID,
        providers: &[String],
    ) -> impl std::future::Future<Output = eyre::Result<usize>>;

    fn add_card_to_collection(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        quantity: i32,
        foil_quantity: i32,
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

    fn move_cards_between_collections(
        &mut self,
        cards: &[CollectionCard],
        to_collection_id: CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<()>>;

    fn record_purchase(
        &mut self,
        collection_id: &CollectionID,
        card_uuid: &CardID,
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
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

    fn get_collection_purchase_totals(
        &self,
        collection_id: &CollectionID,
    ) -> impl std::future::Future<Output = eyre::Result<std::collections::HashMap<CardID, PurchaseSummary>>>;

    fn delete_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
    ) -> impl std::future::Future<Output = eyre::Result<bool>>;

    fn update_purchase_entry(
        &mut self,
        collection_id: &CollectionID,
        entry_id: i64,
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
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
    pub total_normal_paid: f64,
    pub total_foil_paid: f64,
    pub quantity: i32,
    pub foil_quantity: i32,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct PurchaseHistoryEntry {
    pub id: i64,
    pub card_uuid: String,
    pub quantity: i32,
    pub foil_quantity: i32,
    pub normal_price_per_unit: Option<f64>,
    pub foil_price_per_unit: Option<f64>,
    pub provider: String,
    pub recorded_at: String,
}
