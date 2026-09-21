//! `unique` search modes for the Riftbound backend. Besides the default `prints`, `cards`
//! collapses every printing of a card into one result: the same name and rules text, whatever
//! its set, art variant or collector number.
//!
//! Name alone isn't enough, since different cards share a name (two Setts, two Ahris), which
//! is why the text is part of the key. Filters apply to printings first, then the matches
//! collapse. It all happens in Rust over the matching rows, as the whole table is only ~1.2k
//! cards.

use std::collections::{HashMap, HashSet};

use ::models::filters::{UNIQUE_PRINTS, UniqueMode};

use super::models::SqlCard;

/// `UniqueMode::id` for one result per card.
pub(super) const UNIQUE_CARDS: &str = "cards";

pub(super) fn unique_modes() -> Vec<UniqueMode> {
    vec![
        UniqueMode::new(UNIQUE_PRINTS, "Prints", "Every printing of a card"),
        UniqueMode::new(
            UNIQUE_CARDS,
            "Cards",
            "One result per card: same name and text across sets and art variants",
        ),
    ]
}

/// The rules text with its markup and spacing removed, so printings of a card compare equal
/// however the text was wrapped.
fn normalized_text(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                plain.push(' ');
            }
            _ if !in_tag => plain.push(c),
            _ => {}
        }
    }
    plain.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// What printings of one card share. A card with no text (runes, tokens) is told apart by
/// name alone.
fn key(card: &SqlCard) -> (String, String) {
    (card.name.to_lowercase(), normalized_text(&card.text))
}

/// How far a printing is from the card's regular one; lower is better. Ids read
/// `{set}-{number}-{total}` for a regular printing, with the number suffixed `a` for an
/// alternate art (`ogn-164a-298`), a `-star-` marker for a star variant of the same art
/// (`sfd-232-star-221`), and `sp` before the number for a special (`ven-sp4-006`).
pub(super) fn variant_rank(id: &str) -> u8 {
    let rest = id.split_once('-').map_or(id, |(_, rest)| rest);
    let number = rest.split('-').next().unwrap_or_default();
    if number.starts_with("sp") {
        3
    } else if number.len() > 1
        && number.ends_with(|c: char| c.is_ascii_alphabetic())
        && number[..number.len() - 1].chars().all(|c| c.is_ascii_digit())
    {
        2
    } else if rest.contains("-star-") {
        1
    } else {
        0
    }
}

/// `cards` (in the order they should be listed) with all but one printing of each card
/// removed. Each survivor keeps its own position, so the list stays in sort order.
///
/// The card is represented by its regular printing over a variant. The data has no release
/// dates to prefer a newer one, so ties go to the lowest id, i.e. the earliest set.
pub(super) fn collapse_by_card(cards: Vec<SqlCard>) -> Vec<SqlCard> {
    let keys: Vec<_> = cards.iter().map(key).collect();
    let rank = |i: usize| (variant_rank(&cards[i].id), &cards[i].id);

    let mut best: HashMap<&(String, String), usize> = HashMap::new();
    for (i, key) in keys.iter().enumerate() {
        best.entry(key)
            .and_modify(|current| {
                if rank(i) < rank(*current) {
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
        .map(|(_, card)| card)
        .collect()
}
