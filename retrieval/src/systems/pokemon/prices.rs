use std::{collections::HashMap, path::PathBuf};

use ::models::{CardPrices, RetailerPrices, parse_price_date};
use chrono::NaiveDate;
use tracing::info;

use crate::http::stream_to_file;

/// The latest usable value of one price column, and the date it was
/// listed on.
pub(super) struct Quote {
    pub price: f64,
    pub date: Option<NaiveDate>,
}

/// SQL selecting, for the card id in `card_expr`, each price column's latest
/// usable value followed by that value's date — six columns in `Quote`
/// order: raw, PSA 10, PSA 9. Zero and the scraper's `20.0` placeholder are
/// skipped.
pub(super) fn latest_quotes_sql(card_expr: &str) -> String {
    ["rawPrice", "gradedPriceTen", "gradedPriceNine"]
        .iter()
        .flat_map(|col| {
            let filter = format!(
                "FROM prices WHERE cardId = {card_expr} AND {col} > 0 AND {col} != 20.0 ORDER BY date DESC LIMIT 1"
            );
            [format!("(SELECT {col} {filter})"), format!("(SELECT date {filter})")]
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reads the six columns of `latest_quotes_sql` starting at `first`.
pub(super) fn quotes_from_row(row: &rusqlite::Row, first: usize) -> rusqlite::Result<[Quote; 3]> {
    let quote = |i: usize| -> rusqlite::Result<Quote> {
        Ok(Quote {
            price: row.get::<_, Option<f64>>(first + 2 * i)?.unwrap_or(0.0),
            // Stored as RFC 3339 timestamps; an unreadable one counts as undated.
            date: row.get::<_, Option<String>>(first + 2 * i + 1)?.as_deref().and_then(parse_price_date),
        })
    };
    Ok([quote(0)?, quote(1)?, quote(2)?])
}

pub(super) fn row_to_card_prices(uuid: &str, [raw, psa10, psa9]: [Quote; 3]) -> CardPrices {
    let mut paper = HashMap::new();
    for (retailer, quote) in [("raw", raw), ("graded_psa10", psa10), ("graded_psa9", psa9)] {
        if quote.price > 0.0 {
            paper.insert(
                retailer.to_string(),
                RetailerPrices {
                    normal: Some(quote.price),
                    foil: None,
                    currency: "USD".to_string(),
                    date: quote.date,
                },
            );
        }
    }
    CardPrices {
        uuid: uuid.to_string(),
        paper,
    }
}

pub async fn download_pokemon_prices(path: &str) -> eyre::Result<()> {
    const DOWNLOAD_URL: &str =
        "https://github.com/poketrax/pokedata/raw/refs/heads/main/databases/prices.sqlite";

    let target = PathBuf::from(path);
    let target_parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));

    if !target_parent.exists() {
        std::fs::create_dir_all(target_parent)?;
    }

    if crate::mirror::try_mirrors("pokemon_prices.sqlite", &target, None).await {
        return Ok(());
    }

    // Download to system temp dir, then stage in target's directory for atomic rename.
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("prices.sqlite");

    info!(url = DOWNLOAD_URL, "Downloading Pokemon prices");
    stream_to_file(
        DOWNLOAD_URL,
        "Download complete",
        &temp_path,
        None,
        "downloading",
    )
    .await?;

    let mut staging = tempfile::NamedTempFile::new_in(target_parent)?;
    std::io::copy(&mut std::fs::File::open(&temp_path)?, &mut staging)?;
    staging.persist(&target).map_err(|e| e.error)?;
    info!(dest = ?target, "Pokemon prices saved");
    Ok(())
}
