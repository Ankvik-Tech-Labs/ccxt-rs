//! Currency data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Currency - information about a tradeable currency/token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currency {
    /// Unified currency code (e.g., "BTC", "USDT")
    pub code: String,

    /// Exchange-specific currency ID
    pub id: String,

    /// Currency name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Whether the currency is active (deposits/withdrawals enabled)
    pub active: bool,

    /// Deposit enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit: Option<bool>,

    /// Withdrawal enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw: Option<bool>,

    /// Fee for withdrawals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<Decimal>,

    /// Precision (decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<i32>,

    /// Limits for deposits/withdrawals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<CurrencyLimits>,

    /// Networks (for multi-chain currencies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<Network>>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Currency limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyLimits {
    /// Withdrawal limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw: Option<MinMax>,

    /// Deposit limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit: Option<MinMax>,
}

/// Min/max value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinMax {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Decimal>,
}

/// Network information for multi-chain currencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// Network ID (e.g., "ETH", "BSC", "TRX")
    pub id: String,

    /// Network name
    pub network: String,

    /// Network full name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Whether deposits are enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit: Option<bool>,

    /// Whether withdrawals are enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw: Option<bool>,

    /// Withdrawal fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<Decimal>,

    /// Precision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<i32>,

    /// Limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<CurrencyLimits>,
}
