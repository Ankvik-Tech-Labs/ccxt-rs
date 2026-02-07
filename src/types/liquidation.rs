//! Liquidation data structures

use crate::types::common::OrderSide;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Liquidation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Liquidation {
    /// Unified symbol
    pub symbol: String,

    /// Liquidation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Liquidation price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// Base value (in base currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_value: Option<Decimal>,

    /// Quote value (in quote currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_value: Option<Decimal>,

    /// Number of contracts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contracts: Option<Decimal>,

    /// Side that was liquidated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<OrderSide>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
