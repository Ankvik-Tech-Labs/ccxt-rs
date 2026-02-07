//! Ticker data structure

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Ticker - current market price and 24h stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    /// Unified symbol (e.g., "BTC/USDT")
    pub symbol: String,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Highest price in last 24h
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<Decimal>,

    /// Lowest price in last 24h
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<Decimal>,

    /// Current best bid price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid: Option<Decimal>,

    /// Current best bid amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_volume: Option<Decimal>,

    /// Current best ask price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<Decimal>,

    /// Current best ask amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_volume: Option<Decimal>,

    /// Volume-weighted average price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vwap: Option<Decimal>,

    /// Opening price (24h ago)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<Decimal>,

    /// Closing price (most recent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close: Option<Decimal>,

    /// Last traded price (same as close)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Decimal>,

    /// Last traded price before current
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_close: Option<Decimal>,

    /// Price change (absolute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<Decimal>,

    /// Price change (percentage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<Decimal>,

    /// Average price (high + low) / 2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average: Option<Decimal>,

    /// Base volume (24h)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_volume: Option<Decimal>,

    /// Quote volume (24h)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_volume: Option<Decimal>,

    /// Raw exchange response (for debugging/advanced use)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
