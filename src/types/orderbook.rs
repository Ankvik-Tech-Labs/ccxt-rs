//! Order book data structure

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Order book entry (price and amount)
pub type OrderBookEntry = (Decimal, Decimal);

/// Order book - bids and asks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    /// Unified symbol
    pub symbol: String,

    /// Timestamp in milliseconds
    pub timestamp: i64,

    /// ISO 8601 datetime string
    pub datetime: String,

    /// Nonce (sequence number, if supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,

    /// Bids (buy orders) sorted by price descending
    /// Each entry is (price, amount)
    pub bids: Vec<OrderBookEntry>,

    /// Asks (sell orders) sorted by price ascending
    /// Each entry is (price, amount)
    pub asks: Vec<OrderBookEntry>,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

impl OrderBook {
    /// Get best bid (highest buy price)
    pub fn best_bid(&self) -> Option<&OrderBookEntry> {
        self.bids.first()
    }

    /// Get best ask (lowest sell price)
    pub fn best_ask(&self) -> Option<&OrderBookEntry> {
        self.asks.first()
    }

    /// Get spread (ask - bid)
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask.0 - bid.0),
            _ => None,
        }
    }

    /// Get mid price ((bid + ask) / 2)
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => {
                Some((ask.0 + bid.0) / Decimal::from(2))
            }
            _ => None,
        }
    }
}
