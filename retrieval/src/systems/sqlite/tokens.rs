//! MTGJSON keeps tokens (and emblems, checklist cards, ...) out of `cards`, in a `tokens` table
//! of their own with a `tokenIdentifiers` table beside it. Every lookup tries `cards` first and
//! falls back to tokens only for what `cards` didn't match.

use std::collections::{HashMap, HashSet};

use ::models::filters::{CardSearchFilters, SortField, UNIQUE_PRINTS};
use rusqlite::{Connection, types::Value};

use super::models::LEGALITY_FORMATS;
use crate::systems::sql_helpers::{sql_placeholders, sql_sort_dir};

/// `FRONT_FACE_UUID` for a `tokens AS a` row: double-faced tokens are stored like double-faced
/// cards, one row per face sharing `setCode` and `number`.
pub(super) const TOKEN_FRONT_FACE_UUID: &str = "CASE WHEN a.side IS NULL THEN a.uuid ELSE \
     COALESCE((SELECT f.uuid FROM tokens AS f \
     WHERE f.setCode = a.setCode AND f.number = a.number ORDER BY f.side LIMIT 1), a.uuid) END";

/// The `SEARCH_INDEXES` a token search needs: one per sort, so it streams off an index and
/// stops at the end of the page, and (setCode, number) for `TOKEN_FRONT_FACE_UUID`.
const TOKEN_INDEXES: &str = "
    CREATE INDEX IF NOT EXISTS idx_tokens_name_nocase ON tokens (name COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_tokens_setcode_number ON tokens (setCode, number);
    CREATE INDEX IF NOT EXISTS idx_tokens_setcode_nocase ON tokens (setCode COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_tokens_artist_nocase ON tokens (artist COLLATE NOCASE);
    CREATE INDEX IF NOT EXISTS idx_tokens_number_int ON tokens (CAST(number AS INTEGER));";

pub(super) fn create_indexes(conn: &Connection) -> eyre::Result<()> {
    if table_exists(conn, "tokens")? {
        conn.execute_batch(TOKEN_INDEXES)?;
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> eyre::Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

/// The tokens counterpart of `select_base`: the same column names, so `SqlCard::from_row`
/// reads both. `None` when the database has no tokens (older or test databases).
///
/// Tokens have no rarity (Scryfall calls them common), mana value, loyalty, defense, reserved
/// status or legalities, and several text columns are NULL where `cards` would hold a value.
/// A token set (e.g. `TM13`) has no `sets` row of its own; it's named after the set listing it
/// as its `tokenSetCode`, the way Scryfall names it ("Magic 2013 Tokens").
pub(super) fn token_select_base(conn: &Connection) -> eyre::Result<Option<String>> {
    if !table_exists(conn, "tokens")? || !table_exists(conn, "tokenIdentifiers")? {
        return Ok(None);
    }
    let has_token_set_code: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('sets') WHERE name = 'tokenSetCode')",
        [],
        |row| row.get(0),
    )?;
    let set_name = if has_token_set_code {
        "(SELECT s.name || ' Tokens' FROM sets AS s WHERE s.tokenSetCode = a.setCode LIMIT 1)"
    } else {
        "NULL"
    };
    let legality_cols: String = LEGALITY_FORMATS
        .iter()
        .map(|f| format!("NULL AS {f}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(format!(
        "SELECT a.uuid, a.name, a.setCode, 'common' AS rarity, \
         COALESCE(a.artist, '') AS artist, COALESCE(a.colorIdentity, '') AS colorIdentity, \
         COALESCE(a.text, '') AS text, b.scryfallId, a.number, \
         COALESCE(a.subtypes, '') AS subtypes, COALESCE(a.supertypes, '') AS supertypes, \
         COALESCE(a.types, '') AS types, a.manaCost, NULL AS manaValue, a.type, a.power, \
         a.toughness, NULL AS loyalty, NULL AS defense, a.keywords, a.colors, a.finishes, \
         NULL AS isReserved, a.isPromo, a.isReprint, a.borderColor, a.frameEffects, a.isFullArt, \
         a.watermark, a.flavorText, {set_name} AS set_name, {legality_cols} \
         FROM tokens AS a \
         JOIN tokenIdentifiers AS b ON a.uuid = b.uuid"
    )))
}

/// Where a printing falls in a search's sort order, compared the way SQLite compares the
/// `sort_col` expressions `search_cards` sorts by: `COLLATE NOCASE` folds ASCII case only, and
/// `CAST(number AS INTEGER)` reads a collector number's leading digits ("63s" is 63, "ab89"
/// is 0).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SortValue {
    Int(i64),
    Text(Vec<u8>),
}

impl SortValue {
    fn new(sort_by: Option<&SortField>, row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let text = |column: &str| -> rusqlite::Result<SortValue> {
            let value: Option<String> = row.get(column)?;
            Ok(SortValue::Text(value.unwrap_or_default().to_ascii_lowercase().into_bytes()))
        };
        match sort_by {
            Some(SortField::Rarity) => text("rarity"),
            Some(SortField::SetCode) => text("setCode"),
            Some(SortField::Artist) => text("artist"),
            Some(SortField::CollectorNumber) => {
                let number: Option<String> = row.get("number")?;
                Ok(SortValue::Int(leading_integer(number.as_deref().unwrap_or_default())))
            }
            _ => text("name"),
        }
    }
}

/// SQLite's `CAST(x AS INTEGER)` for text: an optional sign and the digits after it, else 0.
fn leading_integer(value: &str) -> i64 {
    let value = value.trim_start();
    let (sign, digits) = match value.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, value.strip_prefix('+').unwrap_or(value)),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<i64>().map_or(0, |n| sign * n)
}

/// The sort value of each card in `ids` (front-face uuids from a search).
pub(super) fn card_sort_values(
    conn: &Connection,
    ids: &[String],
    sort_by: Option<&SortField>,
) -> eyre::Result<HashMap<String, SortValue>> {
    let mut values = HashMap::with_capacity(ids.len());
    // Well under SQLite's limit on bound parameters.
    for chunk in ids.chunks(900) {
        let query = format!(
            "SELECT uuid, name, rarity, setCode, number, artist FROM cards WHERE uuid IN ({})",
            sql_placeholders(chunk.len())
        );
        let mut stmt = conn.prepare_cached(&query)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(chunk))?;
        while let Some(row) = rows.next()? {
            values.insert(row.get("uuid")?, SortValue::new(sort_by, row)?);
        }
    }
    Ok(values)
}

/// Merges sorted cards and sorted tokens into one sorted list of `(is_token, uuid)`. On a tie
/// the card comes first. A card without a sort value (it vanished between queries) sorts
/// where it stands.
pub(super) fn merge<'a>(
    cards: impl IntoIterator<Item = (&'a String, Option<&'a SortValue>)>,
    tokens: &'a [(String, SortValue)],
    descending: bool,
) -> Vec<(bool, &'a String)> {
    let mut merged = Vec::new();
    let mut tokens = tokens.iter().peekable();
    for (card, value) in cards {
        while let Some((token, token_value)) = tokens.peek() {
            let token_first = match value {
                Some(value) if descending => token_value > value,
                Some(value) => token_value < value,
                None => false,
            };
            if !token_first {
                break;
            }
            merged.push((true, token));
            tokens.next();
        }
        merged.push((false, card));
    }
    merged.extend(tokens.map(|(token, _)| (true, token)));
    merged
}

/// Front-face uuids and sort values of the first `max` tokens matching `filters`, in search
/// order: no more can land on a page ending at `max`.
///
/// The tokens table is small (~10k rows), so faces and `unique` modes are collapsed here rather
/// than in SQL. A filter on something tokens don't have (rarity, mana
/// value, loyalty, defense, reserved list, legality) matches no token.
pub(super) fn search_tokens(
    conn: &Connection,
    filters: &CardSearchFilters,
    mode: &str,
    max: usize,
) -> eyre::Result<Vec<(String, SortValue)>> {
    if max == 0
        || filters.rarity.is_some()
        || filters.mana_value_min.is_some_and(|min| min > 0.0)
        || filters.mana_value_max.is_some_and(|max| max < 0.0)
        || filters.loyalty.as_ref().is_some_and(|l| !l.is_empty())
        || filters.defense.as_ref().is_some_and(|d| !d.is_empty())
        || filters.is_reserved == Some(true)
        // An unknown format is ignored, as `search_cards` ignores it.
        || filters
            .legal_in
            .as_ref()
            .is_some_and(|f| LEGALITY_FORMATS.contains(&f.to_lowercase().as_str()))
    {
        return Ok(vec![]);
    }

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut push = |condition: &str, value: String| {
        params.push(Value::Text(value));
        conditions.push(condition.replace('?', &format!("?{}", params.len())));
    };

    // Case-insensitive substring matches, like the trigram FTS index `cards` is searched with.
    for (column, value) in [
        ("a.name", &filters.name),
        ("a.artist", &filters.artist),
        ("a.text", &filters.text),
    ] {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty()) {
            push(&format!("instr(lower({column}), lower(?)) > 0"), value.clone());
        }
    }
    if let Some(set_code) = filters.set_code.as_ref().filter(|s| !s.is_empty()) {
        if set_code.contains(['%', '_']) {
            push("a.setCode LIKE ?", set_code.clone());
        } else {
            push("a.setCode = ? COLLATE NOCASE", set_code.clone());
        }
    }
    if let Some(number) = filters.collector_number.as_ref().filter(|n| !n.is_empty()) {
        push("a.number = ?", number.clone());
    }
    for colour in filters.color_identities.iter().flatten() {
        push("a.colorIdentity LIKE ?", format!("%{colour}%"));
    }
    for colour in filters.colors.iter().flatten() {
        push("a.colors LIKE ?", format!("%{colour}%"));
    }
    for t in filters.types.iter().flatten() {
        push("a.types LIKE ?", format!("%{t}%"));
    }
    for s in filters.subtypes.iter().flatten() {
        push("a.subtypes LIKE ?", format!("%{s}%"));
    }
    if let Some(supertype) = filters.supertypes.as_ref().filter(|s| !s.is_empty()) {
        push("a.supertypes LIKE ?", format!("%{supertype}%"));
    }
    for k in filters.keywords.iter().flatten() {
        push("a.keywords LIKE ?", format!("%{k}%"));
    }
    for (column, value) in [("a.power", &filters.power), ("a.toughness", &filters.toughness)] {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty()) {
            push(&format!("{column} = ?"), value.clone());
        }
    }
    if let Some(border) = filters.border_color.as_ref().filter(|b| !b.is_empty()) {
        push("a.borderColor = ? COLLATE NOCASE", border.clone());
    }
    // After the last `push`, which borrows both lists.
    if let Some(any_types) = filters.any_types.as_ref().filter(|t| !t.is_empty()) {
        let any: Vec<String> = any_types
            .iter()
            .map(|t| {
                params.push(Value::Text(format!("%{t}%")));
                format!("a.types LIKE ?{}", params.len())
            })
            .collect();
        conditions.push(format!("({})", any.join(" OR ")));
    }
    for (column, value) in [
        ("a.isPromo", filters.is_promo),
        ("a.isReprint", filters.is_reprint),
        ("a.isFullArt", filters.is_full_art),
    ] {
        match value {
            Some(true) => conditions.push(format!("{column} = 1")),
            Some(false) => conditions.push(format!("({column} IS NULL OR {column} = 0)")),
            None => {}
        }
    }

    let key = match mode {
        "cards" => "a.name",
        "art" => "COALESCE(b.scryfallIllustrationId, a.uuid)",
        _ => {
            debug_assert_eq!(mode, UNIQUE_PRINTS);
            "''"
        }
    };
    let sort_col = match &filters.sort_by {
        Some(SortField::SetCode) => "a.setCode COLLATE NOCASE",
        Some(SortField::CollectorNumber) => "CAST(a.number AS INTEGER)",
        Some(SortField::Artist) => "a.artist COLLATE NOCASE",
        // Tokens have no rarity to sort by.
        _ => "a.name COLLATE NOCASE",
    };
    let mut query = format!(
        "SELECT {TOKEN_FRONT_FACE_UUID} AS front_uuid, {key} AS unique_key, a.name, \
         'common' AS rarity, a.setCode, a.number, a.artist FROM tokens AS a \
         JOIN tokenIdentifiers AS b ON a.uuid = b.uuid"
    );
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }
    query.push_str(&format!(" ORDER BY {sort_col} {}", sql_sort_dir(&filters.sort_order)));

    let mut stmt = conn.prepare_cached(&query)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
    let mut seen_fronts = HashSet::new();
    let mut seen_keys = HashSet::new();
    let mut found = Vec::new();
    while let Some(row) = rows.next()? {
        let front: String = row.get("front_uuid")?;
        let key: String = row.get("unique_key")?;
        if !seen_fronts.insert(front.clone()) || (mode != UNIQUE_PRINTS && !seen_keys.insert(key))
        {
            continue;
        }
        found.push((front, SortValue::new(filters.sort_by.as_ref(), row)?));
        if found.len() == max {
            break;
        }
    }
    Ok(found)
}
