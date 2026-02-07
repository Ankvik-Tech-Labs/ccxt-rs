//! Deposit/withdraw fee data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Network fee information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkFee {
    /// Withdrawal fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_fee: Option<Decimal>,

    /// Deposit fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_fee: Option<Decimal>,

    /// Whether the fee is a percentage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_fee_percentage: Option<bool>,

    /// Whether the deposit fee is a percentage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_fee_percentage: Option<bool>,
}

/// Deposit and withdrawal fees for a currency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositWithdrawFee {
    /// Currency code
    pub currency: String,

    /// Default withdrawal fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_fee: Option<Decimal>,

    /// Default deposit fee
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_fee: Option<Decimal>,

    /// Network-specific fees (keyed by network name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<HashMap<String, NetworkFee>>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
