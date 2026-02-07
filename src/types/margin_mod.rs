//! Margin modification data structures

use crate::types::common::MarginMode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Type of margin modification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginModificationType {
    Add,
    Reduce,
    Set,
}

/// Margin modification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginModification {
    /// Unified symbol
    pub symbol: String,

    /// Modification type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modification_type: Option<MarginModificationType>,

    /// Margin mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_mode: Option<MarginMode>,

    /// Amount modified
    pub amount: Decimal,

    /// Total margin after modification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<Decimal>,

    /// Currency code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Status of modification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Timestamp in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,

    /// ISO 8601 datetime string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datetime: Option<String>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
