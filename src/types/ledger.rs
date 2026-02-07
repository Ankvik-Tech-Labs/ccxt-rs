//! Ledger entry data structures

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Direction of a ledger entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LedgerDirection {
    In,
    Out,
}

/// Type of ledger entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryType {
    Trade,
    Fee,
    Deposit,
    Withdrawal,
    Transfer,
    Rebate,
    Cashback,
    Referral,
    FundingFee,
    Liquidation,
    Margin,
    Other,
}

/// Ledger entry - a record of account activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Ledger entry ID
    pub id: String,

    /// Direction (in or out)
    pub direction: LedgerDirection,

    /// Account type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,

    /// Referral or order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,

    /// Reference account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_account: Option<String>,

    /// Type of ledger entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_type: Option<LedgerEntryType>,

    /// Currency code
    pub currency: String,

    /// Amount
    pub amount: Decimal,

    /// Balance before the entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Decimal>,

    /// Balance after the entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Decimal>,

    /// Fee paid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<LedgerFee>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Associated symbol (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Fee information for a ledger entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerFee {
    /// Fee cost
    pub cost: Decimal,

    /// Fee currency
    pub currency: String,
}
