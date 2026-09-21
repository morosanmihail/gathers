pub mod plugin;
pub mod pokemon;
pub mod riftsqlite;
pub mod scryfall;
pub mod sqlite;
pub(crate) mod sql_helpers;

use models::filters::{UNIQUE_PRINTS, UniqueMode};

/// The `unique` modes shared by both Magic backends, named after Scryfall's `unique=`
/// parameter. `prints` comes first, making it the default: every printing is its own search
/// result and collection entry, rather than Scryfall's own default of one result per card.
pub(crate) fn mtg_unique_modes() -> Vec<UniqueMode> {
    vec![
        UniqueMode::new(UNIQUE_PRINTS, "Prints", "Every printing of a card"),
        UniqueMode::new("cards", "Cards", "One result per card"),
        UniqueMode::new("art", "Art", "One result per distinct artwork"),
    ]
}
