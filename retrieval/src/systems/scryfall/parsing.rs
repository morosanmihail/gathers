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
