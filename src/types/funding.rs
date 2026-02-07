//! Funding rate data structures (for perpetual swaps)

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Funding rate for perpetual swap contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    /// Unified symbol
    pub symbol: String,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Current funding rate
    pub funding_rate: Decimal,

    /// Next funding timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_timestamp: Option<i64>,

    /// Next funding datetime (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_datetime: Option<String>,

    /// Mark price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<Decimal>,

    /// Index price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_price: Option<Decimal>,

    /// Interest rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interest_rate: Option<Decimal>,

    /// Estimated next funding rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_settle_price: Option<Decimal>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
