use std::collections::HashMap;

use eyre::{Context, bail};
use reqwest::StatusCode;

use crate::models::{
    AdjustWantQuantityRequest, AllPurchaseHistoryResponse, CardToAdd, Collection,
    CollectionAddResponse, CollectionRemoveResponse, CollectionCard, CollectionValueBreakdown,
    PluginCard, PluginSummary, PublicCollectionPage, PurchaseHistoryResponse, ShareLink,
    ShareLinkRevokeResponse, SystemInfo,
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

    pub async fn rename_collection(&self, id: &str, new_id: &str) -> eyre::Result<Collection> {
        self.post(
            &format!("/api/collection/rename/{}", urlenc(id)),
            &serde_json::json!({ "new_id": new_id }),
        )
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
        self.add_cards_with_provider(collection_id, card_id, quantity, foil_quantity, purchase_price, None)
            .await
    }

    /// Like `add_cards`, but names which system/plugin the card came from
    /// instead of letting the server guess by probing every configured one —
    /// required when an id isn't unique across all of them (see
    /// `plugin_provider_resolution.rs`). Pass `provider: None` for the old
    /// probe-everything behavior.
    pub async fn add_cards_with_provider(
        &self,
        collection_id: &str,
        card_id: &str,
        quantity: i32,
        foil_quantity: i32,
        purchase_price: Option<f64>,
        provider: Option<&str>,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!("/api/collection/cards/{}/add", urlenc(collection_id)),
            &CardToAdd {
                id: card_id.to_string(),
                quantity,
                foil_quantity,
                purchase_price,
                provider: provider.map(str::to_string),
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
                provider: None,
            },
        )
        .await
    }

    /// Adjust (by delta, floored at 0) the quantity of a card the collection
    /// owner wants to acquire, independent of owned quantity/foil_quantity.
    pub async fn adjust_want(
        &self,
        collection_id: &str,
        card_id: &str,
        delta: i32,
    ) -> eyre::Result<CollectionCard> {
        self.adjust_want_with_provider(collection_id, card_id, delta, None).await
    }

    /// Like `adjust_want`, but see `add_cards_with_provider` — only used
    /// when the card doesn't already have a row in the collection; an
    /// existing row keeps its own provider regardless.
    pub async fn adjust_want_with_provider(
        &self,
        collection_id: &str,
        card_id: &str,
        delta: i32,
        provider: Option<&str>,
    ) -> eyre::Result<CollectionCard> {
        self.post(
            &format!("/api/collection/cards/{}/want", urlenc(collection_id)),
            &AdjustWantQuantityRequest {
                id: card_id.to_string(),
                delta,
                provider: provider.map(str::to_string),
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

    /// Update a purchase history entry in place. Returns the response status
    /// (204 on success, 404 if the entry doesn't exist, 400 on a validation
    /// error such as recording more copies than the collection owns) plus
    /// the response body text (non-empty only for a 400).
    pub async fn update_purchase_entry(
        &self,
        collection_id: &str,
        entry_id: i64,
        quantity: i32,
        foil_quantity: i32,
        normal_price_per_unit: Option<f64>,
        foil_price_per_unit: Option<f64>,
    ) -> eyre::Result<(StatusCode, String)> {
        let url = format!(
            "{}/api/collection/cards/{}/purchase_history_entry/{entry_id}",
            self.base_url,
            urlenc(collection_id)
        );
        let resp = self
            .client
            .patch(&url)
            .json(&serde_json::json!({
                "quantity": quantity,
                "foil_quantity": foil_quantity,
                "normal_price_per_unit": normal_price_per_unit,
                "foil_price_per_unit": foil_price_per_unit,
            }))
            .send()
            .await
            .with_context(|| format!("PATCH {url}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Ok((status, body))
    }

    /// Delete a purchase history entry. Returns the response status (204 if
    /// removed, 404 if no such entry existed).
    pub async fn delete_purchase_entry(
        &self,
        collection_id: &str,
        entry_id: i64,
    ) -> eyre::Result<StatusCode> {
        let url = format!(
            "{}/api/collection/cards/{}/purchase_history_entry/{entry_id}",
            self.base_url,
            urlenc(collection_id)
        );
        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .with_context(|| format!("DELETE {url}"))?;
        Ok(resp.status())
    }

    // ── collection value & search ───────────────────────────────────────────

    pub async fn value_breakdown(&self, collection_id: &str) -> eyre::Result<CollectionValueBreakdown> {
        self.get(&format!(
            "/api/collection/cards/{}/value_breakdown",
            urlenc(collection_id)
        ))
        .await
    }

    /// Search within a collection's cards using the same filter shape as the
    /// global card search endpoints (see `mtg_search`), scoped to just this
    /// collection's owned/wanted entries.
    pub async fn search_collection_cards(
        &self,
        collection_id: &str,
        filters: &serde_json::Value,
        offset: usize,
        limit: usize,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!(
                "/api/collection/cards/{}/search?offset={offset}&limit={limit}",
                urlenc(collection_id)
            ),
            filters,
        )
        .await
    }

    pub async fn search_collection_cards_count(
        &self,
        collection_id: &str,
        filters: &serde_json::Value,
    ) -> eyre::Result<usize> {
        self.post(
            &format!("/api/collection/cards/{}/search/count", urlenc(collection_id)),
            filters,
        )
        .await
    }

    // ── CSV import/export ───────────────────────────────────────────────────

    pub async fn export_csv(&self, collection_id: &str) -> eyre::Result<String> {
        let url = format!("{}/api/collection/export/{}", self.base_url, urlenc(collection_id));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GET {url} → {status}: {body}");
        }
        resp.text().await.with_context(|| format!("GET {url} read body"))
    }

    pub async fn import_csv(&self, collection_name: &str, csv: &str) -> eyre::Result<()> {
        let url = format!("{}/api/collection/import", self.base_url);
        let part = reqwest::multipart::Part::bytes(csv.as_bytes().to_vec())
            .file_name("import.csv")
            .mime_str("text/csv")?;
        let form = reqwest::multipart::Form::new()
            .text("collection", collection_name.to_string())
            .part("file", part);
        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        parse_response(resp, "POST", &url).await
    }

    // ── system info ─────────────────────────────────────────────────────────

    pub async fn system_info(&self) -> eyre::Result<SystemInfo> {
        self.get("/api/system").await
    }

    // ── plugins ──────────────────────────────────────────────────────────────

    pub async fn list_plugins(&self) -> eyre::Result<Vec<PluginSummary>> {
        self.get("/api/plugins").await
    }

    pub async fn plugin_search(
        &self,
        name: &str,
        text: Option<&str>,
        set_code: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<PluginCard>> {
        self.post(
            &format!("/api/plugins/{}/search", urlenc(name)),
            &serde_json::json!({
                "filters": { "text": text, "set_code": set_code },
                "skip": skip,
                "limit": limit,
            }),
        )
        .await
    }

    pub async fn plugin_cards_by_ids(
        &self,
        name: &str,
        ids: Vec<String>,
    ) -> eyre::Result<HashMap<String, PluginCard>> {
        self.post(&format!("/api/plugins/{}/cards/by-ids", urlenc(name)), &ids)
            .await
    }

    pub async fn plugin_update(&self, name: &str) -> eyre::Result<String> {
        self.get(&format!("/api/plugins/{}/update", urlenc(name))).await
    }

    // ── mtg card catalog ─────────────────────────────────────────────────────

    pub async fn mtg_search(
        &self,
        filters: &serde_json::Value,
        skip: usize,
        limit: usize,
    ) -> eyre::Result<Vec<serde_json::Value>> {
        self.post(&format!("/api/mtg/cards/search?skip={skip}&limit={limit}"), filters)
            .await
    }

    pub async fn mtg_cards_by_ids(
        &self,
        ids: &[String],
    ) -> eyre::Result<HashMap<String, serde_json::Value>> {
        let qs: String = ids
            .iter()
            .map(|id| format!("ids={}", urlenc(id)))
            .collect::<Vec<_>>()
            .join("&");
        self.get(&format!("/api/mtg/cards?{qs}")).await
    }

    pub async fn mtg_random_card(&self) -> eyre::Result<serde_json::Value> {
        self.get("/api/mtg/cards/random").await
    }

    pub async fn mtg_sets(&self) -> eyre::Result<Vec<serde_json::Value>> {
        self.get("/api/mtg/sets").await
    }

    pub async fn mtg_bulk_prices(
        &self,
        ids: &[String],
    ) -> eyre::Result<HashMap<String, serde_json::Value>> {
        let qs: String = ids
            .iter()
            .map(|id| format!("ids={}", urlenc(id)))
            .collect::<Vec<_>>()
            .join("&");
        self.get(&format!("/api/mtg/prices?{qs}")).await
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
