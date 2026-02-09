//! Order data structure

use crate::types::common::{OrderSide, OrderStatus, OrderType, TimeInForce};
use crate::types::trade::Trade;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Order - limit, market, or other order type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Order ID
    pub id: String,

    /// Client order ID (if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,

    /// Unified symbol
    pub symbol: String,

    /// Order type (market, limit, etc.)
    pub order_type: OrderType,

    /// Order side (buy or sell)
    pub side: OrderSide,

    /// Order status (open, closed, canceled, etc.)
    pub status: OrderStatus,

    /// Order creation timestamp (milliseconds)
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Last update timestamp (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_trade_timestamp: Option<i64>,

    /// Order price (None for market orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// Average fill price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average: Option<Decimal>,

    /// Order amount (in base currency)
    pub amount: Decimal,

    /// Filled amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filled: Option<Decimal>,

    /// Remaining amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<Decimal>,

    /// Cost (filled * average price, in quote currency)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Decimal>,

    /// Fee paid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<OrderFee>,

    /// Time in force
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<TimeInForce>,

    /// Post only flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_only: Option<bool>,

    /// Reduce only flag (derivatives)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,

    /// Stop price (for stop orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Decimal>,

    /// Trigger price (for trigger orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_price: Option<Decimal>,

    /// Stop loss price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_price: Option<Decimal>,

    /// Take profit price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_price: Option<Decimal>,

    /// Last update timestamp (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_timestamp: Option<i64>,

    /// Associated trades
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trades: Option<Vec<Trade>>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

/// Fee information for an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    /// Fee amount
    pub cost: Decimal,

    /// Fee currency
    pub currency: String,

    /// Fee rate (e.g., 0.001 for 0.1%)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<Decimal>,
}

/// Order request for batch order creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    /// Unified symbol
    pub symbol: String,

    /// Order type (market, limit, etc.)
    pub order_type: OrderType,

    /// Order side (buy or sell)
    pub side: OrderSide,

    /// Order amount (in base currency)
    pub amount: Decimal,

    /// Order price (None for market orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// Exchange-specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// Cancellation request for batch order cancellation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationRequest {
    /// Order ID
    pub id: String,

    /// Client order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,

    /// Unified symbol (required by some exchanges)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// Edit order request for batch order editing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOrderRequest {
    /// Order ID
    pub id: String,

    /// Unified symbol
    pub symbol: String,

    /// Order type
    pub order_type: OrderType,

    /// Order side
    pub side: OrderSide,

    /// New amount (None to keep current)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<Decimal>,

    /// New price (None to keep current)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
}
