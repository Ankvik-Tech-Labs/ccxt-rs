//! Option contract data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Option contract type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    Call,
    Put,
}

/// Option contract - information about an options contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    /// Unified symbol
    pub symbol: String,

    /// Currency code of the underlying
    pub currency: String,

    /// Strike price
    pub strike: Decimal,

    /// Expiry timestamp in milliseconds
    pub expiry: i64,

    /// Expiry datetime (ISO 8601)
    pub expiry_datetime: String,

    /// Option type (call or put)
    pub option_type: OptionType,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Implied volatility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implied_volatility: Option<Decimal>,

    /// Open interest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_interest: Option<Decimal>,

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
