//! Conversion (currency exchange) data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Conversion - currency exchange/swap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    /// Conversion ID
    pub id: String,

    /// Source currency code
    pub from_currency: String,

    /// Source amount
    pub from_amount: Decimal,

    /// Destination currency code
    pub to_currency: String,

    /// Destination amount
    pub to_amount: Decimal,

    /// Conversion price (exchange rate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
