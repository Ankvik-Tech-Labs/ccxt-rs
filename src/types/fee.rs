//! Fee data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Trading fee structure for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingFees {
    /// Unified symbol
    pub symbol: String,

    /// Maker fee rate
    pub maker: Decimal,

    /// Taker fee rate
    pub taker: Decimal,

    /// Percentage-based fee (true) or fixed (false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<bool>,

    /// Tier-based fees
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_based: Option<bool>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Transaction fee (for deposits/withdrawals)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFee {
    /// Fee type (e.g., "withdrawal", "deposit")
    pub fee_type: String,

    /// Currency code
    pub currency: String,

    /// Fee rate or fixed amount
    pub rate: Decimal,

    /// Fee cost (actual amount paid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Decimal>,
}
