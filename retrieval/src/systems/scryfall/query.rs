use models::filters::CardSearchFilters;

/// Scryfall returns at most this many cards per page of search results.
pub const PAGE_SIZE: usize = 175;

/// Wraps a value in double quotes for Scryfall's query syntax, escaping any
/// literal double-quotes, so values containing spaces or commas (card names,
/// artist names, oracle text snippets) are treated as one term instead of
/// being split into several unscoped search words.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

/// Builds a Scryfall `q=` search-query string from `filters`, mirroring the
/// filter set the SQL retrieval system supports over its own columns.
pub fn build_query_string(filters: &CardSearchFilters) -> String {
    let mut query = vec![];

    if let Some(name) = &filters.name {
        query.push(format!("name:{}", quote(name)));
    }

    if let Some(set_code) = &filters.set_code {
        query.push(format!("set:{set_code}"));
    }

    if let Some(collector_number) = &filters.collector_number {
        query.push(format!("cn:{collector_number}"));
    }

    if let Some(artist) = &filters.artist {
        query.push(format!("a:{}", quote(artist)));
    }

    // Oracle text search uses `o:`; `t:` is the type-line operator and would
    // silently match nothing for prose queries.
    if let Some(text) = &filters.text {
        query.push(format!("o:{}", quote(text)));
    }

    if let Some(rarity) = &filters.rarity {
        query.push(format!("r:{}", rarity.to_single_string()));
    }

    if let Some(types) = &filters.types {
        for t in types {
            query.push(format!("type:{t}"));
        }
    }

    if let Some(subtypes) = &filters.subtypes {
        for s in subtypes {
            query.push(format!("type:{s}"));
        }
    }

    if let Some(supertypes) = &filters.supertypes {
        query.push(format!("type:{supertypes}"));
    }

    // `id:` is colour identity; `c:` is the colours actually printed on the
    // card. Using `c:` for `color_identities` (as this used to) silently
    // narrowed results to cards whose printed colours also happened to match.
    if let Some(color_identities) = &filters.color_identities {
        for color in color_identities {
            query.push(format!("id:{color}"));
        }
    }

    if let Some(colors) = &filters.colors {
        for color in colors {
            query.push(format!("c:{color}"));
        }
    }

    if let Some(min) = filters.mana_value_min {
        query.push(format!("mv>={min}"));
    }

    if let Some(max) = filters.mana_value_max {
        query.push(format!("mv<={max}"));
    }

    if let Some(keywords) = &filters.keywords {
        for k in keywords {
            query.push(format!("kw:{}", quote(k)));
        }
    }

    if let Some(power) = &filters.power {
        query.push(format!("pow:{power}"));
    }

    if let Some(toughness) = &filters.toughness {
        query.push(format!("tou:{toughness}"));
    }

    if let Some(loyalty) = &filters.loyalty {
        query.push(format!("loy:{loyalty}"));
    }

    if let Some(defense) = &filters.defense {
        query.push(format!("defense:{defense}"));
    }

    if let Some(is_reserved) = filters.is_reserved {
        query.push(bool_term("is:reserved", is_reserved));
    }

    if let Some(is_promo) = filters.is_promo {
        query.push(bool_term("is:promo", is_promo));
    }

    if let Some(is_reprint) = filters.is_reprint {
        query.push(bool_term("is:reprint", is_reprint));
    }

    if let Some(is_full_art) = filters.is_full_art {
        query.push(bool_term("is:fullart", is_full_art));
    }

    if let Some(border_color) = &filters.border_color {
        query.push(format!("border:{border_color}"));
    }

    if let Some(legal_in) = &filters.legal_in {
        query.push(format!("f:{legal_in}"));
    }

    query.join(" ")
}

fn bool_term(term: &str, value: bool) -> String {
    if value {
        term.to_string()
    } else {
        format!("-{term}")
    }
}

/// Maps a `skip` offset onto Scryfall's 1-indexed, [`PAGE_SIZE`]-per-page
/// pagination. Scryfall has no arbitrary-offset pagination, so a `skip` that
/// doesn't fall on a page boundary is only approximated (the page it lands
/// in is returned in full).
pub fn scryfall_page(skip: Option<usize>) -> usize {
    skip.map(|s| s / PAGE_SIZE + 1).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{CardColour, Rarity};

    #[test]
    fn empty_filters_produce_empty_query() {
        assert_eq!(build_query_string(&CardSearchFilters::default()), "");
    }

    #[test]
    fn name_is_quoted() {
        let filters = CardSearchFilters::new().with_name("Birgi, God of Storytelling");
        assert_eq!(
            build_query_string(&filters),
            "name:\"Birgi, God of Storytelling\""
        );
    }

    #[test]
    fn text_uses_oracle_operator_not_type() {
        let filters = CardSearchFilters::new().with_text("draw a card");
        assert_eq!(build_query_string(&filters), "o:\"draw a card\"");
    }

    #[test]
    fn color_identities_use_id_not_c() {
        let filters = CardSearchFilters::new().with_color_identities(vec![CardColour::Red]);
        assert_eq!(build_query_string(&filters), "id:R");
    }

    #[test]
    fn colors_use_c() {
        let filters = CardSearchFilters::new().with_colors(vec![CardColour::Blue]);
        assert_eq!(build_query_string(&filters), "c:U");
    }

    #[test]
    fn mana_value_range() {
        let filters = CardSearchFilters::new()
            .with_mana_value_min(2.0)
            .with_mana_value_max(4.0);
        assert_eq!(build_query_string(&filters), "mv>=2 mv<=4");
    }

    #[test]
    fn boolean_flags_negate_when_false() {
        let filters = CardSearchFilters {
            is_reserved: Some(true),
            is_promo: Some(false),
            ..Default::default()
        };
        assert_eq!(build_query_string(&filters), "is:reserved -is:promo");
    }

    #[test]
    fn rarity_uses_single_string() {
        let filters = CardSearchFilters::new().with_rarity(Rarity::Mythic);
        assert_eq!(build_query_string(&filters), "r:mythic");
    }

    #[test]
    fn legal_in_maps_to_format_operator() {
        let filters = CardSearchFilters::new().with_legal_in("modern");
        assert_eq!(build_query_string(&filters), "f:modern");
    }

    #[test]
    fn collector_number_and_artist_and_border() {
        let filters = CardSearchFilters::new()
            .with_collector_number("123")
            .with_artist("Eric Deschamps")
            .with_border_color("borderless");
        assert_eq!(
            build_query_string(&filters),
            "cn:123 a:\"Eric Deschamps\" border:borderless"
        );
    }

    #[test]
    fn keywords_power_toughness_loyalty_defense() {
        let filters = CardSearchFilters::new()
            .with_keywords(vec!["Flying".to_string()])
            .with_power("4")
            .with_toughness("4")
            .with_loyalty("3")
            .with_defense("5");
        assert_eq!(
            build_query_string(&filters),
            "kw:\"Flying\" pow:4 tou:4 loy:3 defense:5"
        );
    }

    #[test]
    fn scryfall_page_maps_skip_to_page_size_boundaries() {
        assert_eq!(scryfall_page(None), 1);
        assert_eq!(scryfall_page(Some(0)), 1);
        assert_eq!(scryfall_page(Some(174)), 1);
        assert_eq!(scryfall_page(Some(175)), 2);
        assert_eq!(scryfall_page(Some(350)), 3);
    }
}
