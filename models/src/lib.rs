use enum_dispatch::enum_dispatch;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub mod filters;
pub mod mtg;
pub mod pokemon;
pub mod prices;
pub mod riftbound;

pub use mtg::{CardColour, CardIdentifiers, MagicCard, Rarity};
pub use prices::{CardPrices, RetailerPrices};

use crate::pokemon::PokemonCard;
use crate::riftbound::RiftboundCard;

pub type Artist = String;
pub type CardID = String;
pub type SetCode = String;
pub type CardText = String;
pub type CardName = String;
pub type SetName = String;
pub type CollectionID = String;
pub type CollectorNumber = String;

// MagicCard carries much more mtgjson metadata than the other variants; boxing it would
// mean threading Box<MagicCard> through every `Card::Magic` match arm across the
// workspace for a minor size win, so the size difference is accepted instead.
#[allow(clippy::large_enum_variant)]
#[enum_dispatch]
#[derive(Debug, Clone)]
pub enum Card {
    Magic(MagicCard),
    Riftbound(RiftboundCard),
    Pokemon(PokemonCard),
}

#[enum_dispatch(Card)]
pub trait CardTrait {
    fn get_set(&self) -> SetCode;
    fn get_collector_number(&self) -> CollectorNumber;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Set {
    pub name: SetName,
    pub code: SetCode,
}

#[derive(Debug, Clone)]
pub struct CollectionCard {
    pub uuid: CardID,
    pub quantity: i32,
    pub foil_quantity: i32,
    /// Quantity the owner wants to acquire, independent of `quantity`/`foil_quantity`
    /// already owned. Lets a card be tracked as a wishlist entry before any are owned.
    pub want_quantity: i32,
    pub time_added: String,
    pub collection: CollectionID,
    pub provider: String,
}

