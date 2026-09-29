use eyre::{Result, WrapErr};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::systems::pokemon::scraper::common::{
    format_exp_number, format_id, normalize_set_name, pull_variants,
};
use crate::systems::pokemon::scraper::models::{Card, Expansion, TcgpCode, TcgpSet};

const TCGP_API: &str = "https://mp-search-api.tcgplayer.com/v1/search/request";
const TCGP_IMAGE: &str = "https://product-images.tcgplayer.com/fit-in/437x437";
const CODES_API: &str = "https://mpapi.tcgplayer.com/v2/massentry/sets/3";

pub async fn get_tcgp_sets(client: &reqwest::Client) -> Result<Vec<TcgpSet>> {
    let body = base_request(1, &[], "Cards");
    let resp: TcgpResponse = client
        .post(TCGP_API)
        .query(&[("q", ""), ("isList", "false")])
        .json(&body)
        .send()
        .await
        .wrap_err("fetching TCGP set list")?
        .json()
        .await
        .wrap_err("parsing TCGP set list")?;

    let aggs = resp
        .results
        .into_iter()
        .next()
        .and_then(|r| r.aggregations)
        .map(|a| a.set_name)
        .unwrap_or_default();

    Ok(aggs
        .into_iter()
        .map(|a| TcgpSet {
            url_val: a.url_value,
            value: a.value,
            count: a.count,
        })
        .collect())
}

pub async fn get_tcgp_codes(client: &reqwest::Client) -> Result<Vec<TcgpCode>> {
    #[derive(Deserialize)]
    struct CodeResp {
        results: Vec<CodeEntry>,
    }
    #[derive(Deserialize)]
    struct CodeEntry {
        name: String,
        code: String,
    }

    let resp: CodeResp = client
        .get(CODES_API)
        .send()
        .await
        .wrap_err("fetching TCGP codes")?
        .json()
        .await
        .wrap_err("parsing TCGP codes")?;

    Ok(resp
        .results
        .into_iter()
        .map(|e| TcgpCode {
            name: e.name,
            code: e.code,
        })
        .collect())
}

/// Lowercase alphanumeric words of a set name, with `&` spelled `and` and
/// accents folded, so "Scarlet & Violet", "scarlet-and-violet" and
/// "SV: Scarlet and Violet" tokenize alike.
fn set_tokens(name: &str) -> Vec<String> {
    name.to_lowercase()
        .replace('&', " and ")
        .replace('é', "e")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// A TCGPlayer series code prefix, like `xy`, `sv`, `me04` or `swsh12`.
fn is_series_code(token: &str) -> bool {
    let letters = token.trim_end_matches(|c: char| c.is_ascii_digit());
    (1..=4).contains(&letters.len()) && letters.chars().all(|c| c.is_ascii_lowercase())
}

/// Suffixes TCGPlayer uses for a sub-listing of a set ("Silver Tempest
/// Trainer Gallery") rather than for a different set ("Team Rocket Returns").
const SUBSET_SUFFIXES: &[&[&str]] = &[
    &["trainer", "gallery"],
    &["galarian", "gallery"],
    &["classic", "collection"],
    &["radiant", "collection"],
    &["shiny", "vault"],
    &["shadowless"],
];

/// Whether TCGPlayer set `tcgp` is set `name` itself (or one of its
/// subsets), judged by word structure rather than similarity: `name`'s words
/// must appear either as the whole of `tcgp` or right after a single series
/// code ("XY Evolutions", "SWSH12: Silver Tempest"), optionally followed by
/// a known subset suffix ("Celebrations Classic Collection"). That rules out
/// a different set that merely shares words — "SV Prismatic Evolutions" is
/// not "Evolutions", "ME 30th Celebration" is not "Celebrations", and
/// "EX Team Rocket Returns" is not "Team Rocket".
fn is_structural_match(name: &str, tcgp: &str) -> bool {
    let name = set_tokens(name);
    let tcgp = set_tokens(tcgp);
    if name.is_empty() {
        return false;
    }
    let matches_at = |start: usize| {
        let Some(rest) = tcgp.get(start..).and_then(|t| t.strip_prefix(name.as_slice())) else {
            return false;
        };
        rest.is_empty() || SUBSET_SUFFIXES.iter().any(|suffix| rest == *suffix)
    };
    matches_at(0) || (tcgp.first().is_some_and(|t| is_series_code(t)) && matches_at(1))
}

/// Whether a card TCGPlayer filed under set `tcgp` belongs to set `name`,
/// by the same rules `find_set_from_tcgp` uses to pick a set's TCGPlayer
/// sets in the first place.
pub fn tcgp_set_belongs_to(name: &str, tcgp: &str) -> bool {
    let candidate = [TcgpSet { url_val: tcgp.to_string(), value: tcgp.to_string(), count: 0 }];
    !find_set_from_tcgp(name, &candidate).is_empty()
}

/// Whether `found` — the top hit of a free-text TCGPlayer search for
/// Serebii's `card` (see `search_card`) — really is that card. The search
/// ranks by text relevance, so the hit can be a card from a different set
/// entirely, or a sealed "Code Card" product; storing such a hit would add a
/// bogus card to the set (it's keyed by collector number, so a mismatched
/// one lands as a new row) with the other set's data and release date.
pub fn is_same_card(card: &Card, found: &Card) -> bool {
    if found.name.contains("Code Card") || found.exp_card_number != card.exp_card_number {
        return false;
    }
    // Numbers coincide across sets, so also require the set or the name to
    // agree. TCGPlayer names often add a suffix ("Pidove - 11/12").
    tcgp_set_belongs_to(&card.exp_name, &found.exp_id_tcgp)
        || found.name.to_lowercase().starts_with(&card.name.to_lowercase())
}

/// The TCGPlayer sets (by `url_val`) holding set `name`'s cards. Promo sets
/// are matched by their base name. Other sets prefer structural matches
/// (see `is_structural_match`); only when there are none does it fall back
/// to fuzzy similarity, since the loose rule alone happily merges unrelated
/// sets that share a word (e.g. "Evolutions" with "Prismatic Evolutions").
pub fn find_set_from_tcgp<'a>(name: &str, tcgp_sets: &'a [TcgpSet]) -> Vec<&'a str> {
    let name_norm = normalize_set_name(name).to_lowercase();
    let name_has_promo = name_norm.contains("promo");

    if !name_has_promo {
        let structural: Vec<&str> = tcgp_sets
            .iter()
            .filter(|ts| is_structural_match(name, &ts.value))
            .map(|ts| ts.url_val.as_str())
            .collect();
        if !structural.is_empty() {
            return structural;
        }
    }

    let mut matches = Vec::new();
    for ts in tcgp_sets {
        let tcgp_norm = normalize_set_name(&ts.value).to_lowercase();
        let tcgp_has_promo = tcgp_norm.contains("promo");

        if name_has_promo != tcgp_has_promo {
            continue;
        }
        if name_has_promo {
            let base = name_norm
                .replace("promos", "")
                .replace("promo", "")
                .replace("cards", "")
                .replace("card", "");
            let base = base.trim();
            if !tcgp_norm.contains(base) {
                continue;
            }
        }

        let conf = strsim::sorensen_dice(&tcgp_norm, &name_norm);
        let tcgp_lower = ts.value.to_lowercase();
        if conf > 0.7 || tcgp_lower.contains(&name_norm) {
            debug!("TCGP match: '{tcgp_norm}' ~ '{name_norm}' ({conf:.2})");
            matches.push(ts.url_val.as_str());
        }
    }
    matches
}

pub fn find_tcgp_code<'a>(tcgp_set_name: &str, codes: &'a [TcgpCode]) -> Option<&'a str> {
    codes
        .iter()
        .find(|c| strsim::sorensen_dice(&c.name, tcgp_set_name) > 0.8)
        .map(|c| c.code.as_str())
}

pub async fn pull_set_cards(
    client: &reqwest::Client,
    exp: &Expansion,
    codes: &[TcgpCode],
) -> Result<Vec<Card>> {
    if exp.tcg_name.is_empty() || exp.tcg_name == "[]" || exp.tcg_name == r#"["N/A"]"# {
        return Ok(vec![]);
    }

    let set_names: Vec<String> = serde_json::from_str(&exp.tcg_name).unwrap_or_default();
    if set_names.is_empty() {
        return Ok(vec![]);
    }

    let mut cards = Vec::new();
    let mut from: i64 = 0;
    let size: i64 = 25;
    let mut total: i64 = 1;

    while from < total {
        let body = {
            let mut b = base_request(size, &set_names, "Cards");
            b["from"] = serde_json::json!(from);
            b
        };

        let resp: TcgpResponse = match client
            .post(TCGP_API)
            .query(&[("q", ""), ("isList", "false")])
            .json(&body)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(e) => {
                    warn!("TCGP JSON parse error for '{}': {e}", exp.name);
                    break;
                }
            },
            Err(e) => {
                warn!("TCGP request error for '{}': {e}", exp.name);
                break;
            }
        };

        if let Some(result_set) = resp.results.into_iter().next() {
            total = result_set.total_results;
            for raw in result_set.results {
                if raw.product_name.contains("Code Card") {
                    continue;
                }
                let card = convert_card(raw, &exp.name, &exp.release_date, codes);
                cards.push(card);
            }
        }

        from += size;
    }

    Ok(cards)
}

pub async fn search_card(
    client: &reqwest::Client,
    card_name: &str,
    set_name: &str,
    codes: &[TcgpCode],
) -> Option<Card> {
    let body = base_request(1, &[], "Cards");
    let query = format!("{set_name} {card_name}");
    let resp: TcgpResponse = client
        .post(TCGP_API)
        .query(&[("q", query.as_str()), ("isList", "false")])
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let raw = resp
        .results
        .into_iter()
        .next()?
        .results
        .into_iter()
        .next()?;

    Some(convert_card(raw, set_name, "", codes))
}

fn base_request(size: i64, set_names: &[String], product_type: &str) -> serde_json::Value {
    serde_json::json!({
        "algorithm": "",
        "from": 0,
        "size": size,
        "filters": {
            "term": {
                "productLineName": ["pokemon"],
                "setName": set_names,
                "productTypeName": [product_type]
            },
            "range": {},
            "match": {}
        },
        "listingSearch": {
            "filters": {
                "term": {},
                "range": { "quantity": { "gte": 1 } },
                "exclude": { "channelExclusion": 0 }
            },
            "context": { "cart": {} }
        },
        "context": {
            "cart": {},
            "shippingCountry": "US"
        },
        "sort": {}
    })
}

fn convert_card(raw: TcgpCardRaw, exp_name: &str, set_release: &str, codes: &[TcgpCode]) -> Card {
    let attrs = raw.custom_attributes.as_ref();
    let release_date = attrs
        .and_then(|a| a.release_date.as_deref())
        .filter(|d| !d.is_empty())
        .map(|d| d.to_string())
        .unwrap_or_else(|| set_release.to_string());

    let raw_num = attrs
        .and_then(|a| a.number.as_deref())
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    let card_num = format_exp_number(raw_num);

    let rarity = raw.rarity_name.clone().unwrap_or_default();
    let variants = pull_variants(&rarity);

    let energy_type = attrs
        .and_then(|a| a.energy_type.as_ref())
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();
    let card_type = attrs
        .and_then(|a| a.card_type.as_ref())
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();

    let code = find_tcgp_code(&raw.set_url_name, codes)
        .unwrap_or("")
        .to_string();

    let mut card_id = format_id(exp_name, &raw.product_name, &card_num);

    if raw.product_name.contains("(Exclusive)") {
        card_id = format!("{card_id}-{}", raw.product_id);
        let num = format!("{card_num}-{}", raw.product_id);
        return Card {
            card_id,
            id_tcgp: raw.product_id,
            name: raw.product_name,
            exp_id_tcgp: raw.set_url_name,
            exp_code_tcgp: code,
            exp_name: exp_name.to_string(),
            exp_card_number: num,
            rarity,
            img: format!("{TCGP_IMAGE}/{}.jpg", raw.product_id),
            price: raw.market_price,
            release_date: Some(release_date),
            energy_type: Some(energy_type),
            card_type: Some(card_type),
            variants,
            ..Default::default()
        };
    }

    Card {
        card_id,
        id_tcgp: raw.product_id,
        name: raw.product_name,
        exp_id_tcgp: raw.set_url_name,
        exp_code_tcgp: code,
        exp_name: exp_name.to_string(),
        exp_card_number: card_num,
        rarity,
        img: format!("{TCGP_IMAGE}/{}.jpg", raw.product_id),
        price: raw.market_price,
        release_date: Some(release_date),
        energy_type: Some(energy_type),
        card_type: Some(card_type),
        variants,
        ..Default::default()
    }
}

#[derive(Deserialize)]
struct TcgpResponse {
    results: Vec<TcgpResultSet>,
}

#[derive(Deserialize)]
struct TcgpResultSet {
    #[serde(rename = "totalResults")]
    total_results: i64,
    results: Vec<TcgpCardRaw>,
    aggregations: Option<TcgpAggregations>,
}

#[derive(Deserialize)]
struct TcgpAggregations {
    #[serde(rename = "setName")]
    set_name: Vec<TcgpSetAgg>,
}

#[derive(Deserialize)]
struct TcgpSetAgg {
    #[serde(rename = "urlValue")]
    url_value: String,
    value: String,
    count: i64,
}

#[derive(Deserialize)]
struct TcgpCardRaw {
    #[serde(rename = "productId")]
    product_id: i64,
    #[serde(rename = "productName")]
    product_name: String,
    #[serde(rename = "rarityName")]
    rarity_name: Option<String>,
    #[serde(rename = "setUrlName")]
    set_url_name: String,
    #[serde(rename = "marketPrice")]
    market_price: Option<f64>,
    #[serde(rename = "customAttributes")]
    custom_attributes: Option<TcgpCustomAttrs>,
}

#[derive(Deserialize)]
struct TcgpCustomAttrs {
    number: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "energyType")]
    energy_type: Option<Vec<String>>,
    #[serde(rename = "cardType")]
    card_type: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets(names: &[&str]) -> Vec<TcgpSet> {
        names
            .iter()
            .map(|n| TcgpSet { url_val: n.to_string(), value: n.to_string(), count: 1 })
            .collect()
    }

    // Set names as TCGPlayer lists them, incl. the ones that used to be
    // merged into the wrong expansion.
    const TCGP: &[&str] = &[
        "Celebrations",
        "Celebrations: Classic Collection",
        "ME: 30th Celebration",
        "XY - Evolutions",
        "SV: Prismatic Evolutions",
        "SWSH12: Silver Tempest",
        "SWSH12: Silver Tempest Trainer Gallery",
        "SV: Black Bolt",
        "Black and White Promos",
        "ME04: Chaos Rising",
        "SM - Guardians Rising",
        "Base Set",
        "Base Set (Shadowless)",
        "EX Delta Species",
        "Team Rocket",
        "EX Team Rocket Returns",
        "SV: Scarlet & Violet Promo Cards",
        "SV: Scarlet & Violet 151",
    ];

    fn found(name: &str) -> Vec<String> {
        let tcgp = sets(TCGP);
        let mut v: Vec<String> = find_set_from_tcgp(name, &tcgp).into_iter().map(String::from).collect();
        v.sort();
        v
    }

    #[test]
    fn same_word_different_set_is_not_merged() {
        assert_eq!(found("Celebrations"), ["Celebrations", "Celebrations: Classic Collection"]);
        assert_eq!(found("Evolutions"), ["XY - Evolutions"]);
        assert_eq!(found("Prismatic Evolutions"), ["SV: Prismatic Evolutions"]);
        assert_eq!(found("Chaos Rising"), ["ME04: Chaos Rising"]);
        assert_eq!(found("Team Rocket"), ["Team Rocket"]);
        assert_eq!(found("Team Rocket Returns"), ["EX Team Rocket Returns"]);
    }

    #[test]
    fn series_prefixes_and_subsets_still_match() {
        assert_eq!(found("Silver Tempest"), ["SWSH12: Silver Tempest", "SWSH12: Silver Tempest Trainer Gallery"]);
        assert_eq!(found("Black Bolt"), ["SV: Black Bolt"]);
        assert_eq!(found("Base Set"), ["Base Set", "Base Set (Shadowless)"]);
    }

    fn card(name: &str, number: &str, exp_name: &str, tcgp_set: &str) -> Card {
        Card {
            name: name.to_string(),
            exp_card_number: number.to_string(),
            exp_name: exp_name.to_string(),
            exp_id_tcgp: tcgp_set.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn free_text_hit_must_be_the_same_card() {
        let serebii = card("Reshiram", "036", "White Flare", "");
        // Same name, other set, other number: was inserted as a new card.
        assert!(!is_same_card(&serebii, &card("Reshiram EX - BW36", "BW036", "White Flare", "Black and White Promos")));
        assert!(is_same_card(&serebii, &card("Reshiram", "036", "White Flare", "SV White Flare")));

        let serebii = card("Mega Diancie ex", "", "Phantasmal Flames", "");
        assert!(!is_same_card(&serebii, &card("Code Card - Mega Diancie EX Premium Collection", "", "Phantasmal Flames", "XY Phantom Forces")));

        // Promo sets TCGPlayer names differently are still enriched when the
        // number and name agree.
        let serebii = card("Pidove", "011", "McDonald's Collection (2011)", "");
        assert!(is_same_card(&serebii, &card("Pidove - 11/12", "011", "McDonald's Collection (2011)", "McDonalds Promos 2011")));

        // Matching number alone isn't enough.
        let serebii = card("Bulbasaur", "013", "Delta Reign", "");
        assert!(!is_same_card(&serebii, &card("Rayquaza (Delta Species)", "013", "Delta Reign", "EX Delta Species")));
    }

    #[test]
    fn card_search_hits_from_other_sets_are_rejected() {
        assert!(tcgp_set_belongs_to("Chaos Rising", "ME04 Chaos Rising"));
        assert!(tcgp_set_belongs_to("Lost Origin", "swsh11-lost-origin-trainer-gallery"));
        assert!(!tcgp_set_belongs_to("Chaos Rising", "SM Guardians Rising"));
        assert!(!tcgp_set_belongs_to("Delta Reign", "EX Delta Species"));
        assert!(!tcgp_set_belongs_to("Black Bolt", "Black and White Promos"));
        assert!(!tcgp_set_belongs_to("Phantasmal Flames", "XY Phantom Forces"));
        assert!(!tcgp_set_belongs_to("Scarlet & Violet Promos", "SV Scarlet and Violet 151"));
        assert!(tcgp_set_belongs_to("Scarlet & Violet Promos", "SV: Scarlet & Violet Promo Cards"));
    }
}
