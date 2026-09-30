use enum_dispatch::enum_dispatch;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub mod filters;
pub mod mtg;
pub mod pokemon;
pub mod prices;
pub mod riftbound;

pub use mtg::{CardColour, CardIdentifiers, MagicCard, Rarity};
pub use prices::{CardPrices, DEFAULT_CURRENCY, RetailerPrices, parse_price_date};

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

/// What a retrieval system offers to search by, gathered in one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMeta {
    pub sets: Vec<Set>,
    /// Values for `CardSearchFilters::any_types`.
    pub card_types: Vec<String>,
    /// Values for `CardSearchFilters::unique`, default first.
    pub unique_modes: Vec<filters::UniqueMode>,
}

#[derive(Debug, Clone)]
pub struct CollectionCard {
    pub uuid: CardID,
    /// Which printing/finish of this card this row tracks — MTG's `""`
    /// (nonfoil) / `"foil"` / `"etched"`, Pokemon's `""` / `"reverse
    /// holo"` / `"holo"` / etc, or just `""` for a game with only one
    /// version per collector number (Riftbound). `""` is always the
    /// default/primary finish. Distinct finishes of the same `uuid` are
    /// separate rows (separate `(uuid, finish, collection)` keys) — see
    /// `persistence`'s `cards` table — so a UI groups them back together
    /// by `uuid` to show "one card, several finishes".
    pub finish: String,
    pub quantity: i32,
    /// Quantity the owner wants to acquire, independent of `quantity`
    /// already owned. Only meaningful on the `""` (default) finish row —
    /// wanting a card doesn't pin down which finish you're after yet.
    /// Lets a card be tracked as a wishlist entry before any are owned.
    pub want_quantity: i32,
    pub time_added: String,
    pub collection: CollectionID,
    pub provider: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CollectionKind {
    /// A set of *owned* cards.
    #[default]
    Collection,
    /// A set of *allocated* cards (intended to be played in a given format).
    Deck,
}

impl CollectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Deck => "deck",
        }
    }
}

/// Returned when a stored `kind` doesn't match any known variant.
#[derive(Debug, Clone)]
pub struct UnknownCollectionKind(pub String);

impl std::fmt::Display for UnknownCollectionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown collection kind: {:?}", self.0)
    }
}

impl std::error::Error for UnknownCollectionKind {}

impl std::str::FromStr for CollectionKind {
    type Err = UnknownCollectionKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "collection" => Ok(Self::Collection),
            "deck" => Ok(Self::Deck),
            other => Err(UnknownCollectionKind(other.to_string())),
        }
    }
}

/// A card allocated to one zone of one deck.
///
/// The `(deck, uuid, finish, board)` key means the same card can appear on multiple boards of the
/// same deck with independent quantities. There is no `want_quantity`: a deck is a list to be
/// played, not a wishlist.
#[derive(Debug, Clone)]
pub struct DeckCard {
    pub deck: CollectionID,
    pub uuid: CardID,
    /// Which finish this allocation is for; same meaning as `CollectionCard::finish`.
    pub finish: String,
    /// Which zone of the deck: `""` (default), MTG's `"main"`/`"side"`, Riftbound's
    /// `"legend"`/`"runes"`/`"battlefield"`, and so on. Open-ended per game, like `finish`.
    pub board: String,
    pub quantity: i32,
    pub provider: String,
    pub time_added: String,
    pub time_updated: String,
}

/// Per-deck state that a plain collection has no use for.
///
/// A built deck is physically assembled, so its cards are committed and unavailable for
/// another deck. An unbuilt deck is just a list.
#[derive(Debug, Clone, Default)]
pub struct DeckMeta {
    pub deck: CollectionID,
    pub built: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
}

/// Where the copies of one card currently are.
///
/// `owned` counts every copy across all collections; `committed` counts copies allocated to decks
/// that are currently *built*. `free` is what's left, and may go negative, since over-commitment
/// is reported rather than prevented.
#[derive(Debug, Clone)]
pub struct CardAvailability {
    pub uuid: CardID,
    pub owned: i32,
    pub committed: i32,
    pub free: i32,
    pub over_committed: bool,
    /// Which built decks hold this card, and where those decks physically are.
    pub holdings: Vec<CardHolding>,
}

#[derive(Debug, Clone)]
pub struct CardHolding {
    pub deck: CollectionID,
    pub location: Option<String>,
    pub board: String,
    pub finish: String,
    pub quantity: i32,
}
