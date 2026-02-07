//! Trade data structure

use crate::types::common::OrderSide;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Trade - executed trade on the exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Trade ID
    pub id: String,

    /// Unified symbol
    pub symbol: String,

    /// Order ID that created this trade
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Trade side (buy or sell)
    pub side: OrderSide,

    /// Trade price
    pub price: Decimal,

    /// Trade amount (in base currency)
    pub amount: Decimal,

    /// Trade cost (price * amount, in quote currency)
    pub cost: Decimal,

    /// Fee paid for this trade
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<TradeFee>,

    /// Whether this is a maker or taker trade
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taker_or_maker: Option<String>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Fee information for a trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeFee {
    /// Fee amount
    pub cost: Decimal,

    /// Fee currency
    pub currency: String,

    /// Fee rate (e.g., 0.001 for 0.1%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<Decimal>,
}
