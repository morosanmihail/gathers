//! Pokémon prices: TCGplayer's, from TCGCSV (see `systems::tcgcsv`). The
//! cards db stores each card's TCGplayer product id (`idTCGP`), so the
//! snapshot needs no card matching of its own.

use crate::systems::tcgcsv::{self, Game, Printings};

/// Default file name of the prices db, and its stem on mirrors. Distinct
/// from pokedata's `pokemon_prices.sqlite`, so a mirror or install still
/// holding that older format is never mistaken for this one.
pub const POKEMON_PRICES_FILE: &str = "pokemon_prices_tcgcsv.sqlite";

pub(crate) const POKEMON: Game = Game {
    name: "Pokemon",
    // TCGplayer's category for (English) Pokémon.
    category: 3,
    printings: Printings {
        // A card printed only in holofoil has `["Holofoil"]` as its
        // finishes, so holofoil has to be a candidate normal price too.
        normal: &[
            "Normal",
            "Unlimited",
            "1st Edition",
            "Holofoil",
            "Unlimited Holofoil",
            "1st Edition Holofoil",
            "Reverse Holofoil",
        ],
        foil: &["Reverse Holofoil", "Holofoil", "Unlimited Holofoil", "1st Edition Holofoil"],
    },
    card_id: None,
};

/// Updates the prices db at `path`: from the first configured mirror that
/// has it, else straight from TCGCSV.
pub async fn download_pokemon_prices(path: &str) -> eyre::Result<()> {
    tcgcsv::download_prices_db(&POKEMON, POKEMON_PRICES_FILE, path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::tcgcsv::tests::row;

    fn prices(rows: &[(&str, f64)]) -> (Option<f64>, Option<f64>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prices.sqlite");
        tcgcsv::write_prices_db(
            &path,
            "2026-10-09T20:05:19+00:00",
            rows.iter().map(|(sub_type, price)| row(1, sub_type, Some(*price), None)),
            [],
        )
        .unwrap();
        let conn = tcgcsv::open_prices_db(path.to_str().unwrap()).unwrap().unwrap();
        let got = tcgcsv::prices_for_products(&conn, &[1], &POKEMON.printings).unwrap();
        got.get(&1).map(|r| (r.normal, r.foil)).unwrap_or_default()
    }

    #[test]
    fn normal_and_reverse_holo() {
        assert_eq!(prices(&[("Normal", 1.0), ("Reverse Holofoil", 2.0)]), (Some(1.0), Some(2.0)));
    }

    #[test]
    fn holofoil_only_is_the_normal_price() {
        assert_eq!(prices(&[("Holofoil", 5.0)]), (Some(5.0), None));
        assert_eq!(prices(&[("Holofoil", 5.0), ("Reverse Holofoil", 3.0)]), (Some(5.0), Some(3.0)));
    }

    #[test]
    fn first_edition_and_unlimited() {
        assert_eq!(
            prices(&[("1st Edition Holofoil", 90.0), ("Unlimited Holofoil", 30.0)]),
            (Some(30.0), Some(90.0))
        );
        assert_eq!(prices(&[("1st Edition", 4.0), ("Unlimited", 1.0)]), (Some(1.0), None));
    }
}
