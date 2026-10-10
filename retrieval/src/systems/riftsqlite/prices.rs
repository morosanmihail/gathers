//! Riftbound prices: TCGplayer's, from TCGCSV (see `systems::tcgcsv`). The
//! cards db has no TCGplayer product ids, so the snapshot matches products
//! to card ids itself, from each product's set and collector number.

use crate::systems::tcgcsv::{self, Game, Group, Printings, Product};

/// Default file name of the prices db, and its stem on mirrors.
pub const RIFTBOUND_PRICES_FILE: &str = "riftbound_prices_tcgcsv.sqlite";

pub(crate) const RIFTBOUND: Game = Game {
    name: "Riftbound",
    // TCGplayer's category for Riftbound: League of Legends TCG.
    category: 89,
    printings: Printings {
        normal: &["Normal"],
        foil: &["Foil"],
    },
    card_id: Some(card_id),
};

/// The cards db's id for a TCGplayer product: the set's abbreviation and
/// the product's `Number`, `UNL` + `131/219` → `unl-131-219`. Overnumbered
/// printings are `299*/298` on TCGplayer and `ogn-299-star-298` here.
/// Sealed products have no number, so no card id.
fn card_id(group: &Group, product: &Product) -> Option<String> {
    let set = group.abbreviation.as_deref()?.trim();
    let number = product.extended("Number")?.trim();
    if set.is_empty() || number.is_empty() {
        return None;
    }
    Some(
        format!("{set}-{}", number.replace('*', "-star").replace('/', "-"))
            .to_lowercase(),
    )
}

/// Updates the prices db at `path`: from the first configured mirror that
/// has it, else straight from TCGCSV.
pub async fn download_riftbound_prices(path: &str) -> eyre::Result<()> {
    tcgcsv::download_prices_db(&RIFTBOUND, RIFTBOUND_PRICES_FILE, path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::tcgcsv::ExtendedData;

    fn id(abbreviation: Option<&str>, number: Option<&str>) -> Option<String> {
        let group = Group {
            group_id: 1,
            abbreviation: abbreviation.map(str::to_string),
        };
        let product = Product {
            product_id: 1,
            extended_data: number
                .map(|n| ExtendedData {
                    name: "Number".to_string(),
                    value: n.to_string(),
                })
                .into_iter()
                .collect(),
        };
        card_id(&group, &product)
    }

    #[test]
    fn card_ids_from_set_and_number() {
        assert_eq!(id(Some("UNL"), Some("131/219")).as_deref(), Some("unl-131-219"));
        assert_eq!(id(Some("OGN"), Some("007/298")).as_deref(), Some("ogn-007-298"));
        assert_eq!(id(Some("OGN"), Some("299*/298")).as_deref(), Some("ogn-299-star-298"));
    }

    #[test]
    fn no_card_id_without_set_or_number() {
        assert_eq!(id(Some("UNL"), None), None);
        assert_eq!(id(None, Some("131/219")), None);
        assert_eq!(id(Some("UNL"), Some(" ")), None);
    }
}
