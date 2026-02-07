//! Greeks data structures (for options)

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Greeks - option pricing sensitivities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    /// Unified symbol
    pub symbol: String,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Delta - rate of change of option price with respect to underlying
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Decimal>,

    /// Gamma - rate of change of delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma: Option<Decimal>,

    /// Theta - rate of change of option price with respect to time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<Decimal>,

    /// Vega - rate of change of option price with respect to volatility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vega: Option<Decimal>,

    /// Rho - rate of change of option price with respect to interest rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rho: Option<Decimal>,

    /// Bid implied volatility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_implied_volatility: Option<Decimal>,

    /// Ask implied volatility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_implied_volatility: Option<Decimal>,

    /// Mark implied volatility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_implied_volatility: Option<Decimal>,

    /// Bid price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price: Option<Decimal>,

    /// Ask price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price: Option<Decimal>,

    /// Mark price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<Decimal>,

    /// Last price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_price: Option<Decimal>,

    /// Underlying price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_price: Option<Decimal>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
