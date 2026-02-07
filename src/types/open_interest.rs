//! Open interest data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Open interest for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenInterest {
    /// Unified symbol
    pub symbol: String,

    /// Open interest amount (in contracts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_interest_amount: Option<Decimal>,

    /// Open interest value (in quote currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_interest_value: Option<Decimal>,

    /// Base volume
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_volume: Option<Decimal>,

    /// Quote volume
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_volume: Option<Decimal>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
