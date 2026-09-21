//! `unique` search modes (`cards`, `art`) for the MTGJSON SQLite backend, collapsing every
//! printing that shares a card (or artwork) into one result.
//!
//! Filters apply to printings *first*, then the matches are collapsed, so `set:m20 jace` with
//! `cards` shows the M20 printing. The printing that stands for a card is the best-ranked one
//! among the matches (see [`build_rank_sql`]).
//!
//! Two strategies, depending on the sort:
//! - **By name** (the default) streams the matches in name order off the same index a plain
//!   search uses, collapsing as it goes, and stops once the page is filled.
//! - **Any other sort** has to pick every card's representative before it can order them, so
//!   it collapses all matches in SQL. That costs a few hundred ms for a very broad search
//!   (`unique=cards` with no filters); narrow filters are instant.

use std::collections::HashSet;

use rusqlite::{Connection, types::Value};

use super::FRONT_FACE_UUID;

/// The SQL expression (over a `cards AS a` row) identifying which card, or artwork, a
/// printing belongs to, for a `unique` mode. Printings with equal keys collapse into one.
/// Errors for a mode this backend doesn't know.
///
/// Faces of a double-faced card share a printing, so the key is read from the front face:
/// faces share an oracle id but often not an illustration id. Rows missing the id key on
/// their own printing instead of all collapsing together.
pub(super) fn key_sql(mode: &str) -> eyre::Result<String> {
    let lookup = |column: &str, uuid: &str| {
        format!("(SELECT ci.{column} FROM cardIdentifiers AS ci WHERE ci.uuid = {uuid})")
    };
    Ok(match mode {
        "cards" => format!(
            "COALESCE({}, {FRONT_FACE_UUID})",
            lookup("scryfallOracleId", "a.uuid")
        ),
        "art" => format!(
            "COALESCE({}, {FRONT_FACE_UUID})",
            lookup("scryfallIllustrationId", &format!("({FRONT_FACE_UUID})"))
        ),
        other => eyre::bail!("Unsupported unique mode '{other}'"),
    })
}

/// Builds the SQL expression (over a `cards AS a` row) ranking how well a printing represents
/// its card; lower is better. Preference, most important first: paper over digital-only,
/// English over other languages, ordinary over oversized/funny/textless/alternative, non-promo
/// over promo, then the newest set.
///
/// Built from the columns the database actually has, since a trimmed or older MTGJSON build
/// may lack some of them; with none of them it is a constant and ties fall back to the uuid.
pub(super) fn build_rank_sql(conn: &Connection) -> String {
    let columns = |table: &str| -> HashSet<String> {
        conn.prepare("SELECT name FROM pragma_table_info(?1)")
            .and_then(|mut stmt| {
                stmt.query_map([table], |row| row.get::<_, String>(0))?
                    .collect::<Result<_, _>>()
            })
            .unwrap_or_default()
    };
    let cards = columns("cards");
    let sets = columns("sets");
    let has = |c: &str| cards.contains(c);

    // (weight, conditions any of which puts the printing in that bucket)
    let mut buckets: Vec<(u32, Vec<&str>)> = vec![
        (16, vec![]),
        (8, vec![]),
        (4, vec![]),
        (2, vec![]),
    ];
    if has("availability") {
        buckets[0].1.push("(a.availability IS NOT NULL AND a.availability NOT LIKE '%paper%')");
    }
    if has("isOnlineOnly") {
        buckets[0].1.push("a.isOnlineOnly = 1");
    }
    if has("language") {
        buckets[1].1.push("(a.language IS NOT NULL AND a.language <> 'English')");
    }
    for (column, condition) in [
        ("isOversized", "a.isOversized = 1"),
        ("isFunny", "a.isFunny = 1"),
        ("isTextless", "a.isTextless = 1"),
        ("isAlternative", "a.isAlternative = 1"),
    ] {
        if has(column) {
            buckets[2].1.push(condition);
        }
    }
    if has("isPromo") {
        buckets[3].1.push("a.isPromo = 1");
    }

    let penalties: Vec<String> = buckets
        .into_iter()
        .filter(|(_, conditions)| !conditions.is_empty())
        .map(|(weight, conditions)| {
            format!("CASE WHEN {} THEN {weight} ELSE 0 END", conditions.join(" OR "))
        })
        .collect();
    let penalty = if penalties.is_empty() {
        "0".to_string()
    } else {
        format!("({})", penalties.join(" + "))
    };
    // Dates are `YYYY-MM-DD`, so dropping the dashes gives an integer that sorts the same
    // way; it stays below the penalty's weight (1e8) so it only orders within a bucket.
    let recency = if sets.contains("releaseDate") {
        "COALESCE(CAST(replace((SELECT s.releaseDate FROM sets AS s WHERE s.code = a.setCode), '-', '') AS INTEGER), 0)"
    } else {
        "0"
    };
    format!("({penalty} * 100000000 - {recency})")
}

pub(super) struct UniqueQuery<'a> {
    /// `FROM cards AS a ...` through the end of the `WHERE` clause.
    pub from_where: &'a str,
    /// Values bound to the `?N` placeholders in `from_where`.
    pub params: Vec<Value>,
    /// See [`key_sql`].
    pub key_sql: &'a str,
    /// See [`build_rank_sql`].
    pub rank_sql: &'a str,
}

impl UniqueQuery<'_> {
    /// Collapses matches sorted by name, streaming. Returns the front-face uuids for
    /// `skip..skip + limit` of the collapsed results.
    ///
    /// Printings of a card share a name, so they arrive together, as a "run" of equal names.
    /// A run is only judged once it is complete, so its best-ranked printing can be picked
    /// (the SQL can't order by rank without giving up the streaming index). The result is
    /// paged after collapsing, so a window that collapses down to too few results is
    /// retried, doubled.
    pub fn by_name(
        &self,
        conn: &Connection,
        descending: bool,
        skip: usize,
        limit: usize,
    ) -> eyre::Result<Vec<String>> {
        let query = format!(
            "SELECT {FRONT_FACE_UUID} AS front_uuid, {} AS unique_key, {} AS rank, \
             a.name COLLATE NOCASE AS name {} ORDER BY a.name COLLATE NOCASE {} LIMIT ?{}",
            self.key_sql,
            self.rank_sql,
            self.from_where,
            if descending { "DESC" } else { "ASC" },
            self.params.len() + 1,
        );
        let mut values = self.params.clone();
        values.push(Value::Integer(0));
        let limit_param = values.len() - 1;

        struct Printing {
            key: String,
            front_uuid: String,
            rank: i64,
        }
        // Moves the best printing of each not-yet-seen card in `run` to `out`.
        fn flush(run: &mut Vec<Printing>, seen: &mut HashSet<String>, out: &mut Vec<String>) {
            run.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.front_uuid.cmp(&b.front_uuid)));
            for printing in run.drain(..) {
                if seen.insert(printing.key) {
                    out.push(printing.front_uuid);
                }
            }
        }

        let wanted = skip + limit;
        // A card is typically several printings; start with a window likely to fill the page.
        let mut fetch = (wanted * 4).max(64);
        loop {
            values[limit_param] = Value::Integer(fetch as i64);
            let mut stmt = conn.prepare_cached(&query)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(values.iter()))?;

            let mut seen = HashSet::new();
            let mut collapsed: Vec<String> = Vec::new();
            let mut run: Vec<Printing> = Vec::new();
            let mut run_name: Option<String> = None;
            let mut fetched = 0;
            let mut done = false;
            while let Some(row) = rows.next()? {
                fetched += 1;
                let name: Option<String> = row.get(3)?;
                if !run.is_empty() && name != run_name {
                    flush(&mut run, &mut seen, &mut collapsed);
                    if collapsed.len() >= wanted {
                        done = true;
                        break;
                    }
                }
                run_name = name;
                run.push(Printing {
                    front_uuid: row.get(0)?,
                    key: row.get(1)?,
                    rank: row.get(2)?,
                });
            }
            // Ran out of rows rather than window: the last run is complete too.
            if !done && fetched < fetch {
                flush(&mut run, &mut seen, &mut collapsed);
                done = true;
            }
            if done {
                return Ok(collapsed.into_iter().skip(skip).take(limit).collect());
            }
            fetch *= 2;
        }
    }

    /// Collapses all matches, then sorts them by `sort_col` (an expression over `a`) and
    /// pages in SQL. Returns the front-face uuids for `skip..skip + limit`.
    pub fn grouped(
        &self,
        conn: &Connection,
        sort_col: &str,
        descending: bool,
        skip: usize,
        limit: usize,
    ) -> eyre::Result<Vec<String>> {
        let query = format!(
            "SELECT front_uuid FROM ( \
                 SELECT {FRONT_FACE_UUID} AS front_uuid, {sort_col} AS sort_value, \
                 ROW_NUMBER() OVER (PARTITION BY {} ORDER BY {}, a.uuid) AS position {} \
             ) WHERE position = 1 ORDER BY sort_value {}, front_uuid LIMIT ?{} OFFSET ?{}",
            self.key_sql,
            self.rank_sql,
            self.from_where,
            if descending { "DESC" } else { "ASC" },
            self.params.len() + 1,
            self.params.len() + 2,
        );
        let mut values = self.params.clone();
        values.push(Value::Integer(limit as i64));
        values.push(Value::Integer(skip as i64));
        let mut stmt = conn.prepare_cached(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
