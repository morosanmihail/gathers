use std::collections::HashMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{Artist, CardID, CardName, CardText, CardTrait, CollectorNumber, SetCode};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MagicCard {
    pub id: CardID,
    pub name: CardName,
    pub set_code: SetCode,
    pub collector_number: CollectorNumber,
    pub rarity: Rarity,
    pub artist: Artist,
    pub color_identity: Vec<CardColour>,
    pub text: CardText,
    pub card_identifiers: CardIdentifiers,
    pub subtypes: Vec<String>,
    pub supertypes: Vec<String>,
    pub types: Vec<String>,
    pub mana_cost: String,
    pub mana_value: f64,
    /// Full type line, e.g. "Legendary Artifact Creature — Autobot".
    pub type_line: String,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub keywords: Vec<String>,
    /// Colours actually printed on the card (as opposed to `color_identity`).
    pub colors: Vec<CardColour>,
    /// Format name -> legality status (e.g. "modern" -> "Legal").
    pub legalities: HashMap<String, String>,
    pub finishes: Vec<String>,
    /// Display name of this printing's special foil treatment (e.g. "Surge Foil",
    /// "Step-and-Compleat Foil"), from its promo types — see [`foil_style`]. Only describes
    /// the `foil` finish: a printing that also comes in nonfoil is still a plain nonfoil card.
    /// `None` for ordinary foils and cards with no foil.
    #[serde(default)]
    pub foil_style: Option<String>,
    pub is_reserved: bool,
    pub is_promo: bool,
    pub is_reprint: bool,
    pub border_color: String,
    pub frame_effects: Vec<String>,
    pub is_full_art: bool,
    pub watermark: Option<String>,
    pub flavor_text: Option<String>,
    pub set_name: String,
}

impl CardTrait for MagicCard {
    fn get_set(&self) -> SetCode {
        self.set_code.clone()
    }

    fn get_collector_number(&self) -> CollectorNumber {
        self.collector_number.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Mythic,
    Special,
    Bonus,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CardIdentifiers {
    pub id: String,
    pub scryfall_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CardColour {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colourless,
    Multicoloured,
}

impl Default for MagicCard {
    fn default() -> MagicCard {
        MagicCard {
            id: "".to_string(),
            name: "".to_string(),
            set_code: "".to_string(),
            rarity: Rarity::Common,
            artist: "".to_string(),
            color_identity: vec![],
            text: "".to_string(),
            card_identifiers: CardIdentifiers {
                id: "-1".to_string(),
                scryfall_id: "".to_string(),
            },
            collector_number: "".to_string(),
            subtypes: vec![],
            supertypes: vec![],
            types: vec![],
            mana_cost: "".to_string(),
            mana_value: 0.0,
            type_line: "".to_string(),
            power: None,
            toughness: None,
            loyalty: None,
            defense: None,
            keywords: vec![],
            colors: vec![],
            legalities: HashMap::new(),
            finishes: vec![],
            foil_style: None,
            is_reserved: false,
            is_promo: false,
            is_reprint: false,
            border_color: "".to_string(),
            frame_effects: vec![],
            is_full_art: false,
            watermark: None,
            flavor_text: None,
            set_name: "".to_string(),
        }
    }
}

/// Foil treatments whose promo type doesn't end in "foil" (see [`foil_style_key`]). A
/// promo type starting with one of these counts too, for colour variants like `neoninkred`.
const FOILS_NAMED_OTHERWISE: &[&str] = &[
    "oilslick",
    "stepandcompleat",
    "doublerainbow",
    "textured",
    "embossed",
    "gilded",
    "neonink",
];

/// Display names that can't be generated from the promo type, which runs words together.
const FOIL_LABEL_OVERRIDES: &[(&str, &str)] = &[
    ("stepandcompleat", "Step-and-Compleat Foil"),
    ("chocobotrackfoil", "Chocobo Track Foil"),
    ("firstplacefoil", "First Place Foil"),
    ("dragonscalefoil", "Dragon Scale Foil"),
    ("doublerainbow", "Double Rainbow Foil"),
    ("oilslick", "Oil Slick Foil"),
    ("neonink", "Neon Ink Foil"),
];

/// The foil style a promo type (mtgjson `promoTypes`, Scryfall `promo_types`) names, as a
/// key shared by all its variants, or `None` when it isn't a foil treatment. MTGJSON has no
/// such flag: a promo type is one when its name ends in "foil" (`surgefoil`, `raisedfoil`)
/// or it's one of the few named otherwise ([`FOILS_NAMED_OTHERWISE`]).
pub fn foil_style_key(promo_type: &str) -> Option<&str> {
    if let Some(key) = FOILS_NAMED_OTHERWISE.iter().find(|k| promo_type.starts_with(*k)) {
        return Some(key);
    }
    (promo_type.len() > "foil".len() && promo_type.ends_with("foil")).then_some(promo_type)
}

/// The display name of the foil style `key` (from [`foil_style_key`]): "surgefoil" reads
/// "Surge Foil", "textured" "Textured Foil", except for the [`FOIL_LABEL_OVERRIDES`].
pub fn foil_style_label(key: &str) -> String {
    if let Some((_, label)) = FOIL_LABEL_OVERRIDES.iter().find(|(k, _)| *k == key) {
        return (*label).to_string();
    }
    let stem = key.strip_suffix("foil").unwrap_or(key);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => format!("{}{} Foil", first.to_uppercase(), chars.as_str()),
        None => "Foil".to_string(),
    }
}

/// The display name of the special foil treatment named in `promo_types`, if any (see
/// [`MagicCard::foil_style`]). When there are several, a treatment named otherwise wins over
/// a plain "...foil" one, since those are the specific looks (ONE's oil slick foils are also
/// tagged `raisedfoil`); ties go alphabetically.
pub fn foil_style<S: AsRef<str>>(promo_types: &[S]) -> Option<String> {
    promo_types
        .iter()
        .filter_map(|p| foil_style_key(p.as_ref()))
        .min_by_key(|key| (!FOILS_NAMED_OTHERWISE.contains(key), *key))
        .map(foil_style_label)
}

/// A foil style found in a card database, with every promo type that names it.
#[derive(Debug, Clone, PartialEq)]
pub struct FoilStyle {
    /// Display name, e.g. "Surge Foil".
    pub label: String,
    /// The promo types meaning this style, e.g. `["neonink", "neoninkred", ...]`.
    pub promo_types: Vec<String>,
}

/// Groups the distinct promo types a database uses into its foil styles, sorted by label.
/// Promo types that aren't foil treatments are dropped.
pub fn foil_styles<S: AsRef<str>>(promo_types: impl IntoIterator<Item = S>) -> Vec<FoilStyle> {
    let mut by_key: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for p in promo_types {
        let p = p.as_ref();
        if let Some(key) = foil_style_key(p) {
            let types = by_key.entry(key.to_string()).or_default();
            if !types.iter().any(|t| t == p) {
                types.push(p.to_string());
            }
        }
    }
    let mut styles: Vec<FoilStyle> = by_key
        .into_iter()
        .map(|(key, promo_types)| FoilStyle { label: foil_style_label(&key), promo_types })
        .collect();
    styles.sort_by(|a, b| a.label.cmp(&b.label));
    styles
}

/// Every finish name an MTG search can filter by: the plain finishes, then each foil style
/// in `styles`. These are the labels the UI shows for a finish, so a filter reads the same
/// as a card.
pub fn finish_names(styles: &[FoilStyle]) -> Vec<String> {
    ["Normal", "Foil", "Etched"]
        .into_iter()
        .map(String::from)
        .chain(styles.iter().map(|s| s.label.clone()))
        .collect()
}

impl Display for CardColour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardColour::Red => write!(f, "R"),
            CardColour::White => write!(f, "W"),
            CardColour::Blue => write!(f, "U"),
            CardColour::Green => write!(f, "G"),
            CardColour::Black => write!(f, "B"),
            CardColour::Multicoloured => write!(f, "_"),
            CardColour::Colourless => write!(f, "C"),
        }
    }
}

impl From<&str> for CardColour {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "w" | "white" => CardColour::White,
            "u" | "blue" => CardColour::Blue,
            "b" | "black" => CardColour::Black,
            "r" | "red" => CardColour::Red,
            "g" | "green" => CardColour::Green,
            "c" | "colourless" => CardColour::Colourless,
            "m" | "multicoloured" => CardColour::Multicoloured,
            _ => CardColour::Colourless,
        }
    }
}

impl std::fmt::Display for Rarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rarity::Common => write!(f, "Common"),
            Rarity::Uncommon => write!(f, "Uncommon"),
            Rarity::Rare => write!(f, "Rare"),
            Rarity::Mythic => write!(f, "Mythic"),
            Rarity::Special => write!(f, "Special"),
            Rarity::Bonus => write!(f, "Bonus"),
        }
    }
}

impl Rarity {
    pub fn to_single_string(&self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Mythic => "mythic",
            Rarity::Special => "special",
            Rarity::Bonus => "bonus",
        }
    }
}

impl From<String> for Rarity {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Common" | "common" | "c" => Rarity::Common,
            "Uncommon" | "uncommon" | "u" => Rarity::Uncommon,
            "Rare" | "rare" | "r" => Rarity::Rare,
            "Mythic" | "mythic" | "m" => Rarity::Mythic,
            "Special" | "special" => Rarity::Special,
            "Bonus" | "bonus" | "b" => Rarity::Bonus,
            _ => Rarity::Bonus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_card_default() {
        let c = MagicCard::default();
        assert!(c.id.is_empty());
        assert!(c.name.is_empty());
        assert_eq!(c.rarity, Rarity::Common);
        assert_eq!(c.card_identifiers.id, "-1");
        assert!(c.color_identity.is_empty());
    }

    #[test]
    fn test_foil_style() {
        assert_eq!(foil_style(&["universesbeyond", "surgefoil"]).as_deref(), Some("Surge Foil"));
        assert_eq!(foil_style(&["oilslick", "raisedfoil"]).as_deref(), Some("Oil Slick Foil"));
        assert_eq!(foil_style(&["raisedfoil"]).as_deref(), Some("Raised Foil"));
        assert_eq!(foil_style(&["neoninkred"]).as_deref(), Some("Neon Ink Foil"));
        assert_eq!(foil_style(&["stepandcompleat"]).as_deref(), Some("Step-and-Compleat Foil"));
        assert_eq!(foil_style(&["textured"]).as_deref(), Some("Textured Foil"));
        // A new "...foil" promo type is picked up without being listed anywhere.
        assert_eq!(foil_style(&["sparklefoil"]).as_deref(), Some("Sparkle Foil"));
        assert_eq!(foil_style(&["boosterfun", "prerelease", "fnm"]), None);
        assert_eq!(foil_style::<&str>(&[]), None);
    }

    #[test]
    fn test_foil_styles_group_variants() {
        let styles = foil_styles(["surgefoil", "neonink", "neoninkred", "prerelease", "surgefoil"]);
        assert_eq!(
            styles,
            vec![
                FoilStyle {
                    label: "Neon Ink Foil".into(),
                    promo_types: vec!["neonink".into(), "neoninkred".into()],
                },
                FoilStyle { label: "Surge Foil".into(), promo_types: vec!["surgefoil".into()] },
            ]
        );
        assert_eq!(finish_names(&styles), vec!["Normal", "Foil", "Etched", "Neon Ink Foil", "Surge Foil"]);
    }

    #[test]
    fn test_card_colour_display() {
        assert_eq!(CardColour::White.to_string(), "W");
        assert_eq!(CardColour::Blue.to_string(), "U");
        assert_eq!(CardColour::Black.to_string(), "B");
        assert_eq!(CardColour::Red.to_string(), "R");
        assert_eq!(CardColour::Green.to_string(), "G");
        assert_eq!(CardColour::Colourless.to_string(), "C");
        assert_eq!(CardColour::Multicoloured.to_string(), "_");
    }

    #[test]
    fn test_card_colour_from_string() {
        assert_eq!(CardColour::from("w"), CardColour::White);
        assert_eq!(CardColour::from("white"), CardColour::White);
        assert_eq!(CardColour::from("u"), CardColour::Blue);
        assert_eq!(CardColour::from("blue"), CardColour::Blue);
        assert_eq!(CardColour::from("b"), CardColour::Black);
        assert_eq!(CardColour::from("black"), CardColour::Black);
        assert_eq!(CardColour::from("r"), CardColour::Red);
        assert_eq!(CardColour::from("red"), CardColour::Red);
        assert_eq!(CardColour::from("g"), CardColour::Green);
        assert_eq!(CardColour::from("green"), CardColour::Green);
        assert_eq!(CardColour::from("c"), CardColour::Colourless);
        assert_eq!(CardColour::from("colourless"), CardColour::Colourless);
        assert_eq!(CardColour::from("m"), CardColour::Multicoloured);
        assert_eq!(CardColour::from("multicoloured"), CardColour::Multicoloured);
        assert_eq!(CardColour::from("UNKNOWN"), CardColour::Colourless);
    }

    #[test]
    fn test_card_colour_from_string_case_insensitive() {
        assert_eq!(CardColour::from("WHITE"), CardColour::White);
        assert_eq!(CardColour::from("RED"), CardColour::Red);
    }

    #[test]
    fn test_rarity_display() {
        assert_eq!(Rarity::Common.to_string(), "Common");
        assert_eq!(Rarity::Uncommon.to_string(), "Uncommon");
        assert_eq!(Rarity::Rare.to_string(), "Rare");
        assert_eq!(Rarity::Mythic.to_string(), "Mythic");
        assert_eq!(Rarity::Special.to_string(), "Special");
        assert_eq!(Rarity::Bonus.to_string(), "Bonus");
    }

    #[test]
    fn test_rarity_to_single_string() {
        assert_eq!(Rarity::Common.to_single_string(), "common");
        assert_eq!(Rarity::Uncommon.to_single_string(), "uncommon");
        assert_eq!(Rarity::Rare.to_single_string(), "rare");
        assert_eq!(Rarity::Mythic.to_single_string(), "mythic");
        assert_eq!(Rarity::Special.to_single_string(), "special");
        assert_eq!(Rarity::Bonus.to_single_string(), "bonus");
    }

    #[test]
    fn test_rarity_from_string() {
        assert_eq!(Rarity::from("Common".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("common".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("c".to_string()), Rarity::Common);
        assert_eq!(Rarity::from("Uncommon".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("uncommon".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("u".to_string()), Rarity::Uncommon);
        assert_eq!(Rarity::from("Rare".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("rare".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("r".to_string()), Rarity::Rare);
        assert_eq!(Rarity::from("Mythic".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("mythic".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("m".to_string()), Rarity::Mythic);
        assert_eq!(Rarity::from("Special".to_string()), Rarity::Special);
        assert_eq!(Rarity::from("UNKNOWN".to_string()), Rarity::Bonus);
        assert_eq!(Rarity::from("".to_string()), Rarity::Bonus);
    }

    #[test]
    fn test_rarity_display_roundtrip_via_from() {
        for r in [
            Rarity::Common,
            Rarity::Uncommon,
            Rarity::Rare,
            Rarity::Mythic,
            Rarity::Special,
        ] {
            assert_eq!(Rarity::from(r.to_string()), r);
        }
    }
}
