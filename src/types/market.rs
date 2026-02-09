//! Market data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Market - trading pair information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// Exchange-native symbol ID (e.g., "BTCUSDT" for Binance)
    pub id: String,

    /// Unified symbol (e.g., "BTC/USDT")
    pub symbol: String,

    /// Base currency (e.g., "BTC")
    pub base: String,

    /// Quote currency (e.g., "USDT")
    pub quote: String,

    /// Settlement currency (for derivatives)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle: Option<String>,

    /// Exchange-specific base currency ID
    pub base_id: String,

    /// Exchange-specific quote currency ID
    pub quote_id: String,

    /// Exchange-specific settle currency ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_id: Option<String>,

    /// Market type
    pub market_type: String,

    /// Whether the market is a spot market
    pub spot: bool,

    /// Whether the market is a margin market
    pub margin: bool,

    /// Whether the market is a swap market (perpetual)
    pub swap: bool,

    /// Whether the market is a future market
    pub future: bool,

    /// Whether the market is an option market
    pub option: bool,

    /// Whether the market is active (tradeable)
    pub active: bool,

    /// Contract type (for derivatives)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<bool>,

    /// Whether the market is linear (settled in quote currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linear: Option<bool>,

    /// Whether the market is inverse (settled in base currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<bool>,

    /// Taker fee rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taker: Option<Decimal>,

    /// Maker fee rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maker: Option<Decimal>,

    /// Contract size (for derivatives)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_size: Option<Decimal>,

    /// Expiry timestamp (for futures/options)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<i64>,

    /// Expiry datetime (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_datetime: Option<String>,

    /// Strike price (for options)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<Decimal>,

    /// Option type (call or put)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_type: Option<String>,

    /// Market creation timestamp (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,

    /// Supported margin modes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_modes: Option<MarginModes>,

    /// Precision settings
    pub precision: MarketPrecision,

    /// Limits (min/max values)
    pub limits: MarketLimits,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Supported margin modes for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginModes {
    /// Whether cross margin is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross: Option<bool>,

    /// Whether isolated margin is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolated: Option<bool>,
}

/// Market precision settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrecision {
    /// Price precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<i32>,

    /// Amount precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i32>,

    /// Cost precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<i32>,

    /// Base precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<i32>,

    /// Quote precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<i32>,
}

/// Market limits (min/max constraints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLimits {
    /// Amount limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinMax>,

    /// Price limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<MinMax>,

    /// Cost limits (amount * price)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<MinMax>,

    /// Leverage limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<MinMax>,
}

/// Min/max value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinMax {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Decimal>,
}
