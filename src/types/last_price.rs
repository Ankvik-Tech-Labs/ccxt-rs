//! Last price data structures

use crate::types::common::OrderSide;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Last traded price for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPrice {
    /// Unified symbol
    pub symbol: String,

    /// Last price
    pub price: Decimal,

    /// Side of the last trade
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<OrderSide>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
