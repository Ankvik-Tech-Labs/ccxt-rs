//! Leverage data structures

use crate::types::common::MarginMode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Leverage settings for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leverage {
    /// Unified symbol
    pub symbol: String,

    /// Margin mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_mode: Option<MarginMode>,

    /// Long leverage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_leverage: Option<Decimal>,

    /// Short leverage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_leverage: Option<Decimal>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
