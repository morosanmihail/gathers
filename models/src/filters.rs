use serde::{Deserialize, Deserializer, Serialize};

use crate::{CardColour, Rarity, pokemon::EnergyType, riftbound::CardDomain};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SortField {
    #[default]
    Name,
    Rarity,
    SetCode,
    CollectorNumber,
    Artist,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CardSearchFilters {
    pub name: Option<String>,
    #[serde(alias = "colorIdentities")]
    pub color_identities: Option<Vec<CardColour>>,
    #[serde(alias = "setCode")]
    pub set_code: Option<String>,
    #[serde(alias = "collectorNumber")]
    pub collector_number: Option<String>,
    pub artist: Option<String>,
    pub text: Option<String>,
    #[serde(default, deserialize_with = "empty_string_to_none")]
    pub rarity: Option<Rarity>,
    pub subtypes: Option<Vec<String>>,
    pub supertypes: Option<String>,
    pub types: Option<Vec<String>>,
    /// Riftbound-only: filter by one or more card domains.
    pub domains: Option<Vec<CardDomain>>,
    /// Pokemon-only: filter by one or more energy types.
    #[serde(alias = "energyTypes")]
    pub energy_types: Option<Vec<EnergyType>>,
    #[serde(alias = "sortBy")]
    pub sort_by: Option<SortField>,
    #[serde(alias = "sortOrder")]
    pub sort_order: Option<SortOrder>,
}

impl CardSearchFilters {
    pub fn new() -> Self {
        Self {
            name: None,
            color_identities: None,
            set_code: None,
            collector_number: None,
            artist: None,
            text: None,
            rarity: None,
            subtypes: None,
            supertypes: None,
            types: None,
            domains: None,
            energy_types: None,
            sort_by: None,
            sort_order: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_color_identities(mut self, identities: Vec<CardColour>) -> Self {
        self.color_identities = Some(identities);
        self
    }

    pub fn with_set_code(mut self, set_code: impl Into<String>) -> Self {
        self.set_code = Some(set_code.into());
        self
    }

    pub fn with_collector_number(mut self, collector_number: impl Into<String>) -> Self {
        self.collector_number = Some(collector_number.into());
        self
    }

    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn with_rarity(mut self, rarity: Rarity) -> Self {
        self.rarity = Some(rarity);
        self
    }

    pub fn with_subtypes(mut self, subtypes: Vec<String>) -> Self {
        self.subtypes = Some(subtypes);
        self
    }

    pub fn with_supertypes(mut self, supertypes: impl Into<String>) -> Self {
        self.supertypes = Some(supertypes.into());
        self
    }

    pub fn with_types(mut self, types: Vec<String>) -> Self {
        self.types = Some(types);
        self
    }

    pub fn with_domains(mut self, domains: Vec<CardDomain>) -> Self {
        self.domains = Some(domains);
        self
    }

    pub fn with_energy_types(mut self, energy_types: Vec<EnergyType>) -> Self {
        self.energy_types = Some(energy_types);
        self
    }

    pub fn with_sort_by(mut self, sort_by: SortField) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    pub fn with_sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_field_serde_roundtrip() {
        for field in [
            SortField::Name,
            SortField::Rarity,
            SortField::SetCode,
            SortField::CollectorNumber,
            SortField::Artist,
        ] {
            let json = serde_json::to_string(&field).unwrap();
            let decoded: SortField = serde_json::from_str(&json).unwrap();
            assert_eq!(field, decoded);
        }
    }

    #[test]
    fn test_sort_order_serde_roundtrip() {
        for order in [SortOrder::Asc, SortOrder::Desc] {
            let json = serde_json::to_string(&order).unwrap();
            let decoded: SortOrder = serde_json::from_str(&json).unwrap();
            assert_eq!(order, decoded);
        }
    }

    #[test]
    fn test_sort_field_camelcase_alias_deserialization() {
        let json = r#"{"sortBy":"Rarity","sortOrder":"Desc"}"#;
        let filters: CardSearchFilters = serde_json::from_str(json).unwrap();
        assert_eq!(filters.sort_by, Some(SortField::Rarity));
        assert_eq!(filters.sort_order, Some(SortOrder::Desc));
    }

    #[test]
    fn test_sort_field_snake_case_deserialization() {
        let json = r#"{"sort_by":"CollectorNumber","sort_order":"Asc"}"#;
        let filters: CardSearchFilters = serde_json::from_str(json).unwrap();
        assert_eq!(filters.sort_by, Some(SortField::CollectorNumber));
        assert_eq!(filters.sort_order, Some(SortOrder::Asc));
    }

    #[test]
    fn test_builder_with_sort() {
        let filters = CardSearchFilters::new()
            .with_sort_by(SortField::Artist)
            .with_sort_order(SortOrder::Desc);
        assert_eq!(filters.sort_by, Some(SortField::Artist));
        assert_eq!(filters.sort_order, Some(SortOrder::Desc));
    }

    #[test]
    fn test_default_sort_is_none() {
        let filters = CardSearchFilters::default();
        assert!(filters.sort_by.is_none());
        assert!(filters.sort_order.is_none());
    }

    #[test]
    fn test_builder_all_fields() {
        use crate::{CardColour, Rarity, pokemon::EnergyType, riftbound::CardDomain};

        let filters = CardSearchFilters::new()
            .with_name("Bolt")
            .with_color_identities(vec![CardColour::Red])
            .with_set_code("M13")
            .with_collector_number("42")
            .with_artist("Volkan")
            .with_text("deals 3 damage")
            .with_rarity(Rarity::Rare)
            .with_subtypes(vec!["Instant".to_string()])
            .with_supertypes("Legendary")
            .with_types(vec!["Sorcery".to_string()])
            .with_domains(vec![CardDomain::Fury])
            .with_energy_types(vec![EnergyType::Fire]);

        assert_eq!(filters.name, Some("Bolt".to_string()));
        assert_eq!(filters.color_identities, Some(vec![CardColour::Red]));
        assert_eq!(filters.set_code, Some("M13".to_string()));
        assert_eq!(filters.collector_number, Some("42".to_string()));
        assert_eq!(filters.artist, Some("Volkan".to_string()));
        assert_eq!(filters.text, Some("deals 3 damage".to_string()));
        assert_eq!(filters.rarity, Some(Rarity::Rare));
        assert_eq!(filters.subtypes, Some(vec!["Instant".to_string()]));
        assert_eq!(filters.supertypes, Some("Legendary".to_string()));
        assert_eq!(filters.types, Some(vec!["Sorcery".to_string()]));
        assert_eq!(filters.domains, Some(vec![CardDomain::Fury]));
        assert_eq!(filters.energy_types, Some(vec![EnergyType::Fire]));
    }

    #[test]
    fn test_rarity_deserialization_from_string() {
        let json = r#"{"rarity":"Rare"}"#;
        let f: CardSearchFilters = serde_json::from_str(json).unwrap();
        assert_eq!(f.rarity, Some(Rarity::Rare));
    }

    #[test]
    fn test_rarity_empty_string_becomes_none() {
        let json = r#"{"rarity":""}"#;
        let f: CardSearchFilters = serde_json::from_str(json).unwrap();
        assert!(f.rarity.is_none());
    }

    #[test]
    fn test_rarity_absent_becomes_none() {
        let json = r#"{}"#;
        let f: CardSearchFilters = serde_json::from_str(json).unwrap();
        assert!(f.rarity.is_none());
    }

    #[test]
    fn test_filters_serde_preserves_sort_fields() {
        let original = CardSearchFilters::new()
            .with_name("Lightning Bolt")
            .with_sort_by(SortField::SetCode)
            .with_sort_order(SortOrder::Asc);
        let json = serde_json::to_string(&original).unwrap();
        let decoded: CardSearchFilters = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, Some("Lightning Bolt".to_string()));
        assert_eq!(decoded.sort_by, Some(SortField::SetCode));
        assert_eq!(decoded.sort_order, Some(SortOrder::Asc));
    }
}

fn empty_string_to_none<'de, D>(deserializer: D) -> Result<Option<Rarity>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(match opt.as_deref() {
        Some("") | None => None,
        Some(s) => {
            Some(serde_json::from_str(&format!("\"{}\"", s)).map_err(serde::de::Error::custom)?)
        }
    })
}
