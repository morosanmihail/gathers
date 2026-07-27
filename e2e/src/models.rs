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
