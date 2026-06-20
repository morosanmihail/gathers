use serde::{Deserialize, Serialize};

/// A single Pokemon TCG card (metadata only, no price history).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Card {
    pub card_id: String,
    pub id_tcgp: i64,
    pub name: String,
    pub exp_id_tcgp: String,
    pub exp_code_tcgp: String,
    pub exp_name: String,
    pub exp_card_number: String,
    pub rarity: String,
    pub img: String,
    pub price: Option<f64>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub energy_type: Option<String>,
    pub card_type: Option<String>,
    pub pokedex: Option<i64>,
    pub variants: Vec<String>,
}

/// A TCG expansion / set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Expansion {
    pub name: String,
    pub series: String,
    /// JSON-encoded array of TCGPlayer set URL names, e.g. `["sword-shield-base-set"]`
    pub tcg_name: String,
    pub number_of_cards: i64,
    pub logo_url: String,
    pub symbol_url: String,
    pub release_date: String,
}

/// A National Pokedex entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pokemon {
    pub id: i64,
    pub name: String,
}

// ── Intermediate scraper types ────────────────────────────────────────────────

/// A set as returned by Serebii.net (before DB enrichment).
#[derive(Debug, Clone)]
pub struct SerebiiSet {
    pub name: String,
    pub page: String,
    pub logo: String,
    pub symbol: String,
    pub number_of_cards: i64,
}

/// A set listing from the TCGPlayer search API.
#[derive(Debug, Clone)]
pub struct TcgpSet {
    pub url_val: String,
    pub value: String,
    pub count: i64,
}

/// A set code entry from the TCGPlayer mass-entry API.
#[derive(Debug, Clone)]
pub struct TcgpCode {
    pub name: String,
    pub code: String,
}
