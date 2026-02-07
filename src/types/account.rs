//! Account data structures

use serde::{Deserialize, Serialize};

/// Account type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Spot,
    Margin,
    Futures,
    Swap,
    Option,
    Funding,
}

/// Account - exchange sub-account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Account ID
    pub id: String,

    /// Account type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<AccountType>,

    /// Currency code (if account is currency-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
