use std::collections::HashMap;

use models::CardColour;
use serde_json::Value;

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
