use eyre::{Result, WrapErr};
use scraper::{ElementRef, Html, Selector};

use crate::systems::pokemon::scraper::common::{format_exp_number, format_id};
use crate::systems::pokemon::scraper::models::{Card, Pokemon, SerebiiSet};

const BASE: &str = "https://www.serebii.net";

pub async fn scrape_normal_sets(
    client: &reqwest::Client,
    count: Option<usize>,
) -> Result<Vec<SerebiiSet>> {
    let url = format!("{BASE}/card/english.shtml");
    scrape_set_list(client, &url, count, false).await
}

pub async fn scrape_promo_sets(
    client: &reqwest::Client,
    count: Option<usize>,
) -> Result<Vec<SerebiiSet>> {
    let url = format!("{BASE}/card/engpromo.shtml");
    scrape_set_list(client, &url, count, true).await
}

pub async fn scrape_set_details(
    client: &reqwest::Client,
    set_url: &str,
    exp_name: &str,
) -> Result<(String, Vec<Card>)> {
    let html = client
        .get(set_url)
        .send()
        .await
        .wrap_err_with(|| format!("fetching set page {set_url}"))?
        .text()
        .await?;
    let document = Html::parse_document(&html);
    let logo = extract_logo(&document);
    let cards = extract_cards(&document, exp_name);
    Ok((logo, cards))
}

pub async fn scrape_pokedex(client: &reqwest::Client) -> Result<Vec<Pokemon>> {
    let url = format!("{BASE}/pokemon/nationalpokedex.shtml");
    let html = client
        .get(&url)
        .send()
        .await
        .wrap_err("fetching Serebii Pokedex")?
        .text()
        .await?;
    let document = Html::parse_document(&html);

    let table_sel = Selector::parse(".dextable").unwrap();
    let tr_sel = Selector::parse("tr").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    let table = match document.select(&table_sel).next() {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    let mut pokemon = Vec::new();
    for row in table.select(&tr_sel).skip(2) {
        let cells: Vec<_> = row.select(&td_sel).collect();
        if cells.len() < 3 {
            continue;
        }
        let num_str = cells[0].text().collect::<String>().replace('#', "");
        let id = match num_str.trim().parse::<i64>() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let name = cells[2].text().collect::<String>().trim().to_string();
        if !name.is_empty() {
            pokemon.push(Pokemon { id, name });
        }
    }
    Ok(pokemon)
}

async fn scrape_set_list(
    client: &reqwest::Client,
    url: &str,
    count: Option<usize>,
    is_promo: bool,
) -> Result<Vec<SerebiiSet>> {
    let html = client
        .get(url)
        .send()
        .await
        .wrap_err_with(|| format!("fetching Serebii set list {url}"))?
        .text()
        .await?;

    // Extract all data synchronously into owned Strings so all !Send scraper/tendril
    // types are dropped before the first .await below.
    let raw: Vec<(String, String, String, i64)> = {
        let document = Html::parse_document(&html);

        let table_sel = Selector::parse("table").unwrap();
        let tr_sel = Selector::parse("tr").unwrap();
        let td_sel = Selector::parse("td").unwrap();
        let a_sel = Selector::parse("a").unwrap();
        let img_sel = Selector::parse("img").unwrap();

        let table = match document.select(&table_sel).next() {
            Some(t) => t,
            None => return Ok(vec![]),
        };
        let all_rows: Vec<_> = table.select(&tr_sel).collect();
        let data_rows = all_rows.iter().skip(1);
        let data_rows: Box<dyn Iterator<Item = _>> = match count {
            Some(n) => Box::new(data_rows.take(n)),
            None => Box::new(data_rows),
        };

        let mut out = Vec::new();
        for row in data_rows {
            let cells: Vec<_> = row.select(&td_sel).collect();

            let entry: Option<(String, String, String, i64)> = if is_promo {
                if cells.len() < 3 {
                    continue;
                }
                let anchor = cells[0].select(&a_sel).next();
                let sym_img = cells[2].select(&img_sel).next();
                match (anchor, sym_img) {
                    (Some(a), Some(i)) => {
                        let href = a.value().attr("href").unwrap_or("");
                        let name = a.text().collect::<String>().trim().to_string();
                        let page = make_abs(href);
                        let sym = make_abs(i.value().attr("src").unwrap_or(""));
                        let count_str = cells[1].text().collect::<String>();
                        let n = count_str.trim().parse::<i64>().unwrap_or(0);
                        Some((name, page, sym, n))
                    }
                    _ => None,
                }
            } else {
                if cells.len() < 4 {
                    continue;
                }
                let anchor = cells[2].select(&a_sel).next();
                let sym_img = cells[0].select(&img_sel).next();
                match (anchor, sym_img) {
                    (Some(a), Some(i)) => {
                        let href = a.value().attr("href").unwrap_or("");
                        let name = a.text().collect::<String>().trim().to_string();
                        let page = make_abs(href);
                        let sym = make_abs(i.value().attr("src").unwrap_or(""));
                        let count_str = cells[3].text().collect::<String>();
                        let n = count_str.trim().parse::<i64>().unwrap_or(0);
                        Some((name, page, sym, n))
                    }
                    _ => None,
                }
            };

            if let Some(e) = entry {
                if !e.0.is_empty() {
                    out.push(e);
                }
            }
        }
        out
        // document, table, all_rows, data_rows all dropped here
    };

    // Only Send types remain — safe to .await
    let mut sets = Vec::new();
    for (name, page_url, symbol_url, num_cards) in raw {
        let logo = fetch_logo(client, &page_url).await.unwrap_or_default();
        sets.push(SerebiiSet {
            name,
            page: page_url,
            logo,
            symbol: symbol_url,
            number_of_cards: num_cards,
        });
    }

    Ok(sets)
}

async fn fetch_logo(client: &reqwest::Client, set_url: &str) -> Result<String> {
    let html = client.get(set_url).send().await?.text().await?;
    let document = Html::parse_document(&html);
    Ok(extract_logo(&document))
}

fn extract_logo(document: &Html) -> String {
    let sel = Selector::parse("main img").unwrap();
    document
        .select(&sel)
        .next()
        .and_then(|img| img.value().attr("src"))
        .map(make_abs)
        .unwrap_or_default()
}

fn extract_cards(document: &Html, exp_name: &str) -> Vec<Card> {
    let table_sel = Selector::parse(".dextable").unwrap();
    let a_sel = Selector::parse("a").unwrap();
    let img_sel = Selector::parse("img").unwrap();

    let table = match document.select(&table_sel).next() {
        Some(t) => t,
        None => return vec![],
    };

    let mut cards = Vec::new();

    for row in own_rows(table).into_iter().skip(1) {
        let cells = own_cells(row);
        if cells.len() != 4 {
            continue;
        }

        let mut raw_num = cells[0].text().collect::<String>();
        if let Some(a) = cells[0].select(&a_sel).next() {
            let set_text = a.text().collect::<String>();
            raw_num = raw_num.replace(&set_text, "");
        }
        let card_num = format_exp_number(raw_num.trim());

        let rarity = cells[0]
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(parse_rarity)
            .unwrap_or_else(|| "Common".to_string());

        let img_url = cells[1]
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(|src| make_abs(src).replace("/th", ""))
            .unwrap_or_default();

        let name = cells[2].text().collect::<String>().trim().to_string();
        if name.is_empty() {
            continue;
        }

        let energy = cells[3]
            .select(&img_sel)
            .next()
            .and_then(|img| img.value().attr("src"))
            .map(parse_energy)
            .unwrap_or_default();

        let mut card_id = format_id(exp_name, &name, &card_num);
        card_id = card_id.replace(' ', "-");

        cards.push(Card {
            card_id,
            id_tcgp: 0,
            name,
            exp_code_tcgp: String::new(),
            exp_id_tcgp: String::new(),
            exp_name: exp_name.to_string(),
            exp_card_number: card_num,
            rarity,
            img: img_url,
            energy_type: Some(energy),
            ..Default::default()
        });
    }

    cards
}

/// The rows of `table` itself, leaving out rows of tables nested in its
/// cells. A Pokémon card's "Card Details" cell holds a table of its own
/// (HP, weakness, retreat cost).
fn own_rows(table: ElementRef<'_>) -> Vec<ElementRef<'_>> {
    let tr_sel = Selector::parse("tr").unwrap();
    table
        .select(&tr_sel)
        .filter(|tr| {
            tr.ancestors()
                .filter_map(ElementRef::wrap)
                .find(|e| e.value().name() == "table")
                .is_some_and(|t| t.id() == table.id())
        })
        .collect()
}

/// The `td` cells directly in `row`. `row.select("td")` would also return
/// the cells of a table nested in one of them, so a Pokémon row would no
/// longer have 4 cells and would be skipped, leaving only Trainer and
/// Energy cards.
fn own_cells(row: ElementRef<'_>) -> Vec<ElementRef<'_>> {
    row.children()
        .filter_map(ElementRef::wrap)
        .filter(|e| e.value().name() == "td")
        .collect()
}

fn make_abs(src: &str) -> String {
    if src.starts_with("http") {
        src.to_string()
    } else {
        format!("{BASE}{src}")
    }
}

fn parse_rarity(src: &str) -> String {
    if src.contains("common") {
        "Common"
    } else if src.contains("uncommon") {
        "Uncommon"
    } else if src.contains("holographic") {
        "Holo Rare"
    } else if src.contains("lvxrar") {
        "Ultra Rare"
    } else {
        "Common"
    }
    .to_string()
}

fn parse_energy(src: &str) -> String {
    if src.contains("grass") {
        "Grass"
    } else if src.contains("fire") {
        "Fire"
    } else if src.contains("water") {
        "Water"
    } else if src.contains("electric") {
        "Lightning"
    } else if src.contains("psychic") {
        "Psychic"
    } else if src.contains("fighting") {
        "Fighting"
    } else if src.contains("darkness") {
        "Darkness"
    } else if src.contains("metal") {
        "Metal"
    } else if src.contains("dragon") {
        "Dragon"
    } else if src.contains("colorless") {
        "Colorless"
    } else {
        ""
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two rows from serebii.net/card/megapromos: a Pokémon (whose details
    // cell holds a nested table) and a Stadium.
    const SET_PAGE: &str = r#"<html><body><main><table class="dextable">
  <tr>
	<td class="fooevo">Set Number</td>
	<td width="142" class="fooevo">Picture</td>
	<td class="fooevo" width="20%">Card Name</td>
	<td class="fooevo" width="40%">Card Details</td>
	<tr>
		<td class="cen"><a href="/card/megapromos/">Mega Promos</a><br />39		<img src="/card/image/promo.png" loading="lazy" /></a></td>
		<td width="142" class="cen"><a href="/card/megapromos/039.shtml"><img src="/card/th/megapromos/39.jpg" loading="lazy" border="0" /></a></td>
		<td class="cen" width="20%"><a href="/card/megapromos/039.shtml"> <font size="2">Squirtle</font> </a></td>
		<td align="right" class="fooinfo" width="300"><table border="0" cellspacing="0" cellpadding="0" width="300" align="center"><tr><td align="right" colspan="3"><b>80HP</b> <img border="0" src="/card/image/water.png" loading="lazy" width="20" height="20"></td></tr>
		<tr> <td width="33%" align="center"><b>Weakness</b></td><td width="33%" align="center"><b>Resistance</b></td><td width="33%" align="center"><b>Retreat Cost</b></td></tr>
		<td width="33%" align="center"><img border="0" src="/card/image/electric.png" loading="lazy" width="20" height="20">x2</td></td><td width="33%" align="center">&nbsp;</td><td width="33%" align="center">		   <img border="0" src="/card/image/colorless.png" loading="lazy" align="absmiddle" width="20" height="20"><img border="0" src="/card/image/colorless.png" loading="lazy" align="absmiddle" width="20" height="20"></td></tr></table></td>
	<tr>
		<td class="cen"><a href="/card/megapromos/">Mega Promos</a><br />92 / 200 <img src="/card/image/promo.png" loading="lazy" /></a></td>
		<td width="142" class="cen"><a href="/card/megapromos/092.shtml"><img src="/card/th/megapromos/92.jpg" loading="lazy" border="0" /></a></td>
		<td class="cen" width="20%"><a href="/card/megapromos/092.shtml">Paradise Resort</a></td>
		<td align="right" class="fooinfo" width="300"><div align="right">Stadium</div><br /><br /><font size="1"><i>This card stays in play when you play it.</i></font></td>
 </tr>
</table></main></body></html>"#;

    #[test]
    fn pokemon_rows_with_nested_details_table_are_parsed() {
        let cards = extract_cards(&Html::parse_document(SET_PAGE), "Mega Promos");
        let got: Vec<_> = cards
            .iter()
            .map(|c| (c.exp_card_number.as_str(), c.name.as_str(), c.energy_type.as_deref()))
            .collect();
        assert_eq!(
            got,
            [("039", "Squirtle", Some("Water")), ("092", "Paradise Resort", Some(""))]
        );
        assert_eq!(cards[0].card_id, "Mega-Promos-Squirtle-039");
        assert_eq!(cards[0].img, "https://www.serebii.net/card/megapromos/39.jpg");
    }
}
