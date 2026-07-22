use std::collections::HashMap;

use models::{CardIdentifiers, MagicCard};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum APIRarity {
    Common,
    Uncommon,
    Rare,
    Mythic,
    Special,
    Bonus,
}

impl From<models::Rarity> for APIRarity {
    fn from(value: models::Rarity) -> Self {
        match value {
            models::Rarity::Common => APIRarity::Common,
            models::Rarity::Uncommon => APIRarity::Uncommon,
            models::Rarity::Rare => APIRarity::Rare,
            models::Rarity::Mythic => APIRarity::Mythic,
            models::Rarity::Special => APIRarity::Special,
            models::Rarity::Bonus => APIRarity::Bonus,
        }
    }
}

impl From<APIRarity> for models::Rarity {
    fn from(value: APIRarity) -> Self {
        match value {
            APIRarity::Common => models::Rarity::Common,
            APIRarity::Uncommon => models::Rarity::Uncommon,
            APIRarity::Rare => models::Rarity::Rare,
            APIRarity::Mythic => models::Rarity::Mythic,
            APIRarity::Special => models::Rarity::Special,
            APIRarity::Bonus => models::Rarity::Bonus,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum APICardColour {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colourless,
    Multicoloured,
}

impl From<models::CardColour> for APICardColour {
    fn from(value: models::CardColour) -> Self {
        match value {
            models::CardColour::White => APICardColour::White,
            models::CardColour::Blue => APICardColour::Blue,
            models::CardColour::Black => APICardColour::Black,
            models::CardColour::Red => APICardColour::Red,
            models::CardColour::Green => APICardColour::Green,
            models::CardColour::Colourless => APICardColour::Colourless,
            models::CardColour::Multicoloured => APICardColour::Multicoloured,
        }
    }
}

impl From<APICardColour> for models::CardColour {
    fn from(value: APICardColour) -> Self {
        match value {
            APICardColour::White => models::CardColour::White,
            APICardColour::Blue => models::CardColour::Blue,
            APICardColour::Black => models::CardColour::Black,
            APICardColour::Red => models::CardColour::Red,
            APICardColour::Green => models::CardColour::Green,
            APICardColour::Colourless => models::CardColour::Colourless,
            APICardColour::Multicoloured => models::CardColour::Multicoloured,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct APICard {
    pub id: String,
    pub name: String,
    #[serde(rename = "setCode")]
    pub set_code: String,
    #[serde(rename = "collectorNumber")]
    pub collector_number: String,
    pub rarity: APIRarity,
    pub artist: String,
    #[serde(rename = "colorIdentity")]
    pub color_identity: Vec<APICardColour>,
    pub text: String,
    #[serde(rename = "cardIdentifiers")]
    pub card_identifiers: APICardIdentifiers,
    #[serde(rename = "manaCost")]
    pub mana_cost: String,
    #[serde(rename = "manaValue")]
    pub mana_value: f64,
    #[serde(rename = "typeLine")]
    pub type_line: String,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub keywords: Vec<String>,
    pub colors: Vec<APICardColour>,
    pub legalities: HashMap<String, String>,
    pub finishes: Vec<String>,
    #[serde(rename = "isReserved")]
    pub is_reserved: bool,
    #[serde(rename = "isPromo")]
    pub is_promo: bool,
    #[serde(rename = "isReprint")]
    pub is_reprint: bool,
    #[serde(rename = "borderColor")]
    pub border_color: String,
    #[serde(rename = "frameEffects")]
    pub frame_effects: Vec<String>,
    #[serde(rename = "isFullArt")]
    pub is_full_art: bool,
    pub watermark: Option<String>,
    #[serde(rename = "flavorText")]
    pub flavor_text: Option<String>,
    #[serde(rename = "setName")]
    pub set_name: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct APICardIdentifiers {
    pub id: String,
    #[serde(rename = "scryfallId")]
    pub scryfall_id: String,
}

impl From<MagicCard> for APICard {
    fn from(value: MagicCard) -> Self {
        APICard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            collector_number: value.collector_number,
            rarity: value.rarity.into(),
            artist: value.artist,
            color_identity: value.color_identity.into_iter().map(Into::into).collect(),
            text: value.text,
            card_identifiers: value.card_identifiers.into(),
            mana_cost: value.mana_cost,
            mana_value: value.mana_value,
            type_line: value.type_line,
            power: value.power,
            toughness: value.toughness,
            loyalty: value.loyalty,
            defense: value.defense,
            keywords: value.keywords,
            colors: value.colors.into_iter().map(Into::into).collect(),
            legalities: value.legalities,
            finishes: value.finishes,
            is_reserved: value.is_reserved,
            is_promo: value.is_promo,
            is_reprint: value.is_reprint,
            border_color: value.border_color,
            frame_effects: value.frame_effects,
            is_full_art: value.is_full_art,
            watermark: value.watermark,
            flavor_text: value.flavor_text,
            set_name: value.set_name,
        }
    }
}

impl From<CardIdentifiers> for APICardIdentifiers {
    fn from(value: CardIdentifiers) -> Self {
        APICardIdentifiers {
            id: value.id,
            scryfall_id: value.scryfall_id,
        }
    }
}
