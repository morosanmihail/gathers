use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A card allocated to one zone of one deck.
///
/// Distinct from `collections_models::CollectionCard`: a deck entry carries a `board` and has no
/// want quantity, since a deck is a list to be played rather than a record of what is owned.
#[derive(Serialize, JsonSchema)]
pub struct DeckCard {
    pub id: String,
    /// Which printing/finish this allocation is for; see `collections_models::CardToAdd::finish`.
    #[serde(default)]
    pub finish: String,
    /// Which zone of the deck this belongs to.
    ///
    /// Open-ended per game rather than a fixed main/side pair: MTG has a mainboard and sideboard
    /// (and a command zone for Commander), Riftbound has a main deck, a legend, runes, and
    /// battlefields, and Pokemon has a single deck. `""` is the default zone.
    #[serde(default)]
    pub board: String,
    pub quantity: i32,
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
            provider: card.provider,
            time_added: parse_time(&card.time_added),
            time_updated: parse_time(&card.time_updated),
        }
    }
}

/// A change applied to one card on one board.
///
/// Positive adds copies, negative removes them; the result is floored at zero.
#[derive(Deserialize, JsonSchema)]
pub struct DeckCardAdjustment {
    pub id: String,
    #[serde(default)]
    pub finish: String,
    #[serde(default)]
    pub board: String,
    pub quantity: i32,
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
    pub quantity: i32,
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
