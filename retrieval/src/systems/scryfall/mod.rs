mod parsing;

use std::collections::HashMap;

use eyre::OptionExt;
use models::{
    Card, CardID, CardIdentifiers, CollectorNumber, SetCode, filters::{CardSearchFilters, SortField, SortOrder},
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

impl RetrievalSystemTrait for ScryfallRetrievalSystem {
    #[allow(unused_variables)]
    async fn search_cards(
        &self,
        filters: CardSearchFilters,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> eyre::Result<Vec<Card>> {
        let mut query = vec![];

        if let Some(name) = &filters.name {
            query.push(format!("name:{}", name));
        }

        if let Some(set_code) = &filters.set_code {
            query.push(format!("set:{}", set_code));
        }

        if let Some(color_identities) = &filters.color_identities {
            for color in color_identities {
                query.push(format!("c:{}", color));
            }
        }

        if let Some(text) = &filters.text {
            query.push(format!("t:{}", text));
        }

        if let Some(types) = &filters.types {
            for t in types {
                query.push(format!("type:{}", t));
            }
        }

        if let Some(subtypes) = &filters.subtypes {
            for s in subtypes {
                query.push(format!("type:{}", s));
            }
        }

        if let Some(supertypes) = &filters.supertypes {
            query.push(format!("type:{}", supertypes));
        }

        let query_string = query.join(" ");

        let page = skip.map(|s| s / 100).unwrap_or(1);
        let unique = "cards";
        let order = match &filters.sort_by {
            Some(SortField::Rarity) => "rarity",
            Some(SortField::SetCode) => "set",
            Some(SortField::CollectorNumber) => "collector_number",
            Some(SortField::Artist) => "artist",
            _ => "name",
        };
        let dir = if matches!(&filters.sort_order, Some(SortOrder::Desc)) { "desc" } else { "asc" };
        let include_extras = false;

        let url = format!(
            "https://api.scryfall.com/cards/search?q={}&page={}&unique={}&order={}&dir={}&include_extras={}",
            query_string, page, unique, order, dir, include_extras
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("gathers_cli/1.0"),
        );
        headers.insert("Accept", reqwest::header::HeaderValue::from_static("*/*"));
        let client = reqwest::Client::new();
        let response = client.get(url).headers(headers).send().await?;
        let json: Value = response.json().await?;

        if let Some(error) = json.get("object").and_then(Value::as_str)
            && error == "error"
        {
            let error_msg = json
                .get("details")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            return Err(eyre::eyre!("Scryfall API error: {}", error_msg));
        }

        let cards_array = json
            .get("data")
            .and_then(Value::as_array)
            .ok_or_eyre("Could not retrieve cards array")?;

        let limit = limit.unwrap_or(cards_array.len());
        let cards = cards_array
            .iter()
            .take(limit)
            .filter_map(|card| {
                let card = card.as_object()?;
                let card_name = card.get("name")?.as_str()?;
                let card_id = card.get("id")?.as_str()?;
                let set_code = card.get("set")?.as_str()?;
                let artist = card.get("artist")?.as_str()?;
                let rarity = card.get("rarity")?.as_str()?;
                let oracle_text = card.get("oracle_text")?.as_str()?;
                let collector_number = card.get("collector_number")?.as_str()?;

                let color_identity = parsing::parse_color_identity(
                    card.get("color_identity").and_then(Value::as_array)?,
                );

                let type_line = card.get("type_line")?.as_str()?;
                let mut types = vec![];
                let mut subtypes = vec![];
                let mut supertypes = vec![];

                let parts: Vec<&str> = type_line.split("—").map(|p| p.trim()).collect();
                if !parts.is_empty() {
                    let type_part = parts[0];
                    let type_tokens: Vec<&str> = type_part.split(' ').collect();
                    for token in type_tokens {
                        match token {
                            // TODO: add the rest
                            "Legendary" | "Basic" | "World" => supertypes.push(token.to_string()),
                            _ => types.push(token.to_string()),
                        }
                    }
                }
                if parts.len() > 1 {
                    let subtype_part = parts[1];
                    let subtype_tokens: Vec<&str> = subtype_part.split(' ').collect();
                    subtypes = subtype_tokens.iter().map(|s| s.to_string()).collect();
                }

                Some(Card::Magic(models::MagicCard {
                    name: card_name.to_string(),
                    set_code: set_code.to_string(),
                    artist: artist.to_string(),
                    color_identity,
                    id: card_id.to_string(),
                    rarity: rarity.to_string().into(),
                    text: oracle_text.to_string(),
                    card_identifiers: CardIdentifiers {
                        scryfall_id: card_id.to_string(),
                        id: card_id.to_string(),
                    },
                    collector_number: collector_number.to_string(),
                    subtypes,
                    supertypes,
                    types,
                }))
            })
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
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_static("gathers_cli/1.0"),
            );
            headers.insert("Accept", reqwest::header::HeaderValue::from_static("*/*"));
            let client = reqwest::Client::new();
            let response = client.get(url).headers(headers).send().await?;
            let json: Value = response.json().await?;

            let card_name = json
                .get("name")
                .and_then(Value::as_str)
                .ok_or_eyre("Could not retrieve name")?;
            let card_id = json
                .get("id")
                .and_then(Value::as_str)
                .ok_or_eyre("Could not retrieve id")?
                .to_string();

            let card = models::MagicCard {
                name: card_name.to_string(),
                set_code: json
                    .get("set")
                    .and_then(Value::as_str)
                    .ok_or_eyre("Could not retrieve set")?
                    .to_string(),
                artist: json
                    .get("artist")
                    .and_then(Value::as_str)
                    .ok_or_eyre("Could not retrieve artist")?
                    .to_string(),
                color_identity: parsing::parse_color_identity(
                    json.get("color_identity")
                        .and_then(Value::as_array)
                        .ok_or_eyre("Could not retrieve color identity")?,
                ),
                id: card_id.clone(),
                rarity: json
                    .get("rarity")
                    .and_then(Value::as_str)
                    .ok_or_eyre("Could not retrieve rarity")?
                    .to_string()
                    .into(),
                text: json
                    .get("oracle_text")
                    .and_then(Value::as_str)
                    .ok_or_eyre("Could not retrieve oracle text")?
                    .to_string(),
                card_identifiers: CardIdentifiers {
                    scryfall_id: card_id.clone(),
                    id: card_id.clone(),
                },
                collector_number: json
                    .get("collector_number")
                    .and_then(Value::as_str)
                    .ok_or_eyre("Could not retrieve collector number")?
                    .to_string(),
                subtypes: vec![],
                supertypes: vec![],
                types: vec![],
            };

            result.insert(id, models::Card::Magic(card));
        }

        Ok(result)
    }

    async fn get_sets(&self) -> eyre::Result<Vec<models::Set>> {
        // TODO: implement this
        Ok(vec![])
    }

    #[allow(unused_variables)]
    async fn bulk_search_cards(
        &self,
        cards: Vec<(SetCode, CollectorNumber)>,
    ) -> eyre::Result<Vec<(SetCode, CollectorNumber, CardID)>> {
        Ok(vec![])
    }

    async fn update_backend(&self) -> eyre::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
