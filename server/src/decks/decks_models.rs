use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A card allocated to one zone of one deck.
///
/// Distinct from `collections_models::CollectionCard` in carrying a `board`. It keeps the same
/// owned/wanted pair: a deck records both the copies it has to hand and the ones it still calls
/// for, and a card can be both at once.
#[derive(Serialize, JsonSchema)]
pub struct DeckCard {
    pub id: String,
    /// Which printing/finish this allocation is for; see `collections_models::CardToAdd::finish`.
    #[serde(default)]
    pub finish: String,
    /// Which zone of the deck this belongs to.
    ///
    /// Free-form text the user can rename, rather than a fixed main/side pair: MTG has a mainboard
    /// and sideboard (and a command zone for Commander), Riftbound has a main deck, a legend,
    /// runes, and battlefields, and Pokemon has a single deck. A new deck starts with `"Mainboard"`
    /// and `"Sideboard"`.
    #[serde(default)]
    pub board: String,
    /// Copies the deck has to hand.
    pub quantity: i32,
    /// Copies the deck calls for. Not added to `quantity`: the two overlap, so a deck's size
    /// counts the greater of them per card.
    #[serde(rename = "wantQuantity")]
    pub want_quantity: i32,
    #[serde(default)]
    pub provider: String,
    #[serde(rename = "timeAdded")]
    #[schemars(default = "epoch")]
    pub time_added: DateTime<Utc>,
    #[serde(rename = "timeUpdated")]
    #[schemars(default = "epoch")]
    pub time_updated: DateTime<Utc>,
}

impl From<models::DeckCard> for DeckCard {
    fn from(card: models::DeckCard) -> Self {
        Self {
            id: card.uuid,
            finish: card.finish,
            board: card.board,
            quantity: card.quantity,
            want_quantity: card.want_quantity,
            provider: card.provider,
            time_added: parse_time(&card.time_added),
            time_updated: parse_time(&card.time_updated),
        }
    }
}

/// A change applied to one card on one board.
///
/// Positive adds copies, negative removes them; each count is floored at zero. The two counts move
/// independently, so a copy can be acquired by sending `quantity: 1, wantQuantity: -1`.
///
/// Adding to a board the deck hasn't got declares it, board names being free-form.
#[derive(Deserialize, JsonSchema)]
pub struct DeckCardAdjustment {
    pub id: String,
    #[serde(default)]
    pub finish: String,
    /// Defaults to the deck's first board when omitted.
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub quantity: i32,
    #[serde(rename = "wantQuantity", default)]
    pub want_quantity: i32,
    /// Which system/plugin this card came from. Optional for the same reason as
    /// `collections_models::CardToAdd::provider`: when omitted the server probes the configured
    /// systems for the id.
    #[serde(default)]
    pub provider: Option<String>,
}

/// Moves copies of a card from one of a deck's boards to another.
#[derive(Deserialize, JsonSchema)]
pub struct DeckCardMove {
    pub id: String,
    #[serde(default)]
    pub finish: String,
    #[serde(rename = "fromBoard", default)]
    pub from_board: String,
    #[serde(rename = "toBoard", default)]
    pub to_board: String,
    /// Copies to move, unsigned as a transfer's count is. The two boards already say which way
    /// they go, so there is nothing for a sign to mean.
    #[serde(default)]
    pub quantity: u32,
    #[serde(rename = "wantQuantity", default)]
    pub want_quantity: u32,
}

/// One zone of a deck, and where it sits in the deck's order.
#[derive(Serialize, JsonSchema)]
pub struct DeckBoard {
    pub board: String,
    pub position: i32,
}

impl From<models::DeckBoard> for DeckBoard {
    fn from(board: models::DeckBoard) -> Self {
        Self {
            board: board.board,
            position: board.position,
        }
    }
}

/// Adds a board to a deck. Already having one by that name is a no-op.
#[derive(Deserialize, JsonSchema)]
pub struct BoardCreate {
    pub board: String,
}

/// Renames one of a deck's boards, taking its cards with it.
#[derive(Deserialize, JsonSchema)]
pub struct BoardRename {
    pub from: String,
    pub to: String,
}

/// Sets the order of a deck's boards.
///
/// Boards left out keep their place after the ones listed, so a partial order can't drop one.
#[derive(Deserialize, JsonSchema)]
pub struct BoardReorder {
    pub boards: Vec<String>,
}

/// Which way a transfer goes, and how many copies go that way.
///
/// The count rides on the variant rather than sitting beside it, so the two can't disagree: there
/// is no way to express a direction without a count, and `NonZeroU32` leaves no room for a
/// negative or empty move that the handler would have to reject by hand.
///
/// Adjacently tagged, so the wire form stays two plain fields: `"direction": "intoDeck",
/// "quantity": 3`.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "direction", content = "quantity", rename_all = "camelCase")]
pub enum Transfer {
    /// Out of the collection and onto the deck's board.
    IntoDeck(NonZeroU32),
    /// Off the deck's board and back into the collection.
    OutOfDeck(NonZeroU32),
}

/// Moves copies of a card between a collection and one of a deck's boards.
///
/// A transfer rather than an allocation: the copies leave wherever they came from. Allocating a
/// card to a deck while keeping it in the collection is what the adjust endpoint is for.
#[derive(Deserialize, JsonSchema)]
pub struct DeckCardTransfer {
    pub id: String,
    #[serde(default)]
    pub finish: String,
    /// The collection the copies come from, or go back to.
    pub collection: String,
    /// Defaults to the deck's first board when omitted.
    #[serde(default)]
    pub board: String,
    #[serde(flatten)]
    pub transfer: Transfer,
}

/// How many copies a transfer actually moved, which is capped at what the source holds.
#[derive(Serialize, JsonSchema)]
pub struct DeckCardTransferResult {
    pub moved: i32,
}

/// Whether a deck is physically assembled, and where it is.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DeckMeta {
    /// A built deck is assembled in real life, so its cards are committed and unavailable to
    /// another deck. An unbuilt deck is only a list.
    pub built: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
}

impl From<models::DeckMeta> for DeckMeta {
    fn from(meta: models::DeckMeta) -> Self {
        Self {
            built: meta.built,
            location: meta.location,
            notes: meta.notes,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AvailabilityRequest {
    /// Card ids to report on. Empty yields an empty response.
    pub ids: Vec<String>,
}

/// Where the copies of one card currently are.
#[derive(Serialize, JsonSchema)]
pub struct CardAvailability {
    pub id: String,
    /// Copies owned across every collection.
    pub owned: i32,
    /// Copies allocated to decks that are currently built.
    pub committed: i32,
    /// `owned - committed`. Goes negative when more copies are committed than owned, which is
    /// reported rather than prevented.
    pub free: i32,
    #[serde(rename = "overCommitted")]
    pub over_committed: bool,
    pub holdings: Vec<CardHolding>,
    /// Which collections the owned copies are in, so a deck can show where the ones it is short
    /// of are to be found.
    pub copies: Vec<CardCopies>,
}

/// How many copies of a card one collection holds.
#[derive(Serialize, JsonSchema)]
pub struct CardCopies {
    pub collection: String,
    pub quantity: i32,
}

impl From<models::CardCopies> for CardCopies {
    fn from(c: models::CardCopies) -> Self {
        Self {
            collection: c.collection,
            quantity: c.quantity,
        }
    }
}

/// One built deck's claim on a card, and where that deck physically is.
#[derive(Serialize, JsonSchema)]
pub struct CardHolding {
    pub deck: String,
    pub location: Option<String>,
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub finish: String,
    pub quantity: i32,
}

impl From<models::CardAvailability> for CardAvailability {
    fn from(a: models::CardAvailability) -> Self {
        Self {
            id: a.uuid,
            owned: a.owned,
            committed: a.committed,
            free: a.free,
            over_committed: a.over_committed,
            holdings: a.holdings.into_iter().map(CardHolding::from).collect(),
            copies: a.copies.into_iter().map(CardCopies::from).collect(),
        }
    }
}

impl From<models::CardHolding> for CardHolding {
    fn from(h: models::CardHolding) -> Self {
        Self {
            deck: h.deck,
            location: h.location,
            board: h.board,
            finish: h.finish,
            quantity: h.quantity,
        }
    }
}

fn parse_time(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Fixed placeholder used only for the schema's documented default, for determinism.
///
/// Same reasoning as `collections_models::epoch`.
fn epoch() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
