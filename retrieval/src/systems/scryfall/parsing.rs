use std::collections::HashMap;

use models::{Card, CardColour, CardIdentifiers, MagicCard};
use serde_json::{Map, Value};

pub fn parse_color_identity(arr: &[Value]) -> Vec<CardColour> {
    arr.iter()
        .filter_map(Value::as_str)
        .map(|c| match c {
            "B" => CardColour::Black,
            "U" => CardColour::Blue,
            "W" => CardColour::White,
            "G" => CardColour::Green,
            "R" => CardColour::Red,
            _ => CardColour::Colourless,
        })
        .collect()
}

pub fn parse_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Scryfall's `legalities` object maps format -> `"legal" | "not_legal" | "banned" |
/// "restricted"`; normalise to mtgjson's `"Legal" | "Not Legal" | "Banned" | "Restricted"`
/// so callers see the same casing regardless of backend.
pub fn parse_legalities(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .filter_map(|(format, status)| {
                    let status = status.as_str()?;
                    let readable = status
                        .split('_')
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                Some(first) => {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                }
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    Some((format.clone(), readable))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parses a single Scryfall card JSON object into a `Card::Magic`, shared by
/// both the search-results list mapping and the random-card lookup. Returns
/// `None` if any field required to construct a `MagicCard` is missing.
pub fn parse_card(card: &Map<String, Value>) -> Option<Card> {
    let card_name = card.get("name")?.as_str()?;
    let card_id = card.get("id")?.as_str()?;
    let set_code = card.get("set")?.as_str()?;
    let artist = card.get("artist")?.as_str()?;
    let rarity = card.get("rarity")?.as_str()?;
    let oracle_text = card.get("oracle_text")?.as_str()?;
    let collector_number = card.get("collector_number")?.as_str()?;

    let color_identity =
        parse_color_identity(card.get("color_identity").and_then(Value::as_array)?);

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

    Some(Card::Magic(MagicCard {
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
        mana_cost: card
            .get("mana_cost")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        mana_value: card.get("cmc").and_then(Value::as_f64).unwrap_or_default(),
        type_line: type_line.to_string(),
        power: card.get("power").and_then(Value::as_str).map(String::from),
        toughness: card
            .get("toughness")
            .and_then(Value::as_str)
            .map(String::from),
        loyalty: card
            .get("loyalty")
            .and_then(Value::as_str)
            .map(String::from),
        defense: card
            .get("defense")
            .and_then(Value::as_str)
            .map(String::from),
        keywords: parse_string_array(card.get("keywords")),
        colors: parse_color_identity(
            card.get("colors")
                .and_then(Value::as_array)
                .unwrap_or(&vec![]),
        ),
        legalities: parse_legalities(card.get("legalities")),
        finishes: parse_string_array(card.get("finishes")),
        is_reserved: card
            .get("reserved")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_promo: card.get("promo").and_then(Value::as_bool).unwrap_or(false),
        is_reprint: card
            .get("reprint")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        border_color: card
            .get("border_color")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        frame_effects: parse_string_array(card.get("frame_effects")),
        is_full_art: card
            .get("full_art")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        watermark: card
            .get("watermark")
            .and_then(Value::as_str)
            .map(String::from),
        flavor_text: card
            .get("flavor_text")
            .and_then(Value::as_str)
            .map(String::from),
        set_name: card
            .get("set_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }))
}
