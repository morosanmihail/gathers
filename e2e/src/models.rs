use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionAddResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRemoveResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub time_added: String,
    #[serde(default)]
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardToAdd {
    pub id: String,
    pub quantity: i32,
    #[serde(rename = "foilQuantity")]
    pub foil_quantity: i32,
    #[serde(rename = "purchasePrice")]
    pub purchase_price: Option<f64>,
    /// Which system/plugin this card came from. When omitted, the server
    /// falls back to probing every configured system and plugin for the
    /// id — see `plugin_provider_resolution.rs`.
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustWantQuantityRequest {
    pub id: String,
    pub delta: i32,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseHistoryResponse {
    pub entries: Vec<PurchaseHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllPurchaseHistoryResponse {
    pub entries: Vec<CollectionPurchaseHistoryEntry>,
}

/// Response from the read-only `/api/share/{token}` endpoint. Each entry in
/// `cards` is a full card (whichever provider it belongs to) merged with its
/// collection metadata (quantity, provider, ...) — the shape varies by
/// provider, so it's left as raw JSON rather than a fixed struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicCollectionPage {
    pub cards: Vec<serde_json::Value>,
    pub total: usize,
}

/// A shareable, read-only link granting public access to a collection via
/// its opaque `token`, minted through `/api/collection/share/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLink {
    pub token: String,
    #[serde(rename = "collectionId")]
    pub collection_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLinkRevokeResponse {
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionValueBreakdown {
    pub total_value: f64,
    pub profit: f64,
    pub untracked_value: f64,
    pub priced_count: usize,
    pub total_count: usize,
    pub wanted_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressInfo {
    pub downloaded: u64,
    pub total: u64,
    pub phase: String,
}

/// Response from `GET /api/system`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub system: String,
    pub systems: Vec<String>,
    pub plugins: Vec<String>,
    pub downloading: HashMap<String, DownloadProgressInfo>,
    pub demo_mode: bool,
    pub pricing_enabled: bool,
    pub collections_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub base_url: String,
}

/// A card served by a third-party plugin (see `retrieval::systems::plugin`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCard {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub set_code: String,
    #[serde(default)]
    pub set_name: String,
    #[serde(default)]
    pub collector_number: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}
