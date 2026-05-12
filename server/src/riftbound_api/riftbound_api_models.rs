use models::riftbound::RiftboundCard;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum APICardDomain {
    Calm,
    Chaos,
    Fury,
    Mind,
    Body,
    Order,
    Colorless,
}

impl From<models::riftbound::CardDomain> for APICardDomain {
    fn from(value: models::riftbound::CardDomain) -> Self {
        match value {
            models::riftbound::CardDomain::Calm => APICardDomain::Calm,
            models::riftbound::CardDomain::Chaos => APICardDomain::Chaos,
            models::riftbound::CardDomain::Fury => APICardDomain::Fury,
            models::riftbound::CardDomain::Mind => APICardDomain::Mind,
            models::riftbound::CardDomain::Body => APICardDomain::Body,
            models::riftbound::CardDomain::Order => APICardDomain::Order,
            models::riftbound::CardDomain::Colorless => APICardDomain::Colorless,
        }
    }
}

impl From<APICardDomain> for models::riftbound::CardDomain {
    fn from(value: APICardDomain) -> Self {
        match value {
            APICardDomain::Calm => models::riftbound::CardDomain::Calm,
            APICardDomain::Chaos => models::riftbound::CardDomain::Chaos,
            APICardDomain::Fury => models::riftbound::CardDomain::Fury,
            APICardDomain::Mind => models::riftbound::CardDomain::Mind,
            APICardDomain::Body => models::riftbound::CardDomain::Body,
            APICardDomain::Order => models::riftbound::CardDomain::Order,
            APICardDomain::Colorless => models::riftbound::CardDomain::Colorless,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct APIRiftboundCard {
    pub id: String,
    pub name: String,
    #[serde(rename = "setCode")]
    pub set_code: String,
    #[serde(rename = "collectorNumber")]
    pub collector_number: String,
    pub rarity: RBRarity,
    pub artists: Vec<String>,
    pub domains: Vec<APICardDomain>,
    pub text: String,
    pub image: String,
}

impl From<RiftboundCard> for APIRiftboundCard {
    fn from(value: RiftboundCard) -> Self {
        APIRiftboundCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            collector_number: value.collector_number,
            rarity: value.rarity.into(),
            artists: value.artists,
            domains: value.domains.into_iter().map(Into::into).collect(),
            text: value.text,
            image: value.image,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum RBRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Showcase,
}

impl std::fmt::Display for RBRarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RBRarity::Common => write!(f, "Common"),
            RBRarity::Uncommon => write!(f, "Uncommon"),
            RBRarity::Rare => write!(f, "Rare"),
            RBRarity::Epic => write!(f, "Epic"),
            RBRarity::Showcase => write!(f, "Showcase"),
        }
    }
}

impl From<models::riftbound::RBRarity> for RBRarity {
    fn from(value: models::riftbound::RBRarity) -> Self {
        match value {
            models::riftbound::RBRarity::Common => RBRarity::Common,
            models::riftbound::RBRarity::Uncommon => RBRarity::Uncommon,
            models::riftbound::RBRarity::Rare => RBRarity::Rare,
            models::riftbound::RBRarity::Epic => RBRarity::Epic,
            models::riftbound::RBRarity::Showcase => RBRarity::Showcase,
        }
    }
}
