use eyre::{Context, bail};
use reqwest::StatusCode;

use crate::models::{
    AllPurchaseHistoryResponse, CardToAdd, Collection, CollectionAddResponse, CollectionCard,
    CollectionRemoveResponse, PublicCollectionPage, PurchaseHistoryResponse, ShareLink,
    ShareLinkRevokeResponse,
};

/// HTTP client for the GatheRs server.
///
/// All methods return `eyre::Result`. On a non-2xx response the body is
/// included in the error message to aid debugging.
pub struct GathersClient {
    base_url: String,
    client: reqwest::Client,
}

impl GathersClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ── collections ──────────────────────────────────────────────────────────

    pub async fn list_collections(&self) -> eyre::Result<Vec<Collection>> {
        self.get("/api/collection/list").await
    }

    pub async fn add_collection(&self, name: &str) -> eyre::Result<CollectionAddResponse> {
        self.post("/api/collection/add", &Collection { id: name.to_string() })
            .await
    }

    pub async fn remove_collection(&self, id: &str) -> eyre::Result<CollectionRemoveResponse> {
        self.post_empty(&format!("/api/collection/remove/{}", urlenc(id)))
            .await
    }

    // ── cards in a collection ─────────────────────────────────────────────────

    pub async fn list_cards(&self, collection_id: &str) -> eyre::Result<Vec<CollectionCard>> {
        self.get(&format!(
            "/api/collection/cards/{}/list?limit=1000",
            urlenc(collection_id)
        ))
        .await
    }

    pub async fn card_count(&self, collection_id: &str) -> eyre::Result<usize> {
        self.get(&format!(
            "/api/collection/cards/{}/count",
            urlenc(collection_id)
        ))
        .await
    }

    /// Add (or subtract when qty is negative) cards to a collection.
    /// Pass `purchase_price` to record a purchase history entry.
    pub async fn add_cards(
        &self,
        collection_id: &str,
        card_id: &str,
        quantity: i32,
        foil_quantity: i32,
        purchase_price: Option<f64>,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!("/api/collection/cards/{}/add", urlenc(collection_id)),
            &CardToAdd {
                id: card_id.to_string(),
                quantity,
                foil_quantity,
                purchase_price,
            },
        )
        .await
    }

    /// Remove cards from a collection (positive quantities = how many to remove).
    pub async fn remove_cards(
        &self,
        collection_id: &str,
        card_id: &str,
        quantity: i32,
        foil_quantity: i32,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!("/api/collection/cards/{}/delete", urlenc(collection_id)),
            &CardToAdd {
                id: card_id.to_string(),
                quantity,
                foil_quantity,
                purchase_price: None,
            },
        )
        .await
    }

    /// Move cards from their source collection (encoded in each `CollectionCard`)
    /// to `to_collection_id`.
    pub async fn move_cards(
        &self,
        to_collection_id: &str,
        cards: &[CollectionCard],
    ) -> eyre::Result<()> {
        self.post(
            &format!("/api/collection/move/{}", urlenc(to_collection_id)),
            cards,
        )
        .await
    }

    // ── shareable read-only view ────────────────────────────────────────────

    /// One page of the read-only, shareable collection view — full card data
    /// merged with collection metadata, in a single request. `token` is a
    /// share link minted via `create_share_link`, not the collection id.
    pub async fn public_cards(
        &self,
        token: &str,
        offset: usize,
        limit: usize,
    ) -> eyre::Result<PublicCollectionPage> {
        self.get(&format!(
            "/api/share/{}?offset={offset}&limit={limit}",
            urlenc(token)
        ))
        .await
    }

    // ── share link management (owner-only) ──────────────────────────────────

    pub async fn create_share_link(&self, collection_id: &str) -> eyre::Result<ShareLink> {
        self.post_empty(&format!("/api/collection/share/{}", urlenc(collection_id)))
            .await
    }

    pub async fn list_share_links(&self, collection_id: &str) -> eyre::Result<Vec<ShareLink>> {
        self.get(&format!("/api/collection/share/{}", urlenc(collection_id)))
            .await
    }

    pub async fn revoke_share_link(
        &self,
        collection_id: &str,
        token: &str,
    ) -> eyre::Result<ShareLinkRevokeResponse> {
        self.delete(&format!(
            "/api/collection/share/{}/{}",
            urlenc(collection_id),
            urlenc(token)
        ))
        .await
    }

    // ── purchase history ──────────────────────────────────────────────────────

    pub async fn purchase_history(
        &self,
        collection_id: &str,
        card_id: &str,
    ) -> eyre::Result<PurchaseHistoryResponse> {
        self.get(&format!(
            "/api/collection/cards/{}/purchase_history/{}",
            urlenc(collection_id),
            urlenc(card_id),
        ))
        .await
    }

    pub async fn all_purchase_history(
        &self,
        collection_id: &str,
    ) -> eyre::Result<AllPurchaseHistoryResponse> {
        self.get(&format!(
            "/api/collection/cards/{}/purchase_history",
            urlenc(collection_id),
        ))
        .await
    }

    // ── low-level helpers ─────────────────────────────────────────────────────

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> eyre::Result<T> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        parse_response(resp, "GET", &url).await
    }

    async fn post<B: serde::Serialize + ?Sized, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> eyre::Result<T> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        parse_response(resp, "POST", &url).await
    }

    async fn post_empty<T: serde::de::DeserializeOwned>(&self, path: &str) -> eyre::Result<T> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Content-Length", "0")
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        parse_response(resp, "POST", &url).await
    }

    async fn delete<T: serde::de::DeserializeOwned>(&self, path: &str) -> eyre::Result<T> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .with_context(|| format!("DELETE {url}"))?;
        parse_response(resp, "DELETE", &url).await
    }
}

async fn parse_response<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    method: &str,
    url: &str,
) -> eyre::Result<T> {
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!("{method} {url} → {status}: {body}");
    }
    // 204 No Content or unit response: try to deserialize "()" from an empty/null body
    if status == StatusCode::NO_CONTENT {
        return serde_json::from_str("null").with_context(|| format!("{method} {url} deserialize"));
    }
    let bytes = resp.bytes().await.with_context(|| format!("{method} {url} read body"))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("{method} {url} deserialize: {}", String::from_utf8_lossy(&bytes)))
}

fn urlenc(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            ' ' => vec!['%', '2', '0'],
            '/' => vec!['%', '2', 'F'],
            '#' => vec!['%', '2', '3'],
            '?' => vec!['%', '3', 'F'],
            _ => vec![c],
        })
        .collect()
}
