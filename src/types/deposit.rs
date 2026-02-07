//! Deposit and withdrawal data structures

use crate::types::common::{TransactionStatus, TransactionType};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Deposit address information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositAddress {
    /// Currency code
    pub currency: String,

    /// Deposit address
    pub address: String,

    /// Address tag/memo (for currencies that require it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Network/chain (e.g., "ETH", "BSC", "TRX")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Deposit transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    /// Transaction ID
    pub id: String,

    /// Transaction hash (blockchain txid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Network/chain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// Deposit address
    pub address: String,

    /// Address tag/memo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Transaction type (deposit)
    pub transaction_type: TransactionType,

    /// Deposit amount
    pub amount: Decimal,

    /// Currency code
    pub currency: String,

    /// Transaction status
    pub status: TransactionStatus,

    /// Last update timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<i64>,

    /// Fee paid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<TransactionFee>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Withdrawal transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Withdrawal {
    /// Transaction ID
    pub id: String,

    /// Transaction hash (blockchain txid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Network/chain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// Withdrawal address
    pub address: String,

    /// Address tag/memo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Transaction type (withdrawal)
    pub transaction_type: TransactionType,

    /// Withdrawal amount
    pub amount: Decimal,

    /// Currency code
    pub currency: String,

    /// Transaction status
    pub status: TransactionStatus,

    /// Last update timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<i64>,

    /// Fee paid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<TransactionFee>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Fee information for deposits/withdrawals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFee {
    /// Fee amount
    pub cost: Decimal,

    /// Fee currency
    pub currency: String,
}
