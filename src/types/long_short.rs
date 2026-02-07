//! Long/short ratio data structures

use crate::types::common::Timeframe;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Long/short ratio for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongShortRatio {
    /// Unified symbol
    pub symbol: String,

    /// Long/short ratio value
    pub long_short_ratio: Decimal,

    /// Long account percentage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_account: Option<Decimal>,

    /// Short account percentage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_account: Option<Decimal>,

    /// Timeframe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<Timeframe>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
