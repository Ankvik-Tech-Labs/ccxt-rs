//! Transfer data structure (internal account transfers)

use crate::types::common::TransactionStatus;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Transfer - internal transfer between accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    /// Transfer ID
    pub id: String,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Currency code
    pub currency: String,

    /// Transfer amount
    pub amount: Decimal,

    /// Source account
    pub from_account: String,

    /// Destination account
    pub to_account: String,

    /// Transfer status
    pub status: TransactionStatus,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
