//! Borrow rate data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Borrow rate for margin trading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorrowRate {
    /// Currency code (for cross margin)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Unified symbol (for isolated margin)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,

    /// Borrow rate
    pub rate: Decimal,

    /// Period (e.g., "1h", "1d")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
