use std::collections::HashMap;

use models::{CardColour, MagicCard};

/// `cardLegalities` columns, in the order they're selected. Also used to build the
/// whitelist for the `legal_in` filter, since the format name is interpolated into SQL.
pub const LEGALITY_FORMATS: &[&str] = &[
    "alchemy",
    "brawl",
    "commander",
    "duel",
    "future",
    "gladiator",
    "historic",
    "legacy",
    "modern",
    "oathbreaker",
    "oldschool",
    "pauper",
    "paupercommander",
    "penny",
    "pioneer",
    "predh",
    "premodern",
    "standard",
    "standardbrawl",
    "timeless",
    "tlr",
    "vintage",
];

#[derive(Debug, PartialEq, Clone)]
pub struct SqlCardIdentifiers {
    pub id: String,
    pub scryfall_id: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SqlCard {
    pub id: String,
    pub name: String,
    pub set_code: String,
    pub rarity: String,
    pub artist: String,
    pub color_identity: String,
    pub text: String,
    pub card_identifiers: SqlCardIdentifiers,
    pub collector_number: String,
    pub subtype: String,
    pub supertype: String,
    pub types: String,
    pub mana_cost: String,
    pub mana_value: f64,
    pub type_line: String,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub keywords: String,
    pub colors: String,
    pub legalities: HashMap<String, String>,
    pub finishes: String,
    pub is_reserved: bool,
    pub is_promo: bool,
    pub is_reprint: bool,
    pub border_color: String,
    pub frame_effects: String,
    pub is_full_art: bool,
    pub watermark: Option<String>,
    pub flavor_text: Option<String>,
    pub set_name: String,
}

/// Splits an mtgjson comma-separated list column (e.g. `"Human, Wizard"`) into trimmed parts.
fn split_list(value: &str) -> Vec<String> {
    if value.is_empty() {
        return vec![];
    }
    value.split(',').map(|s| s.trim().to_string()).collect()
}

impl SqlCard {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let mut legalities = HashMap::new();
        for format in LEGALITY_FORMATS {
            if let Some(status) = row.get::<_, Option<String>>(*format)? {
                legalities.insert((*format).to_string(), status);
            }
        }

        Ok(SqlCard {
            id: row.get("uuid")?,
            name: row.get("name")?,
            set_code: row.get("setCode")?,
            color_identity: row.get("colorIdentity")?,
            text: row.get("text")?,
            rarity: row.get("rarity")?,
            artist: row.get("artist")?,
            card_identifiers: SqlCardIdentifiers {
                scryfall_id: row.get("scryfallId")?,
                id: row.get("uuid")?,
            },
            collector_number: row.get("number")?,
            subtype: row.get("subtypes")?,
            supertype: row.get("supertypes")?,
            types: row.get("types")?,
            mana_cost: row
                .get::<_, Option<String>>("manaCost")?
                .unwrap_or_default(),
            mana_value: row.get::<_, Option<f64>>("manaValue")?.unwrap_or_default(),
            type_line: row.get::<_, Option<String>>("type")?.unwrap_or_default(),
            power: row.get("power")?,
            toughness: row.get("toughness")?,
            loyalty: row.get("loyalty")?,
            defense: row.get("defense")?,
            keywords: row
                .get::<_, Option<String>>("keywords")?
                .unwrap_or_default(),
            colors: row.get::<_, Option<String>>("colors")?.unwrap_or_default(),
            legalities,
            finishes: row
                .get::<_, Option<String>>("finishes")?
                .unwrap_or_default(),
            // mtgjson stores these boolean columns as 1 for true and NULL (not 0) for false.
            is_reserved: row.get::<_, Option<bool>>("isReserved")?.unwrap_or(false),
            is_promo: row.get::<_, Option<bool>>("isPromo")?.unwrap_or(false),
            is_reprint: row.get::<_, Option<bool>>("isReprint")?.unwrap_or(false),
            border_color: row
                .get::<_, Option<String>>("borderColor")?
                .unwrap_or_default(),
            frame_effects: row
                .get::<_, Option<String>>("frameEffects")?
                .unwrap_or_default(),
            is_full_art: row.get::<_, Option<bool>>("isFullArt")?.unwrap_or(false),
            watermark: row.get("watermark")?,
            flavor_text: row.get("flavorText")?,
            set_name: row
                .get::<_, Option<String>>("set_name")?
                .unwrap_or_default(),
        })
    }
}

impl From<SqlCard> for MagicCard {
    fn from(value: SqlCard) -> Self {
        let colours: Vec<CardColour> = value
            .color_identity
            .chars()
            .filter_map(|c| match c {
                'W' | 'w' => Some(CardColour::White),
                'U' | 'u' => Some(CardColour::Blue),
                'B' | 'b' => Some(CardColour::Black),
                'R' | 'r' => Some(CardColour::Red),
                'G' | 'g' => Some(CardColour::Green),
                ' ' => None,
                ',' => None,
                _ => None,
            })
            .collect();
        let colours = if colours.is_empty() {
            vec![CardColour::Colourless]
        } else {
            colours
        };
        let colors: Vec<CardColour> = value
            .colors
            .chars()
            .filter_map(|c| match c {
                'W' | 'w' => Some(CardColour::White),
                'U' | 'u' => Some(CardColour::Blue),
                'B' | 'b' => Some(CardColour::Black),
                'R' | 'r' => Some(CardColour::Red),
                'G' | 'g' => Some(CardColour::Green),
                ' ' => None,
                ',' => None,
                _ => None,
            })
            .collect();
        let subtypes = split_list(&value.subtype);
        let supertypes = split_list(&value.supertype);
        let types = split_list(&value.types);
        let keywords = split_list(&value.keywords);
        let finishes = split_list(&value.finishes);
        let frame_effects = split_list(&value.frame_effects);
        MagicCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            rarity: value.rarity.into(),
            artist: value.artist,
            color_identity: colours,
            text: value.text,
            card_identifiers: models::CardIdentifiers {
                id: value.card_identifiers.id,
                scryfall_id: value.card_identifiers.scryfall_id,
            },
            collector_number: value.collector_number,
            subtypes,
            supertypes,
            types,
            mana_cost: value.mana_cost,
            mana_value: value.mana_value,
            type_line: value.type_line,
            power: value.power,
            toughness: value.toughness,
            loyalty: value.loyalty,
            defense: value.defense,
            keywords,
            colors,
            legalities: value.legalities,
            finishes,
            is_reserved: value.is_reserved,
            is_promo: value.is_promo,
            is_reprint: value.is_reprint,
            border_color: value.border_color,
            frame_effects,
            is_full_art: value.is_full_art,
            watermark: value.watermark,
            flavor_text: value.flavor_text,
            set_name: value.set_name,
        }
    }
}
