//! Position data structures (for derivatives)

use crate::types::common::{MarginMode, PositionSide};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Position - open position in derivatives market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Unified symbol
    pub symbol: String,

    /// Position ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Position side (long/short)
    pub side: PositionSide,

    /// Margin mode (isolated/cross)
    pub margin_mode: MarginMode,

    /// Position contracts/amount
    pub contracts: Decimal,

    /// Contract size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_size: Option<Decimal>,

    /// Notional value (contracts * contract_size * mark_price)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notional: Option<Decimal>,

    /// Leverage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leverage: Option<Decimal>,

    /// Entry price (average)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_price: Option<Decimal>,

    /// Mark price (current)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<Decimal>,

    /// Unrealized PnL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unrealized_pnl: Option<Decimal>,

    /// Realized PnL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realized_pnl: Option<Decimal>,

    /// Collateral amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collateral: Option<Decimal>,

    /// Initial margin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_margin: Option<Decimal>,

    /// Maintenance margin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_margin: Option<Decimal>,

    /// Liquidation price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation_price: Option<Decimal>,

    /// Margin ratio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_ratio: Option<Decimal>,

    /// Percentage (PnL percentage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<Decimal>,

    /// Stop loss price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_price: Option<Decimal>,

    /// Take profit price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_price: Option<Decimal>,

    /// Whether this is a hedged position
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hedged: Option<bool>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}
