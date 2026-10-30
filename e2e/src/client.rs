use eyre::{Context, bail};
use reqwest::StatusCode;

use crate::models::{
    AdjustWantQuantityRequest, AllPurchaseHistoryResponse, AvailabilityRequest, BoardCreate,
    BoardRename, BoardReorder, CardAvailability, CardToAdd, Collection, CollectionAddResponse,
    CollectionCard, CollectionKind, CollectionKindChange, CollectionRemoveResponse,
    CollectionRenameRequest, CollectionToAdd, CollectionValueCards, CollectionValueHistory,
    DeckBoard, DeckCard, DeckCardAdjustment, DeckCardMove, DeckCardTransfer,
    DeckCardTransferResult, DeckMeta, MtgCard, PriceHistoryResponse, PublicCollectionPage,
    PurchaseHistoryResponse, ShareLink, ShareLinkRevokeResponse, SystemInfo, SystemMeta, Transfer,
    ValueBreakdown,
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
        self.post(
            "/api/collection/add",
            &Collection { id: name.to_string(), kind: None },
        )
        .await
    }

    /// Renames a collection or deck. A deck stays a deck, and its cards follow it.
    pub async fn rename_collection(&self, id: &str, new_id: &str) -> eyre::Result<Collection> {
        self.post(
            &format!("/api/collection/rename/{}", urlenc(id)),
            &CollectionRenameRequest { new_id: new_id.to_string() },
        )
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

    /// Add (or subtract when qty is negative) cards to a collection, for a
    /// specific finish (`""` = the default/primary finish, MTG's `"foil"`/
    /// `"etched"`, or any other finish label — the server doesn't validate
    /// `finish` against the card's own catalog finishes, see
    /// `finishes.rs`). Pass `purchase_price` to record a purchase history entry.
    pub async fn add_cards(
        &self,
        collection_id: &str,
        card_id: &str,
        finish: &str,
        quantity: i32,
        purchase_price: Option<f64>,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.add_cards_with_provider(collection_id, card_id, finish, quantity, purchase_price, None)
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
        finish: &str,
        quantity: i32,
        purchase_price: Option<f64>,
        provider: Option<&str>,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!("/api/collection/cards/{}/add", urlenc(collection_id)),
            &CardToAdd {
                id: card_id.to_string(),
                finish: finish.to_string(),
                quantity,
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
        finish: &str,
        quantity: i32,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(
            &format!("/api/collection/cards/{}/delete", urlenc(collection_id)),
            &CardToAdd {
                id: card_id.to_string(),
                finish: finish.to_string(),
                quantity,
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

    // ── MTG cards ─────────────────────────────────────────────────────────────

    /// Searches the MTG system with `filters` (an `APICardSearchFilters` object).
    pub async fn search_mtg(&self, filters: &serde_json::Value, limit: usize) -> eyre::Result<Vec<MtgCard>> {
        self.post(&format!("/api/mtg/cards/search?limit={limit}"), filters).await
    }

    /// MTG cards by id; ids the system doesn't know are left out.
    pub async fn mtg_cards(&self, ids: &[&str]) -> eyre::Result<std::collections::HashMap<String, MtgCard>> {
        let query: Vec<String> = ids.iter().map(|id| format!("ids={}", urlenc(id))).collect();
        self.get(&format!("/api/mtg/cards?{}", query.join("&"))).await
    }

    /// A game's (`mtg`, `riftbound`, `pokemon`) sets, card types and grouping modes.
    pub async fn meta(&self, game: &str) -> eyre::Result<SystemMeta> {
        self.get(&format!("/api/{game}/meta")).await
    }

    /// Searches `game` (`mtg`, `riftbound`, `pokemon`) with `filters` (an
    /// `APICardSearchFilters` object), returning the cards as raw JSON.
    pub async fn search(
        &self,
        game: &str,
        filters: &serde_json::Value,
        limit: usize,
    ) -> eyre::Result<Vec<serde_json::Value>> {
        self.post(&format!("/api/{game}/cards/search?limit={limit}"), filters).await
    }

    /// The entries of `collection_id` whose cards match `filters`.
    pub async fn search_collection(
        &self,
        collection_id: &str,
        filters: &serde_json::Value,
    ) -> eyre::Result<Vec<CollectionCard>> {
        self.post(&format!("/api/collection/cards/{}/search", urlenc(collection_id)), filters)
            .await
    }

    /// Imports `csv` (gathers' own `Set,CollectorNumber,Quantity,FoilQuantity`
    /// format) as a new collection named `collection`.
    pub async fn import_csv(&self, collection: &str, csv: &str) -> eyre::Result<()> {
        let url = format!("{}/api/collection/import", self.base_url);
        let form = reqwest::multipart::Form::new()
            .text("collection", collection.to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(csv.as_bytes().to_vec()).file_name("import.csv"),
            );
        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        parse_response(resp, "POST", &url).await
    }

    // ── backups ───────────────────────────────────────────────────────────────

    /// Downloads a full backup zip (`GET /api/settings/backup`).
    pub async fn download_backup(&self) -> eyre::Result<Vec<u8>> {
        let url = format!("{}/api/settings/backup", self.base_url);
        let resp = self.client.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.bytes().await.with_context(|| format!("reading GET {url}"))?;
        if !status.is_success() {
            bail!("GET {url} returned {status}: {}", String::from_utf8_lossy(&body));
        }
        Ok(body.to_vec())
    }

    /// Replaces the server's data with a backup zip (`POST /api/settings/backup`).
    pub async fn restore_backup(&self, zip: Vec<u8>) -> eyre::Result<serde_json::Value> {
        let url = format!("{}/api/settings/backup", self.base_url);
        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(zip).file_name("backup.zip"));
        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        parse_response(resp, "POST", &url).await
    }

    // ── system & prices ───────────────────────────────────────────────────────

    pub async fn system_info(&self) -> eyre::Result<SystemInfo> {
        self.get("/api/system").await
    }

    /// Current MTG prices for `card_id`, keyed by retailer (empty when the
    /// server has no MTG price database).
    pub async fn mtg_prices(
        &self,
        card_id: &str,
    ) -> eyre::Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut prices: std::collections::HashMap<String, serde_json::Value> =
            self.get(&format!("/api/mtg/prices?ids={}", urlenc(card_id))).await?;
        Ok(prices
            .remove(card_id)
            .and_then(|p| p.get("paper").cloned())
            .and_then(|p| serde_json::from_value(p).ok())
            .unwrap_or_default())
    }

    /// Starts a background price database update for `game` (`mtg` or
    /// `pokemon`); poll `system_info().downloading` for `{key}-prices`.
    pub async fn update_prices(&self, game: &str) -> eyre::Result<String> {
        self.post_empty(&format!("/api/{game}/prices/update")).await
    }

    /// A card's recorded daily prices under `provider`.
    pub async fn price_history(&self, provider: &str, card_id: &str) -> eyre::Result<PriceHistoryResponse> {
        self.get(&format!(
            "/api/collection/price_history/{}/{}",
            urlenc(provider),
            urlenc(card_id),
        ))
        .await
    }

    /// What the cards now in `collection_id` were worth on each recorded day.
    pub async fn value_history(&self, collection_id: &str) -> eyre::Result<CollectionValueHistory> {
        self.get(&format!("/api/collection/cards/{}/value_history", urlenc(collection_id))).await
    }

    /// Every owned, priced entry's value, with its price `days` ago.
    pub async fn value_cards(&self, collection_id: &str, days: u32) -> eyre::Result<CollectionValueCards> {
        self.get(&format!("/api/collection/cards/{}/value_cards?days={days}", urlenc(collection_id))).await
    }

    // ── low-level helpers ─────────────────────────────────────────────────────

    // ── decks ────────────────────────────────────────────────────────────────

    /// Creates a deck: a collection with `kind = "deck"`, whose cards live in `deck_cards` and are
    /// reached through `/api/deck` rather than `/api/collection`.
    pub async fn add_deck(&self, name: &str) -> eyre::Result<CollectionAddResponse> {
        self.post(
            "/api/collection/add",
            &CollectionToAdd {
                id: name.to_string(),
                kind: CollectionKind::Deck,
            },
        )
        .await
    }

    pub async fn deck_cards(&self, deck: &str) -> eyre::Result<Vec<DeckCard>> {
        self.get(&format!("/api/deck/cards/{}/list", urlenc(deck))).await
    }

    /// Slots filled in the deck; the server's count, which takes the greater of a row's owned
    /// and wanted copies rather than their sum.
    pub async fn deck_card_count(&self, deck: &str) -> eyre::Result<i64> {
        self.get(&format!("/api/deck/cards/{}/count", urlenc(deck))).await
    }

    /// A collection's value; reports nothing for a deck, whose cards aren't in `cards`.
    pub async fn collection_value(&self, collection: &str) -> eyre::Result<ValueBreakdown> {
        self.get(&format!("/api/collection/cards/{}/value_breakdown", urlenc(collection))).await
    }

    /// A deck's value. Its `wanted_value` prices only the wanted copies the deck doesn't have yet.
    pub async fn deck_value(&self, deck: &str) -> eyre::Result<ValueBreakdown> {
        self.get(&format!("/api/deck/cards/{}/value_breakdown", urlenc(deck))).await
    }

    /// Changes one card's owned count on one board.
    ///
    /// Never changes what any collection says is owned; `transfer_deck_card` is for copies that
    /// actually move.
    pub async fn adjust_deck_card(
        &self,
        deck: &str,
        card_id: &str,
        finish: &str,
        board: &str,
        quantity: i32,
    ) -> eyre::Result<Option<DeckCard>> {
        self.adjust_deck_card_counts(deck, card_id, finish, board, quantity, 0).await
    }

    /// Changes both of a card's counts on one board.
    pub async fn adjust_deck_card_counts(
        &self,
        deck: &str,
        card_id: &str,
        finish: &str,
        board: &str,
        quantity: i32,
        want_quantity: i32,
    ) -> eyre::Result<Option<DeckCard>> {
        self.post(
            &format!("/api/deck/cards/{}/adjust", urlenc(deck)),
            &DeckCardAdjustment {
                id: card_id.to_string(),
                finish: finish.to_string(),
                board: board.to_string(),
                quantity,
                want_quantity,
            },
        )
        .await
    }

    pub async fn move_deck_card(
        &self,
        deck: &str,
        card_id: &str,
        finish: &str,
        from_board: &str,
        to_board: &str,
        quantity: u32,
    ) -> eyre::Result<Vec<DeckCard>> {
        self.post(
            &format!("/api/deck/cards/{}/move", urlenc(deck)),
            &DeckCardMove {
                id: card_id.to_string(),
                finish: finish.to_string(),
                from_board: from_board.to_string(),
                to_board: to_board.to_string(),
                quantity,
                want_quantity: 0,
            },
        )
        .await
    }

    /// Moves copies between a collection and one of a deck's boards.
    pub async fn transfer_deck_card(
        &self,
        deck: &str,
        collection: &str,
        card_id: &str,
        finish: &str,
        board: &str,
        transfer: Transfer,
    ) -> eyre::Result<i32> {
        let result: DeckCardTransferResult = self
            .post(
                &format!("/api/deck/cards/{}/transfer", urlenc(deck)),
                &DeckCardTransfer {
                    id: card_id.to_string(),
                    finish: finish.to_string(),
                    collection: collection.to_string(),
                    board: board.to_string(),
                    transfer,
                },
            )
            .await?;
        Ok(result.moved)
    }

    pub async fn deck_boards(&self, deck: &str) -> eyre::Result<Vec<DeckBoard>> {
        self.get(&format!("/api/deck/boards/{}", urlenc(deck))).await
    }

    pub async fn add_deck_board(&self, deck: &str, board: &str) -> eyre::Result<Vec<DeckBoard>> {
        self.post(
            &format!("/api/deck/boards/{}", urlenc(deck)),
            &BoardCreate { board: board.to_string() },
        )
        .await
    }

    pub async fn rename_deck_board(
        &self,
        deck: &str,
        from: &str,
        to: &str,
    ) -> eyre::Result<Vec<DeckBoard>> {
        self.put(
            &format!("/api/deck/boards/{}", urlenc(deck)),
            &BoardRename { from: from.to_string(), to: to.to_string() },
        )
        .await
    }

    pub async fn reorder_deck_boards(
        &self,
        deck: &str,
        boards: &[&str],
    ) -> eyre::Result<Vec<DeckBoard>> {
        self.post(
            &format!("/api/deck/boards/{}/order", urlenc(deck)),
            &BoardReorder { boards: boards.iter().map(|b| b.to_string()).collect() },
        )
        .await
    }

    /// Converts a collection to a deck or back, carrying its cards across.
    pub async fn set_collection_kind(
        &self,
        collection: &str,
        kind: CollectionKind,
    ) -> eyre::Result<()> {
        let _: serde_json::Value = self
            .post(
                &format!("/api/collection/kind/{}", urlenc(collection)),
                &CollectionKindChange { kind },
            )
            .await?;
        Ok(())
    }

    pub async fn deck_meta(&self, deck: &str) -> eyre::Result<DeckMeta> {
        self.get(&format!("/api/deck/meta/{}", urlenc(deck))).await
    }

    pub async fn set_deck_meta(
        &self,
        deck: &str,
        built: bool,
        location: Option<&str>,
    ) -> eyre::Result<DeckMeta> {
        self.put(
            &format!("/api/deck/meta/{}", urlenc(deck)),
            &DeckMeta {
                built,
                location: location.map(str::to_string),
                notes: None,
            },
        )
        .await
    }

    /// Where the copies of each card currently are: owned, committed to built decks, and which
    /// decks those are.
    pub async fn card_availability(&self, ids: &[&str]) -> eyre::Result<Vec<CardAvailability>> {
        self.post(
            "/api/cards/availability",
            &AvailabilityRequest {
                ids: ids.iter().map(|s| s.to_string()).collect(),
            },
        )
        .await
    }

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

    async fn put<B: serde::Serialize + ?Sized, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> eyre::Result<T> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("PUT {url}"))?;
        parse_response(resp, "PUT", &url).await
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
