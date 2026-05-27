use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{Artist, CardID, CardName, CardText, CardTrait, CollectorNumber, SetCode};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MagicCard {
    pub id: CardID,
    pub name: CardName,
    pub set_code: SetCode,
    pub collector_number: CollectorNumber,
    pub rarity: Rarity,
    pub artist: Artist,
    pub color_identity: Vec<CardColour>,
    pub text: CardText,
    pub card_identifiers: CardIdentifiers,
    pub subtypes: Vec<String>,
    pub supertypes: Vec<String>,
    pub types: Vec<String>,
}

impl CardTrait for MagicCard {
    fn get_set(&self) -> SetCode {
        self.set_code.clone()
    }

    fn get_collector_number(&self) -> CollectorNumber {
        self.collector_number.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Mythic,
    Special,
    Bonus,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CardIdentifiers {
    pub id: String,
    pub scryfall_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CardColour {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colourless,
    Multicoloured,
}

impl Default for MagicCard {
    fn default() -> MagicCard {
        MagicCard {
            id: "".to_string(),
            name: "".to_string(),
            set_code: "".to_string(),
            rarity: Rarity::Common,
            artist: "".to_string(),
            color_identity: vec![],
            text: "".to_string(),
            card_identifiers: CardIdentifiers {
                id: "-1".to_string(),
                scryfall_id: "".to_string(),
            },
            collector_number: "".to_string(),
            subtypes: vec![],
            supertypes: vec![],
            types: vec![],
        }
    }
}

impl Display for CardColour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardColour::Red => write!(f, "R"),
            CardColour::White => write!(f, "W"),
            CardColour::Blue => write!(f, "U"),
            CardColour::Green => write!(f, "G"),
            CardColour::Black => write!(f, "B"),
            CardColour::Multicoloured => write!(f, "_"),
            CardColour::Colourless => write!(f, "C"),
        }
    }
}

impl From<&str> for CardColour {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "w" | "white" => CardColour::White,
            "u" | "blue" => CardColour::Blue,
            "b" | "black" => CardColour::Black,
            "r" | "red" => CardColour::Red,
            "g" | "green" => CardColour::Green,
            "c" | "colourless" => CardColour::Colourless,
            "m" | "multicoloured" => CardColour::Multicoloured,
            _ => CardColour::Colourless,
        }
    }
}

impl std::fmt::Display for Rarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rarity::Common => write!(f, "Common"),
            Rarity::Uncommon => write!(f, "Uncommon"),
            Rarity::Rare => write!(f, "Rare"),
            Rarity::Mythic => write!(f, "Mythic"),
            Rarity::Special => write!(f, "Special"),
            Rarity::Bonus => write!(f, "Bonus"),
        }
    }
}

impl Rarity {
    pub fn to_single_string(&self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Mythic => "mythic",
            Rarity::Special => "special",
            Rarity::Bonus => "bonus",
        }
    }
}

impl From<String> for Rarity {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Common" | "common" | "c" => Rarity::Common,
            "Uncommon" | "uncommon" | "u" => Rarity::Uncommon,
            "Rare" | "rare" | "r" => Rarity::Rare,
            "Mythic" | "mythic" | "m" => Rarity::Mythic,
            "Special" | "special" => Rarity::Special,
            "Bonus" | "bonus" | "b" => Rarity::Bonus,
            _ => Rarity::Bonus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_card_default() {
        let c = MagicCard::default();
        assert!(c.id.is_empty());
        assert!(c.name.is_empty());
        assert_eq!(c.rarity, Rarity::Common);
        assert_eq!(c.card_identifiers.id, "-1");
        assert!(c.color_identity.is_empty());
    }

    #[test]
    fn test_card_colour_display() {
        assert_eq!(CardColour::White.to_string(), "W");
        assert_eq!(CardColour::Blue.to_string(), "U");
        assert_eq!(CardColour::Black.to_string(), "B");
        assert_eq!(CardColour::Red.to_string(), "R");
        assert_eq!(CardColour::Green.to_string(), "G");
        assert_eq!(CardColour::Colourless.to_string(), "C");
        assert_eq!(CardColour::Multicoloured.to_string(), "_");
    }

    #[test]
    fn test_card_colour_from_string() {
        assert_eq!(CardColour::from("w"), CardColour::White);
        assert_eq!(CardColour::from("white"), CardColour::White);
        assert_eq!(CardColour::from("u"), CardColour::Blue);
        assert_eq!(CardColour::from("blue"), CardColour::Blue);
        assert_eq!(CardColour::from("b"), CardColour::Black);
        assert_eq!(CardColour::from("black"), CardColour::Black);
        assert_eq!(CardColour::from("r"), CardColour::Red);
        assert_eq!(CardColour::from("red"), CardColour::Red);
        assert_eq!(CardColour::from("g"), CardColour::Green);
        assert_eq!(CardColour::from("green"), CardColour::Green);
        assert_eq!(CardColour::from("c"), CardColour::Colourless);
        assert_eq!(CardColour::from("colourless"), CardColour::Colourless);
        assert_eq!(CardColour::from("m"), CardColour::Multicoloured);
        assert_eq!(CardColour::from("multicoloured"), CardColour::Multicoloured);
        assert_eq!(CardColour::from("UNKNOWN"), CardColour::Colourless);
    }

    #[test]
    fn test_card_colour_from_string_case_insensitive() {
        assert_eq!(CardColour::from("WHITE"), CardColour::White);
        assert_eq!(CardColour::from("RED"), CardColour::Red);
    }

    #[test]
    fn test_rarity_display() {
        assert_eq!(Rarity::Common.to_string(), "Common");
        assert_eq!(Rarity::Uncommon.to_string(), "Uncommon");
        assert_eq!(Rarity::Rare.to_string(), "Rare");
        assert_eq!(Rarity::Mythic.to_string(), "Mythic");
        assert_eq!(Rarity::Special.to_string(), "Special");
        assert_eq!(Rarity::Bonus.to_string(), "Bonus");
    }

    #[test]
    fn test_rarity_to_single_string() {
        assert_eq!(Rarity::Common.to_single_string(), "common");
        assert_eq!(Rarity::Uncommon.to_single_string(), "uncommon");
        assert_eq!(Rarity::Rare.to_single_string(), "rare");
        assert_eq!(Rarity::Mythic.to_single_string(), "mythic");
        assert_eq!(Rarity::Special.to_single_string(), "special");
        assert_eq!(Rarity::Bonus.to_single_string(), "bonus");
    }

    #[test]
    fn test_rarity_from_string() {
        assert_eq!(Rarity::from("Common".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("common".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("c".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("Uncommon".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("uncommon".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("u".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("Rare".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("rare".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("r".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("Mythic".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("mythic".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("m".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("Special".to_string()), Rarity::Special);
        assert_eq!(Rarity::from("UNKNOWN".to_string()), Rarity::Bonus);
        assert_eq!(Rarity::from("".to_string()), Rarity::Bonus);
    }

    #[test]
    fn test_rarity_display_roundtrip_via_from() {
        for r in [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Mythic, Rarity::Special] {
            assert_eq!(Rarity::from(r.to_string()), r);
        }
    }
}
