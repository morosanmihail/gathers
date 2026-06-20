use std::sync::Mutex;

use eyre::{Result, WrapErr};
use rusqlite::{Connection, params};
use tracing::{debug, warn};

use crate::systems::pokemon::scraper::models::Card;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS cards (
    cardId        TEXT UNIQUE,
    idTCGP        INTEGER,
    name          TEXT,
    expIdTCGP     TEXT,
    expName       TEXT,
    expCardNumber TEXT,
    expCodeTCGP   TEXT,
    rarity        TEXT,
    img           TEXT,
    price         FLOAT,
    description   TEXT,
    releaseDate   TEXT,
    energyType    TEXT,
    cardType      TEXT,
    pokedex       INTEGER,
    variants      TEXT
);
";

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .wrap_err_with(|| format!("creating directory {}", parent.display()))?;
            }
        }
        let conn =
            Connection::open(path).wrap_err_with(|| format!("opening database at {path}"))?;
        conn.execute_batch(SCHEMA).wrap_err("creating schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn find_card(&self, exp_name: &str, card_number: &str) -> Option<Card> {
        find_card_with(&self.conn.lock().unwrap(), exp_name, card_number)
    }

    /// Whether any card for `exp_name` is already saved. Used to skip
    /// re-scraping a whole set once it has any data, on the assumption
    /// that a set already in the db (even if stale) doesn't need
    /// redownloading.
    pub fn has_set(&self, exp_name: &str) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE expName = ?1)",
            params![exp_name],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
    }

    pub fn upsert_card(&self, card: &Card, update_fields: UpdateFields) {
        let variants_json = serde_json::to_string(&card.variants).unwrap_or_default();

        let conn = self.conn.lock().unwrap();
        if find_card_with(&conn, &card.exp_name, &card.exp_card_number).is_none() {
            debug!("Adding card: {}", card.card_id);
            if let Err(e) = conn.execute(
                "INSERT OR IGNORE INTO cards
                 (cardId, idTCGP, name, expIdTCGP, expCodeTCGP, expName, expCardNumber,
                  rarity, img, price, description, releaseDate, energyType, cardType,
                  pokedex, variants)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
                params![
                    card.card_id,
                    card.id_tcgp,
                    card.name,
                    card.exp_id_tcgp,
                    card.exp_code_tcgp,
                    card.exp_name,
                    card.exp_card_number,
                    card.rarity,
                    card.img,
                    card.price,
                    card.description,
                    card.release_date,
                    card.energy_type,
                    card.card_type,
                    card.pokedex,
                    variants_json,
                ],
            ) {
                warn!("Failed to insert card '{}': {e}", card.card_id);
            }
        } else {
            match update_fields {
                UpdateFields::Serebii => {
                    if let Err(e) = conn.execute(
                        "UPDATE cards SET cardId=?1, img=?2
                         WHERE expCardNumber=?3 AND expName=?4",
                        params![card.card_id, card.img, card.exp_card_number, card.exp_name],
                    ) {
                        warn!("Failed to update card (serebii) '{}': {e}", card.card_id);
                    }
                }
                UpdateFields::Tcgp => {
                    if let Err(e) = conn.execute(
                        "UPDATE cards SET idTCGP=?1, expIdTCGP=?2, rarity=?3, cardType=?4,
                         expCodeTCGP=?5, releaseDate=?6, description=?7, variants=?8
                         WHERE expCardNumber=?9 AND expName=?10",
                        params![
                            card.id_tcgp,
                            card.exp_id_tcgp,
                            card.rarity,
                            card.card_type,
                            card.exp_code_tcgp,
                            card.release_date,
                            card.description,
                            variants_json,
                            card.exp_card_number,
                            card.exp_name,
                        ],
                    ) {
                        warn!("Failed to update card (tcgp) '{}': {e}", card.card_id);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum UpdateFields {
    Serebii,
    Tcgp,
}

fn find_card_with(conn: &Connection, exp_name: &str, card_number: &str) -> Option<Card> {
    let denorm = card_number.replace('0', "");
    conn.query_row(
        "SELECT cardId, idTCGP, name, expIdTCGP, expCodeTCGP, expName, expCardNumber,
                rarity, img, price, description, releaseDate, energyType, cardType,
                pokedex, variants
         FROM cards
         WHERE expName = ?1 AND (expCardNumber = ?2 OR expCardNumber = ?3)",
        params![exp_name, card_number, denorm],
        row_to_card,
    )
    .optional_ext()
}

fn row_to_card(r: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
    let variants_str: Option<String> = r.get(15)?;
    let variants = variants_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Ok(Card {
        card_id: r.get(0)?,
        id_tcgp: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
        name: r.get(2)?,
        exp_id_tcgp: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        exp_code_tcgp: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        exp_name: r.get(5)?,
        exp_card_number: r.get(6)?,
        rarity: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
        img: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
        price: r.get(9)?,
        description: r.get(10)?,
        release_date: r.get(11)?,
        energy_type: r.get(12)?,
        card_type: r.get(13)?,
        pokedex: r.get(14)?,
        variants,
    })
}

trait OptionalExt<T> {
    fn optional_ext(self) -> Option<T>;
}
impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional_ext(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(card_id: &str, exp_name: &str, exp_card_number: &str) -> Card {
        Card {
            card_id: card_id.to_string(),
            exp_name: exp_name.to_string(),
            exp_card_number: exp_card_number.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_has_set_false_on_empty_db() {
        let db = Db::open(":memory:").unwrap();
        assert!(!db.has_set("Scarlet & Violet"));
    }

    #[test]
    fn test_has_set_true_after_upsert() {
        let db = Db::open(":memory:").unwrap();
        db.upsert_card(
            &card("c1", "Scarlet & Violet", "001"),
            UpdateFields::Serebii,
        );

        assert!(db.has_set("Scarlet & Violet"));
        assert!(!db.has_set("Some Other Set"));
    }
}
