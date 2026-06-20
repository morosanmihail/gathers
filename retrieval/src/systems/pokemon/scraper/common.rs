use regex::Regex;

/// Normalise a card number string to a canonical padded form.
///
/// Rules (matches TypeScript `formatExpNumber`):
/// - Strip "SVP" prefix
/// - `TG` prefix  → `TG{nn}`   (2-digit)
/// - letter prefix → `{ALPHA}{nnn}` (3-digit)
/// - plain number  → `{nnn}`   (3-digit)
pub fn format_exp_number(number: &str) -> String {
    let number = number.replace("SVP", "");
    let re = Regex::new(r"(TG)?([A-Z]+)?([0-9]+)\s?(/)?\s?([0-9]+)?([A-Z]+)?").unwrap();
    if let Some(caps) = re.captures(&number) {
        let tg = caps.get(1).map(|m| m.as_str());
        let alpha = caps.get(2).map(|m| m.as_str());
        let num = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if tg.is_some() {
            return format!("TG{:0>2}", num);
        }
        if let Some(a) = alpha {
            return format!("{}{:0>3}", a, num);
        }
        return format!("{:0>3}", num);
    }
    String::new()
}

/// Build the canonical card ID: `{set}-{name}-{number}`.
pub fn format_id(set: &str, name: &str, number: &str) -> String {
    let set = set.trim().replace(' ', "-").replace('/', "-");
    // Strip parenthesised annotations and inline fractions from name
    let name_re = Regex::new(r"\([a-zA-Z\s0-9]+\)|\d+/\d+").unwrap();
    let name = name_re
        .replace_all(name, "")
        .trim()
        .replace(' ', "-")
        .replace('/', "-");
    let num = format_exp_number(number.trim());
    format!("{}-{}-{}", set, name, num)
}

/// Normalise a set name so Serebii / TCGPlayer / PMC names can be compared.
/// Replicates the TypeScript `normalizeSetName` function.
pub fn normalize_set_name(name: &str) -> String {
    let patterns: &[(&str, &str)] = &[
        (r"Pokemon|pokemon|Pokémon|pokémon", "PKM"),
        (r"swsh-sword-and-shield", "Sword & Shield"),
        (r"\ssv\s|\sSV\s", " Scarlet & Violet "),
        (r"swsh|SWSH", "Sword & Shield"),
        (r"\ssm\s|\sSM\s", " Sun & Moon "),
        (r"hgss", "HeartGold SoulSilver"),
        (r"\band\b", "&"),
    ];
    let mut s = name.to_string();
    for (pat, rep) in patterns {
        if let Ok(re) = Regex::new(pat) {
            s = re.replace(&s, *rep).to_string();
        }
    }
    s
}

/// Determine card variants from its rarity (matches TypeScript `pullVariants`).
pub fn pull_variants(rarity: &str) -> Vec<String> {
    match rarity {
        "Common" | "Uncommon" => vec!["Normal".into(), "Reverse Holofoil".into()],
        _ => vec!["Holofoil".into()],
    }
}
