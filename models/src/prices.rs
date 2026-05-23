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
}
