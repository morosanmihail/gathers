use serde::{Deserialize, Serialize};

use crate::{CardID, CardName, CardTrait, CollectorNumber, SetCode};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PokemonCard {
    pub id: CardID,
    pub set_code: SetCode,
    /// Short form set key printed in the corner of the card, e.g. "PAR" for Paradox Rift.
    pub set_short_code: Option<String>,
    pub collector_number: CollectorNumber,
    pub name: CardName,
    pub rarity: PokemonRarity,
    pub image: String,
    pub energy_types: Vec<EnergyType>,
    pub card_type: String,
    pub pokedex: Option<i64>,
    pub description: Option<String>,
    pub release_date: Option<String>,
}

impl CardTrait for PokemonCard {
    fn get_set(&self) -> SetCode {
        self.set_code.clone()
    }

    fn get_collector_number(&self) -> CollectorNumber {
        self.collector_number.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum PokemonRarity {
    #[default]
    Common,
    DoubleRare,
    Uncommon,
    Rare,
    RadiantRare,
    UltraRare,
    SecretRare,
    HoloRare,
    Promo,
    ClassicCollection,
    AmazingRare,
    ShinyHoloRare,
    PrismRare,
    RareBreak,
    RareAce,
    SpecialIllustrationRare,
    IllustrationRare,
    HyperRare,
    CodeCard,
    ShinyRare,
    ShinyUltraRare,
    AceSpecRare,
    BlackWhiteRare,
    MegaHyperRare,
    MegaAttackRare,
}

impl std::fmt::Display for PokemonRarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PokemonRarity::Common => write!(f, "Common"),
            PokemonRarity::Uncommon => write!(f, "Uncommon"),
            PokemonRarity::Rare => write!(f, "Rare"),
            PokemonRarity::HoloRare => write!(f, "Holo Rare"),
            PokemonRarity::UltraRare => write!(f, "Ultra Rare"),
            PokemonRarity::SecretRare => write!(f, "Secret Rare"),
            PokemonRarity::DoubleRare => write!(f, "Double Rare"),
            PokemonRarity::RadiantRare => write!(f, "Radiant Rare"),
            PokemonRarity::Promo => write!(f, "Promo"),
            PokemonRarity::ClassicCollection => write!(f, "Classic Collection"),
            PokemonRarity::AmazingRare => write!(f, "Amazing Rare"),
            PokemonRarity::ShinyHoloRare => write!(f, "Shiny Holo Rare"),
            PokemonRarity::PrismRare => write!(f, "Prism Rare"),
            PokemonRarity::RareBreak => write!(f, "Rare Break"),
            PokemonRarity::RareAce => write!(f, "Rare Ace"),
            PokemonRarity::SpecialIllustrationRare => write!(f, "Special Illustration Rare"),
            PokemonRarity::IllustrationRare => write!(f, "Illustration Rare"),
            PokemonRarity::HyperRare => write!(f, "Hyper Rare"),
            PokemonRarity::CodeCard => write!(f, "Code Card"),
            PokemonRarity::ShinyRare => write!(f, "Shiny Rare"),
            PokemonRarity::ShinyUltraRare => write!(f, "Shiny Ultra Rare"),
            PokemonRarity::AceSpecRare => write!(f, "Ace Spec Rare"),
            PokemonRarity::BlackWhiteRare => write!(f, "Black White Rare"),
            PokemonRarity::MegaHyperRare => write!(f, "Mega Hyper Rare"),
            PokemonRarity::MegaAttackRare => write!(f, "Mega Attack Rare"),
        }
    }
}

impl PokemonRarity {
    pub fn to_single_string(&self) -> &'static str {
        match self {
            PokemonRarity::Common => "common",
            PokemonRarity::Uncommon => "uncommon",
            PokemonRarity::Rare => "rare",
            PokemonRarity::DoubleRare => "double rare",
            PokemonRarity::RadiantRare => "radiant rare",
            PokemonRarity::UltraRare => "ultra rare",
            PokemonRarity::SecretRare => "secret rare",
            PokemonRarity::HoloRare => "holo rare",
            PokemonRarity::Promo => "promo",
            PokemonRarity::ClassicCollection => "classic collection",
            PokemonRarity::AmazingRare => "amazing rare",
            PokemonRarity::ShinyHoloRare => "shiny holo rare",
            PokemonRarity::PrismRare => "prism rare",
            PokemonRarity::RareBreak => "rare break",
            PokemonRarity::RareAce => "rare ace",
            PokemonRarity::SpecialIllustrationRare => "special illustration rare",
            PokemonRarity::IllustrationRare => "illustration rare",
            PokemonRarity::HyperRare => "hyper rare",
            PokemonRarity::CodeCard => "code card",
            PokemonRarity::ShinyRare => "shiny rare",
            PokemonRarity::ShinyUltraRare => "shiny ultra rare",
            PokemonRarity::AceSpecRare => "ace spec rare",
            PokemonRarity::BlackWhiteRare => "black white rare",
            PokemonRarity::MegaHyperRare => "mega hyper rare",
            PokemonRarity::MegaAttackRare => "mega attack rare",
        }
    }
}

impl From<String> for PokemonRarity {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "common" => PokemonRarity::Common,
            "uncommon" => PokemonRarity::Uncommon,
            "rare" => PokemonRarity::Rare,
            "holo rare" | "holofoil rare" => PokemonRarity::HoloRare,
            "ultra rare" => PokemonRarity::UltraRare,
            "secret rare" | "secret" => PokemonRarity::SecretRare,
            "double rare" => PokemonRarity::DoubleRare,
            "radiant rare" => PokemonRarity::RadiantRare,
            "promo" => PokemonRarity::Promo,
            "classic collection" => PokemonRarity::ClassicCollection,
            "amazing rare" => PokemonRarity::AmazingRare,
            "shiny holo rare" => Self::ShinyHoloRare,
            "prism rare" => PokemonRarity::PrismRare,
            "rare break" => PokemonRarity::RareBreak,
            "rare ace" => PokemonRarity::RareAce,
            "special illustration rare" => PokemonRarity::SpecialIllustrationRare,
            "illustration rare" => PokemonRarity::IllustrationRare,
            "hyper rare" => PokemonRarity::HyperRare,
            "code card" => PokemonRarity::CodeCard,
            "shiny rare" => PokemonRarity::ShinyRare,
            "shiny ultra rare" => PokemonRarity::ShinyUltraRare,
            "ace spec rare" => PokemonRarity::AceSpecRare,
            "black white rare" => PokemonRarity::BlackWhiteRare,
            "mega hyper rare" => PokemonRarity::MegaHyperRare,
            "mega attack rare" => PokemonRarity::MegaAttackRare,
            _ => PokemonRarity::Common,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EnergyType {
    Fire,
    Water,
    Grass,
    Lightning,
    Psychic,
    Fighting,
    Darkness,
    Metal,
    Dragon,
    Fairy,
    Colorless,
    Energy,
}

impl std::fmt::Display for EnergyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnergyType::Fire => write!(f, "Fire"),
            EnergyType::Water => write!(f, "Water"),
            EnergyType::Grass => write!(f, "Grass"),
            EnergyType::Lightning => write!(f, "Lightning"),
            EnergyType::Psychic => write!(f, "Psychic"),
            EnergyType::Fighting => write!(f, "Fighting"),
            EnergyType::Darkness => write!(f, "Darkness"),
            EnergyType::Metal => write!(f, "Metal"),
            EnergyType::Dragon => write!(f, "Dragon"),
            EnergyType::Fairy => write!(f, "Fairy"),
            EnergyType::Colorless => write!(f, "Colorless"),
            EnergyType::Energy => write!(f, "Energy"),
        }
    }
}

impl From<String> for EnergyType {
    fn from(value: String) -> Self {
        match value.trim().to_lowercase().as_str() {
            "fire" => EnergyType::Fire,
            "water" => EnergyType::Water,
            "grass" => EnergyType::Grass,
            "lightning" | "electric" => EnergyType::Lightning,
            "psychic" => EnergyType::Psychic,
            "fighting" => EnergyType::Fighting,
            "darkness" | "dark" => EnergyType::Darkness,
            "metal" | "steel" => EnergyType::Metal,
            "dragon" => EnergyType::Dragon,
            "fairy" => EnergyType::Fairy,
            "energy" => EnergyType::Energy,
            _ => EnergyType::Colorless,
        }
    }
}
