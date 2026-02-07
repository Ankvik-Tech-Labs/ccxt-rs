//! Funding history data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Funding history entry - a record of funding fee payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingHistory {
    /// Funding entry ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Unified symbol
    pub symbol: String,

    /// Currency code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Funding amount (positive = received, negative = paid)
    pub amount: Decimal,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Funding rate history entry - historical funding rate record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateHistory {
    /// Unified symbol
    pub symbol: String,

    /// Funding rate
    pub funding_rate: Decimal,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
