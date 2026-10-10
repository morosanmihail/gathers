use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CardPrices {
    pub uuid: String,
    /// Keyed by retailer name.
    pub paper: HashMap<String, RetailerPrices>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetailerPrices {
    pub normal: Option<f64>,
    pub foil: Option<f64>,
    /// ISO 4217 currency code of `normal`/`foil` (e.g. "USD", "EUR").
    #[serde(default = "default_currency")]
    pub currency: String,
    /// UTC day these prices are as of, when the source says — the newest of
    /// `normal`'s and `foil`'s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
}

pub const DEFAULT_CURRENCY: &str = "USD";

fn default_currency() -> String {
    DEFAULT_CURRENCY.to_string()
}

/// The UTC day a price source's date names — a plain date (`2026-10-05`) or
/// an RFC 3339 timestamp (`2026-10-05T04:32:34.361Z`). `None` for anything
/// else, rather than guessing.
pub fn parse_price_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    // chrono accepts unpadded fields ("2026-1-5"); only take the exact form.
    (s.len() == 10)
        .then(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .flatten()
        .or_else(|| DateTime::parse_from_rfc3339(s).ok().map(|t| t.with_timezone(&Utc).date_naive()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(y, m, d)
    }

    #[test]
    fn parses_dates_and_timestamps() {
        assert_eq!(parse_price_date("2026-10-05"), day(2026, 10, 5));
        assert_eq!(parse_price_date(" 2026-10-05 "), day(2026, 10, 5));
        assert_eq!(parse_price_date("2026-10-05T04:32:34.361Z"), day(2026, 10, 5));
        // An offset timestamp is the UTC day it falls on.
        assert_eq!(parse_price_date("2026-10-05T23:30:00-02:00"), day(2026, 10, 6));
    }

    #[test]
    fn rejects_anything_else() {
        for bad in ["", "2026-1-5", "2026-13-01", "05/10/2026", "yesterday", "2026-10-05 04:32"] {
            assert_eq!(parse_price_date(bad), None, "{bad:?}");
        }
    }
}
