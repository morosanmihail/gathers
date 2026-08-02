use models::pokemon::{EnergyType, PokemonCard, PokemonRarity};

#[derive(Debug, PartialEq, Clone)]
pub struct SqlPokemonCard {
    pub id: String,
    pub name: String,
    pub set_code: String,
    pub rarity: String,
    pub energy_type: String,
    pub card_type: String,
    pub image: String,
    pub collector_number: String,
    pub pokedex: Option<i64>,
    pub description: Option<String>,
    pub release_date: Option<String>,
}

impl SqlPokemonCard {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(SqlPokemonCard {
            id: row.get(0)?,
            name: row.get(1)?,
            set_code: row.get(2)?,
            rarity: row.get(3)?,
            energy_type: row.get(4)?,
            card_type: row.get(5)?,
            image: row.get(6)?,
            collector_number: row.get(7)?,
            pokedex: row.get(8).ok(),
            description: row.get(9).ok(),
            release_date: row.get(10).ok(),
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

        PokemonCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
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
        }
    }
}
