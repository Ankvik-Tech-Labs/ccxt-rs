//! Leverage tier data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Leverage tier - describes leverage brackets for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeverageTier {
    /// Tier number
    pub tier: u32,

    /// Currency code for notional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,

    /// Minimum notional value for this tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_notional: Option<Decimal>,

    /// Maximum notional value for this tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_notional: Option<Decimal>,

    /// Maintenance margin rate for this tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_margin_rate: Option<Decimal>,

    /// Maximum leverage for this tier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_leverage: Option<Decimal>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
