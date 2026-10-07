use std::collections::HashMap;

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
}

pub const DEFAULT_CURRENCY: &str = "USD";

fn default_currency() -> String {
    DEFAULT_CURRENCY.to_string()
}
