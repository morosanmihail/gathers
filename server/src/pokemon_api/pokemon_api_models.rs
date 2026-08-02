use models::pokemon::PokemonCard;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum APIPokemonRarity {
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

impl From<models::pokemon::PokemonRarity> for APIPokemonRarity {
    fn from(value: models::pokemon::PokemonRarity) -> Self {
        match value {
            models::pokemon::PokemonRarity::Common => APIPokemonRarity::Common,
            models::pokemon::PokemonRarity::DoubleRare => APIPokemonRarity::DoubleRare,
            models::pokemon::PokemonRarity::Uncommon => APIPokemonRarity::Uncommon,
            models::pokemon::PokemonRarity::Rare => APIPokemonRarity::Rare,
            models::pokemon::PokemonRarity::RadiantRare => APIPokemonRarity::RadiantRare,
            models::pokemon::PokemonRarity::UltraRare => APIPokemonRarity::UltraRare,
            models::pokemon::PokemonRarity::SecretRare => APIPokemonRarity::SecretRare,
            models::pokemon::PokemonRarity::HoloRare => APIPokemonRarity::HoloRare,
            models::pokemon::PokemonRarity::Promo => APIPokemonRarity::Promo,
            models::pokemon::PokemonRarity::ClassicCollection => {
                APIPokemonRarity::ClassicCollection
            }
            models::pokemon::PokemonRarity::AmazingRare => APIPokemonRarity::AmazingRare,
            models::pokemon::PokemonRarity::ShinyHoloRare => APIPokemonRarity::ShinyHoloRare,
            models::pokemon::PokemonRarity::PrismRare => APIPokemonRarity::PrismRare,
            models::pokemon::PokemonRarity::RareBreak => APIPokemonRarity::RareBreak,
            models::pokemon::PokemonRarity::RareAce => APIPokemonRarity::RareAce,
            models::pokemon::PokemonRarity::SpecialIllustrationRare => {
                APIPokemonRarity::SpecialIllustrationRare
            }
            models::pokemon::PokemonRarity::IllustrationRare => APIPokemonRarity::IllustrationRare,
            models::pokemon::PokemonRarity::HyperRare => APIPokemonRarity::HyperRare,
            models::pokemon::PokemonRarity::CodeCard => APIPokemonRarity::CodeCard,
            models::pokemon::PokemonRarity::ShinyRare => APIPokemonRarity::ShinyRare,
            models::pokemon::PokemonRarity::ShinyUltraRare => APIPokemonRarity::ShinyUltraRare,
            models::pokemon::PokemonRarity::AceSpecRare => APIPokemonRarity::AceSpecRare,
            models::pokemon::PokemonRarity::BlackWhiteRare => APIPokemonRarity::BlackWhiteRare,
            models::pokemon::PokemonRarity::MegaHyperRare => APIPokemonRarity::MegaHyperRare,
            models::pokemon::PokemonRarity::MegaAttackRare => APIPokemonRarity::MegaAttackRare,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum APIEnergyType {
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

impl From<models::pokemon::EnergyType> for APIEnergyType {
    fn from(value: models::pokemon::EnergyType) -> Self {
        match value {
            models::pokemon::EnergyType::Fire => APIEnergyType::Fire,
            models::pokemon::EnergyType::Water => APIEnergyType::Water,
            models::pokemon::EnergyType::Grass => APIEnergyType::Grass,
            models::pokemon::EnergyType::Lightning => APIEnergyType::Lightning,
            models::pokemon::EnergyType::Psychic => APIEnergyType::Psychic,
            models::pokemon::EnergyType::Fighting => APIEnergyType::Fighting,
            models::pokemon::EnergyType::Darkness => APIEnergyType::Darkness,
            models::pokemon::EnergyType::Metal => APIEnergyType::Metal,
            models::pokemon::EnergyType::Dragon => APIEnergyType::Dragon,
            models::pokemon::EnergyType::Fairy => APIEnergyType::Fairy,
            models::pokemon::EnergyType::Colorless => APIEnergyType::Colorless,
            models::pokemon::EnergyType::Energy => APIEnergyType::Energy,
        }
    }
}

impl From<APIEnergyType> for models::pokemon::EnergyType {
    fn from(value: APIEnergyType) -> Self {
        match value {
            APIEnergyType::Fire => models::pokemon::EnergyType::Fire,
            APIEnergyType::Water => models::pokemon::EnergyType::Water,
            APIEnergyType::Grass => models::pokemon::EnergyType::Grass,
            APIEnergyType::Lightning => models::pokemon::EnergyType::Lightning,
            APIEnergyType::Psychic => models::pokemon::EnergyType::Psychic,
            APIEnergyType::Fighting => models::pokemon::EnergyType::Fighting,
            APIEnergyType::Darkness => models::pokemon::EnergyType::Darkness,
            APIEnergyType::Metal => models::pokemon::EnergyType::Metal,
            APIEnergyType::Dragon => models::pokemon::EnergyType::Dragon,
            APIEnergyType::Fairy => models::pokemon::EnergyType::Fairy,
            APIEnergyType::Colorless => models::pokemon::EnergyType::Colorless,
            APIEnergyType::Energy => models::pokemon::EnergyType::Energy,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct APIPokemonCard {
    pub id: String,
    pub name: String,
    #[serde(rename = "setCode")]
    pub set_code: String,
    pub rarity: APIPokemonRarity,
    pub image: String,
    #[serde(rename = "energyTypes")]
    pub energy_types: Vec<APIEnergyType>,
    #[serde(rename = "cardType")]
    pub card_type: String,
    #[serde(rename = "collectorNumber")]
    pub collector_number: String,
    pub description: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub pokedex: Option<i64>,
}

impl From<PokemonCard> for APIPokemonCard {
    fn from(value: PokemonCard) -> Self {
        APIPokemonCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            rarity: value.rarity.into(),
            image: value.image,
            energy_types: value.energy_types.into_iter().map(Into::into).collect(),
            card_type: value.card_type,
            collector_number: value.collector_number,
            description: value.description,
            release_date: value.release_date,
            pokedex: value.pokedex,
        }
    }
}
