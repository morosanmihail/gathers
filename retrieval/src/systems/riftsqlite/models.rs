use models::{
    Artist,
    riftbound::{CardDomain, RiftboundCard},
};

#[derive(Debug, PartialEq, Clone)]
pub struct SqlCard {
    pub id: String,
    pub name: String,
    pub set_code: String,
    pub rarity: String,
    pub artists: String,
    pub domains: String,
    pub text: String,
    pub image: String,
    pub collector_number: String,
}

impl SqlCard {
    /// Reads a text column that may be NULL as empty. Runes and tokens have no rules text,
    /// and a row that fails to parse over a missing field is silently dropped from every
    /// search.
    fn text(row: &rusqlite::Row, index: usize) -> rusqlite::Result<String> {
        Ok(row.get::<_, Option<String>>(index)?.unwrap_or_default())
    }

    /// Only the id and name are required: without them there is nothing to show or to
    /// refer to. Everything else falls back to empty.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(SqlCard {
            id: row.get(0)?,
            name: row.get(1)?,
            set_code: Self::text(row, 2)?,
            domains: Self::text(row, 5)?,
            text: Self::text(row, 6)?,
            rarity: Self::text(row, 3)?,
            artists: Self::text(row, 4)?,
            image: Self::text(row, 7)?,
            collector_number: Self::text(row, 8)?,
        })
    }
}

impl From<SqlCard> for RiftboundCard {
    fn from(value: SqlCard) -> Self {
        let domains: Vec<CardDomain> = value
            .domains
            .split(",")
            .filter_map(|c| match c {
                "calm" | "Calm" => Some(CardDomain::Calm),
                "fury" | "Fury" => Some(CardDomain::Fury),
                "chaos" | "Chaos" => Some(CardDomain::Chaos),
                "order" | "Order" => Some(CardDomain::Order),
                "mind" | "Mind" => Some(CardDomain::Mind),
                "body" | "Body" => Some(CardDomain::Body),
                _ => None,
            })
            .collect();
        let domains = if domains.is_empty() {
            vec![CardDomain::Colorless]
        } else {
            domains
        };
        let artists: Vec<Artist> = value
            .artists
            .split(",")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        RiftboundCard {
            id: value.id,
            name: value.name,
            set_code: value.set_code,
            rarity: value.rarity.into(),
            artists,
            domains,
            text: value.text,
            image: value.image,
            collector_number: value.collector_number,
        }
    }
}
