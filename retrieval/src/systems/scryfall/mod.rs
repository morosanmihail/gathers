mod parsing;
mod query;

use std::collections::HashMap;

use eyre::OptionExt;
use models::{
    Card, CardID, CollectorNumber, SetCode,
    filters::{CardSearchFilters, SortField, SortOrder},
};
use serde_json::Value;

use crate::{NamedRetrievalSystem, RetrievalSystemTrait};

#[derive(Debug, Clone)]
pub struct ScryfallRetrievalSystem {}

impl NamedRetrievalSystem for ScryfallRetrievalSystem {
    fn name(&self) -> &str {
        "Scryfall"
    }
}

impl ScryfallRetrievalSystem {
    pub fn new() -> eyre::Result<Self> {
        Ok(Self {})
    }
}

/// Scryfall's collection endpoint accepts at most 75 identifiers per request.
const COLLECTION_BATCH_SIZE: usize = 75;

fn request_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("gathers_cli/1.0"),
    );
    headers.insert("Accept", reqwest::header::HeaderValue::from_static("*/*"));
    headers
}

fn scryfall_api_error(json: &Value) -> Option<eyre::Report> {
    let object = json.get("object").and_then(Value::as_str)?;
    if object != "error" {
        return None;
    }
    let error_msg = json
        .get("details")
        .and_then(Value::as_str)
        .unwrap_or("Unknown error");
    Some(eyre::eyre!("Scryfall API error: {}", error_msg))
}

impl RetrievalSystemTrait for ScryfallRetrievalSystem {
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let query_string = query::build_query_string(&filters);
        let page = query::scryfall_page(skip);
        // "prints" (not "cards") so every printing/art of a card is its own
        // search result and collection entry, matching the SQL system (one
        // row per printing) instead of collapsing to a single art per name.
        let unique = "prints";
        let order = match &filters.sort_by {
            Some(SortField::Rarity) => "rarity",
            Some(SortField::SetCode) => "set",
            Some(SortField::CollectorNumber) => "collector_number",
            Some(SortField::Artist) => "artist",
            _ => "name",
        };
        let dir = if matches!(&filters.sort_order, Some(SortOrder::Desc)) {
            "desc"
        } else {
            "asc"
        };
        let include_extras = false;

        let url = format!(
            "https://api.scryfall.com/cards/search?q={}&page={}&unique={}&order={}&dir={}&include_extras={}",
            query_string, page, unique, order, dir, include_extras
        );

        let client = reqwest::Client::new();
        let response = client.get(url).headers(request_headers()).send().await?;
        let json: Value = response.json().await?;

        if let Some(err) = scryfall_api_error(&json) {
            return Err(err);
        }

        let cards_array = json
            .get("data")
            .and_then(Value::as_array)
            .ok_or_eyre("Could not retrieve cards array")?;

        let limit = limit.unwrap_or(cards_array.len());
        let cards = cards_array
            .iter()
            .take(limit)
            .filter_map(|card| parsing::parse_card(card.as_object()?))
            .collect::<Vec<Card>>();

        Ok(cards)
    }

    async fn get_cards_by_ids(
        &self,
        ids: Vec<String>,
    ) -> eyre::Result<HashMap<String, models::Card>> {
        let mut result = HashMap::new();

        for id in ids {
            let url = format!("https://api.scryfall.com/cards/{}", id);
            let client = reqwest::Client::new();
            let response = client.get(url).headers(request_headers()).send().await?;
            let json: Value = response.json().await?;

            if let Some(err) = scryfall_api_error(&json) {
                return Err(err);
            }

            let card = json
                .as_object()
                .and_then(parsing::parse_card)
                .ok_or_eyre("Could not parse card")?;

            result.insert(id, card);
        }

        Ok(result)
    }

    async fn get_sets(&self) -> eyre::Result<Vec<models::Set>> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.scryfall.com/sets")
            .headers(request_headers())
            .send()
            .await?;
        let json: Value = response.json().await?;

        if let Some(err) = scryfall_api_error(&json) {
            return Err(err);
        }

        Ok(parsing::parse_sets_response(&json))
    }

    async fn get_random_card(&self) -> eyre::Result<Option<models::Card>> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.scryfall.com/cards/random")
            .headers(request_headers())
            .send()
            .await?;
        let json: Value = response.json().await?;

        if let Some(err) = scryfall_api_error(&json) {
            return Err(err);
        }

        Ok(json.as_object().and_then(parsing::parse_card))
    }

    async fn bulk_search_cards(
        &self,
        cards: Vec<(SetCode, CollectorNumber)>,
    ) -> eyre::Result<Vec<(SetCode, CollectorNumber, CardID)>> {
        if cards.is_empty() {
            return Ok(vec![]);
        }

        let client = reqwest::Client::new();
        let mut result = vec![];

        for batch in cards.chunks(COLLECTION_BATCH_SIZE) {
            let identifiers: Vec<Value> = batch
                .iter()
                .map(|(set, number)| serde_json::json!({ "set": set, "collector_number": number }))
                .collect();
            let body = serde_json::json!({ "identifiers": identifiers });

            let response = client
                .post("https://api.scryfall.com/cards/collection")
                .headers(request_headers())
                .json(&body)
                .send()
                .await?;
            let json: Value = response.json().await?;

            if let Some(err) = scryfall_api_error(&json) {
                return Err(err);
            }

            result.extend(parsing::parse_collection_response(&json));
        }

        Ok(result)
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
