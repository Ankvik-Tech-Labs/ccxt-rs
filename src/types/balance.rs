//! Balance data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Account balances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balances {
    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Balances by currency code
    pub balances: HashMap<String, Balance>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Balance for a single currency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    /// Currency code (e.g., "BTC", "USDT")
    pub currency: String,

    /// Free (available) amount
    pub free: Decimal,

    /// Used (locked in orders) amount
    pub used: Decimal,

    /// Total amount (free + used)
    pub total: Decimal,
}

impl Balance {
    /// Create a new balance
    pub fn new(currency: String, free: Decimal, used: Decimal) -> Self {
        Self {
            currency,
            free,
            used,
            total: free + used,
        }
    }
}
