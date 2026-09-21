//! `unique` search modes for the Pokemon backend. Besides the default `prints`, `species`
//! collapses every card of one Pokémon (Pikachu, Pikachu V, Pikachu VMAX, ...) into a single
//! result, keyed on its National Pokédex number.
//!
//! Filters apply to printings first, then the matches collapse, so the printing that stands
//! for a species is chosen among the matches. All of it happens in Rust over the matching
//! rows: the whole table is only ~21k cards, so there is nothing to gain from streaming.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use ::models::filters::{UNIQUE_PRINTS, UniqueMode};
use rusqlite::Connection;

use super::models::SqlPokemonCard;

/// `UniqueMode::id` for one result per Pokémon species.
pub(super) const UNIQUE_SPECIES: &str = "species";

pub(super) fn unique_modes() -> Vec<UniqueMode> {
    vec![
        UniqueMode::new(UNIQUE_PRINTS, "Prints", "Every printing of a card"),
        UniqueMode::new(
            UNIQUE_SPECIES,
            "Species",
            "One result per Pokémon, by Pokédex number",
        ),
    ]
}

/// Pokédex numbers at or above this are the scraper's placeholder for "no species" (trainers
/// and energy), not a real Pokémon.
const NO_SPECIES: i64 = 100_000;

/// Lowercases a name for matching, and folds the few characters that differ between the
/// Pokédex's spelling and card names (`Flabébé` vs `Flabebe`, `Nidoran♀` vs `Nidoran F`).
fn fold(name: &str) -> String {
    name.to_lowercase()
        .replace('é', "e")
        .replace('♀', " f")
        .replace('♂', " m")
}

/// Splits a folded name into words, dropping the brackets and ampersands that surround them
/// (`"(Full Art)"`, `"Pikachu & Zekrom GX"`) but keeping hyphens and dots, which species
/// names use (`Ho-Oh`, `Mr. Mime`).
fn words(folded: &str) -> Vec<&str> {
    folded
        .split_whitespace()
        .map(|w| w.trim_matches(['(', ')', '[', ']', ',', '&']))
        .filter(|w| !w.is_empty())
        .collect()
}

/// The Pokédex, for finding the species of a card the scraper has no number for. Recent sets
/// often come through with none, though the card name still says which Pokémon it is.
pub(super) struct SpeciesIndex {
    /// Folded species name (one to three words) to Pokédex number.
    by_name: HashMap<String, i64>,
    /// The reverse: each Pokédex number's folded name.
    names: HashMap<i64, String>,
}

/// Cuts a card name back to the name it is listed under: the scraper appends the collector
/// number (`Crustle - 186/182`) and printing notes (`Pikachu (Full Art)`), which aren't part
/// of the card. Folded, like the index's names.
fn base_name(card_name: &str) -> String {
    let folded = fold(card_name);
    let end = [" - ", " ("].iter().filter_map(|cut| folded.find(cut)).min();
    words(&folded[..end.unwrap_or(folded.len())]).join(" ")
}

impl SpeciesIndex {
    /// Loads the `pokedex` table. A database without one gives an index that finds nothing,
    /// leaving cards to their scraped number alone.
    pub fn load(conn: &Connection) -> Self {
        let names: HashMap<i64, String> = conn
            .prepare("SELECT id, name FROM pokedex")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default()
            .into_iter()
            .map(|(id, name)| (id, words(&fold(&name)).join(" ")))
            .collect();
        let by_name = names.iter().map(|(id, name)| (name.clone(), *id)).collect();
        Self { by_name, names }
    }

    /// The Pokédex number of the first species named in `card_name`. The first, not the
    /// longest: tag-team cards list several Pokémon and are filed under the first.
    fn find_in(&self, card_name: &str) -> Option<i64> {
        let folded = fold(card_name);
        let words = words(&folded);
        (0..words.len()).find_map(|start| {
            // Longest first, so `Mr. Mime` isn't read as some shorter species.
            (1..=3).rev().find_map(|len| {
                let end = start + len;
                (end <= words.len())
                    .then(|| self.by_name.get(&words[start..end].join(" ")).copied())
                    .flatten()
            })
        })
    }

    /// The Pokédex number of the Pokémon `card` is, if it is one.
    ///
    /// Only Pokémon cards have a species. Trainer and energy cards are left alone even when
    /// the scraper gave them a number (`Flying Pikachu` is an item, not a Pikachu), and a card
    /// with no card type is treated as a Pokémon since recent sets arrive without one.
    fn species_of(&self, card: &SqlPokemonCard) -> Option<i64> {
        matches!(card.card_type.trim(), "" | "Pokemon")
            .then(|| {
                card.pokedex
                    .filter(|n| (1..NO_SPECIES).contains(n))
                    .or_else(|| self.find_in(&card.name))
            })
            .flatten()
    }

    /// Whether `card` is listed under the species' own name (`Pikachu`), rather than a form
    /// or team-up of it (`Pikachu V`, `Pikachu & Zekrom GX`, `Ash's Pikachu`).
    fn is_plain(&self, card: &SqlPokemonCard, species: i64) -> bool {
        self.names.get(&species).is_some_and(|name| *name == base_name(&card.name))
    }
}

/// How well a printing represents its species; lower is better. Best is a regular printing of
/// the plain Pokémon; a form or team-up of it ranks lower, a variant printing (full art,
/// secret rare, prerelease, ...) lower still, and a promo last. The newest wins within that.
fn rank(
    card: &SqlPokemonCard,
    expansion_release: Option<&str>,
    plain: bool,
) -> (u8, bool, Reverse<i64>, String) {
    const VARIANTS: [&str; 8] = [
        "(full art)", "(secret", "(alternate", "(prerelease", "pattern)", "(shiny", "holo)", "[",
    ];
    let name = card.name.to_lowercase();
    let is_promo = card.rarity == "Promo" || card.set_code.contains("Promo");
    let is_variant = VARIANTS.iter().any(|v| name.contains(v));
    let penalty = if is_promo { 2 } else { u8::from(is_variant) };

    // Dates come as `2023-03-31T00:00:00Z` or with milliseconds; the day is what matters. A
    // card without one falls back to its expansion's.
    let date = card
        .release_date
        .as_deref()
        .filter(|d| !d.is_empty())
        .or(expansion_release)
        .and_then(|d| d.get(..10))
        .and_then(|d| d.replace('-', "").parse::<i64>().ok())
        .unwrap_or(0);
    (penalty, !plain, Reverse(date), card.id.clone())
}

/// `cards` (in the order they should be listed) with all but the best printing of each species
/// removed. Each survivor keeps its own position, so the list stays in sort order.
pub(super) fn collapse_by_species(
    conn: &Connection,
    cards: Vec<(SqlPokemonCard, Option<String>)>,
) -> Vec<SqlPokemonCard> {
    let index = SpeciesIndex::load(conn);
    let species: Vec<Option<i64>> = cards.iter().map(|(card, _)| index.species_of(card)).collect();
    // A card with no species collapses with nothing: it keys on its own id.
    let keys: Vec<String> = cards
        .iter()
        .zip(&species)
        .map(|((card, _), species)| match species {
            Some(number) => format!("species:{number}"),
            None => format!("card:{}", card.id),
        })
        .collect();
    let rank_of = |i: usize| {
        let (card, expansion_release) = &cards[i];
        let plain = species[i].is_some_and(|number| index.is_plain(card, number));
        rank(card, expansion_release.as_deref(), plain)
    };

    let mut best: HashMap<&str, usize> = HashMap::new();
    for (i, key) in keys.iter().enumerate() {
        best.entry(key)
            .and_modify(|current| {
                if rank_of(i) < rank_of(*current) {
                    *current = i;
                }
            })
            .or_insert(i);
    }
    let winners: HashSet<usize> = best.into_values().collect();
    cards
        .into_iter()
        .enumerate()
        .filter(|(i, _)| winners.contains(i))
        .map(|(_, (card, _))| card)
        .collect()
}
