use models::pokemon::{EnergyType, PokemonCard, PokemonRarity};

#[derive(Debug, PartialEq, Clone)]
pub struct SqlPokemonCard {
    pub id: String,
    pub name: String,
    pub set_code: String,
    pub set_short_code: Option<String>,
    pub rarity: String,
    pub energy_type: String,
    pub card_type: String,
    pub image: String,
    pub collector_number: String,
    pub pokedex: Option<i64>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    /// Raw JSON array from the `variants` column, e.g. `["Normal","Reverse Holofoil"]`.
    pub variants: Option<String>,
}

impl SqlPokemonCard {
    /// Reads a text column that the scraper may leave NULL as empty. Recent sets, for one,
    /// arrive without a card type, and a row that fails to parse over a missing field is
    /// silently dropped from every search.
    fn text(row: &rusqlite::Row, index: usize) -> rusqlite::Result<String> {
        Ok(row.get::<_, Option<String>>(index)?.unwrap_or_default())
    }

    /// Only the id and name are required: without them there is nothing to show or to
    /// refer to. Everything else falls back to empty.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(SqlPokemonCard {
            id: row.get(0)?,
            name: row.get(1)?,
            set_code: Self::text(row, 2)?,
            rarity: Self::text(row, 3)?,
            energy_type: Self::text(row, 4)?,
            card_type: Self::text(row, 5)?,
            image: Self::text(row, 6)?,
            collector_number: Self::text(row, 7)?,
            pokedex: row.get(8).ok(),
            description: row.get(9).ok(),
            release_date: row.get(10).ok(),
            set_short_code: row.get(11).ok(),
            variants: row.get(12).ok(),
        })
    }
}

impl From<SqlPokemonCard> for PokemonCard {
    fn from(value: SqlPokemonCard) -> Self {
        let energy_types: Vec<EnergyType> = value
            .energy_type
            .split(',')
            .map(|s| EnergyType::from(s.to_string()))
            .collect();
        let finishes: Vec<String> = value
            .variants
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        PokemonCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            set_short_code: value.set_short_code.filter(|c| !c.is_empty()),
            rarity: PokemonRarity::from(value.rarity),
            energy_types,
            card_type: value.card_type,
            image: value.image,
            collector_number: value.collector_number,
            pokedex: match value.pokedex {
                Some(p) if p < 100000 => Some(p),
                _ => None,
            },
            description: value.description.filter(|d| !d.is_empty()),
            release_date: value.release_date.filter(|d| !d.is_empty()),
            finishes,
        }
    }
}
