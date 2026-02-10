//! Local order book manager for WebSocket incremental updates.
//!
//! This module provides `LocalOrderBook`, a structure that maintains a real-time
//! order book state by applying incremental updates from WebSocket streams.
//! It uses `BTreeMap` for efficient price-level storage and retrieval.

use crate::base::signer::timestamp_to_iso8601;
use crate::types::orderbook::OrderBook;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// Local order book that maintains bid/ask price levels.
///
/// Stores bids and asks in `BTreeMap` for O(log n) insertions/deletions
/// and efficient sorted iteration. Bids are retrieved in descending order,
/// asks in ascending order.
#[derive(Debug, Clone)]
pub struct LocalOrderBook {
    /// Trading pair symbol (e.g., "BTC/USDT")
    symbol: String,
    /// Bid price levels: price -> quantity
    bids: BTreeMap<Decimal, Decimal>,
    /// Ask price levels: price -> quantity
    asks: BTreeMap<Decimal, Decimal>,
    /// Sequence number for update ordering (exchange-specific)
    nonce: Option<u64>,
    /// Last update timestamp (milliseconds since epoch)
    timestamp: i64,
}

impl LocalOrderBook {
    /// Creates a new empty order book for the given symbol.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTC/USDT")
    ///
    /// # Returns
    /// A new `LocalOrderBook` with empty bid/ask maps and no nonce.
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            nonce: None,
            timestamp: 0,
        }
    }

    /// Returns the trading pair symbol.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Returns all bids sorted by price descending (highest first).
    ///
    /// # Returns
    /// Vector of (price, quantity) tuples in descending price order.
    pub fn bids(&self) -> Vec<(Decimal, Decimal)> {
        self.bids.iter().rev().map(|(p, q)| (*p, *q)).collect()
    }

    /// Returns all asks sorted by price ascending (lowest first).
    ///
    /// # Returns
    /// Vector of (price, quantity) tuples in ascending price order.
    pub fn asks(&self) -> Vec<(Decimal, Decimal)> {
        self.asks.iter().map(|(p, q)| (*p, *q)).collect()
    }

    /// Returns the current nonce (sequence number) if available.
    pub fn nonce(&self) -> Option<u64> {
        self.nonce
    }

    /// Returns the last update timestamp in milliseconds.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// Resets the order book with a full snapshot.
    ///
    /// Clears all existing bid/ask levels and replaces them with the provided snapshot data.
    /// Only inserts levels with non-zero quantities.
    ///
    /// # Arguments
    /// * `bids` - Bid price levels (price, quantity)
    /// * `asks` - Ask price levels (price, quantity)
    /// * `nonce` - Optional sequence number for this snapshot
    /// * `timestamp` - Snapshot timestamp in milliseconds
    pub fn reset(
        &mut self,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
        nonce: Option<u64>,
        timestamp: i64,
    ) {
        self.bids.clear();
        self.asks.clear();

        for (price, amount) in bids {
            if !amount.is_zero() {
                self.bids.insert(price, amount);
            }
        }

        for (price, amount) in asks {
            if !amount.is_zero() {
                self.asks.insert(price, amount);
            }
        }

        self.nonce = nonce;
        self.timestamp = timestamp;
    }

    /// Updates bid levels with delta changes.
    ///
    /// For each (price, amount) update:
    /// - If amount is zero, removes the price level
    /// - Otherwise, inserts or updates the price level
    ///
    /// # Arguments
    /// * `updates` - Array of (price, amount) tuples to apply
    pub fn update_bids(&mut self, updates: &[(Decimal, Decimal)]) {
        for (price, amount) in updates {
            if amount.is_zero() {
                self.bids.remove(price);
            } else {
                self.bids.insert(*price, *amount);
            }
        }
    }

    /// Updates ask levels with delta changes.
    ///
    /// For each (price, amount) update:
    /// - If amount is zero, removes the price level
    /// - Otherwise, inserts or updates the price level
    ///
    /// # Arguments
    /// * `updates` - Array of (price, amount) tuples to apply
    pub fn update_asks(&mut self, updates: &[(Decimal, Decimal)]) {
        for (price, amount) in updates {
            if amount.is_zero() {
                self.asks.remove(price);
            } else {
                self.asks.insert(*price, *amount);
            }
        }
    }

    /// Sets the sequence number (nonce) for update ordering.
    ///
    /// # Arguments
    /// * `nonce` - The sequence number to set
    pub fn set_nonce(&mut self, nonce: u64) {
        self.nonce = Some(nonce);
    }

    /// Sets the last update timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - Timestamp in milliseconds since epoch
    pub fn set_timestamp(&mut self, timestamp: i64) {
        self.timestamp = timestamp;
    }

    /// Exports the current order book state as an `OrderBook` snapshot.
    ///
    /// # Arguments
    /// * `limit` - Optional limit on number of price levels to include per side
    ///
    /// # Returns
    /// An `OrderBook` with current bids/asks, timestamp, and nonce.
    pub fn to_orderbook(&self, limit: Option<usize>) -> OrderBook {
        let bids = if let Some(n) = limit {
            self.bids.iter().rev().take(n).map(|(p, q)| (*p, *q)).collect()
        } else {
            self.bids.iter().rev().map(|(p, q)| (*p, *q)).collect()
        };

        let asks = if let Some(n) = limit {
            self.asks.iter().take(n).map(|(p, q)| (*p, *q)).collect()
        } else {
            self.asks.iter().map(|(p, q)| (*p, *q)).collect()
        };

        OrderBook {
            symbol: self.symbol.clone(),
            timestamp: self.timestamp,
            datetime: timestamp_to_iso8601(self.timestamp),
            nonce: self.nonce,
            bids,
            asks,
            info: None,
        }
    }

    /// Returns the best bid (highest buy price).
    ///
    /// # Returns
    /// `Some((price, quantity))` if bids exist, `None` otherwise.
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    /// Returns the best ask (lowest sell price).
    ///
    /// # Returns
    /// `Some((price, quantity))` if asks exist, `None` otherwise.
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    /// Calculates the spread (best_ask - best_bid).
    ///
    /// # Returns
    /// `Some(spread)` if both best bid and best ask exist, `None` otherwise.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask_price, _)), Some((bid_price, _))) => Some(ask_price - bid_price),
            _ => None,
        }
    }

    /// Calculates the mid price ((best_ask + best_bid) / 2).
    ///
    /// # Returns
    /// `Some(mid_price)` if both best bid and best ask exist, `None` otherwise.
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask_price, _)), Some((bid_price, _))) => {
                Some((ask_price + bid_price) / Decimal::from(2))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_orderbook() {
        let ob = LocalOrderBook::new("BTC/USDT".to_string());
        assert_eq!(ob.symbol(), "BTC/USDT");
        assert!(ob.bids().is_empty());
        assert!(ob.asks().is_empty());
        assert_eq!(ob.nonce(), None);
    }

    #[test]
    fn test_reset_from_snapshot() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![(Decimal::new(50000, 0), Decimal::new(1, 0))],
            vec![(Decimal::new(50100, 0), Decimal::new(2, 0))],
            Some(100),
            1234567890,
        );
        assert_eq!(ob.bids(), vec![(Decimal::new(50000, 0), Decimal::new(1, 0))]);
        assert_eq!(ob.asks(), vec![(Decimal::new(50100, 0), Decimal::new(2, 0))]);
        assert_eq!(ob.nonce(), Some(100));
        assert_eq!(ob.timestamp(), 1234567890);
    }

    #[test]
    fn test_apply_delta_update_level() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![(Decimal::new(50000, 0), Decimal::new(1, 0))],
            vec![],
            None,
            0,
        );
        ob.update_bids(&[(Decimal::new(50000, 0), Decimal::new(5, 0))]);
        assert_eq!(ob.bids(), vec![(Decimal::new(50000, 0), Decimal::new(5, 0))]);
    }

    #[test]
    fn test_apply_delta_add_level() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(vec![], vec![], None, 0);
        ob.update_asks(&[(Decimal::new(51000, 0), Decimal::new(3, 0))]);
        assert_eq!(ob.asks(), vec![(Decimal::new(51000, 0), Decimal::new(3, 0))]);
    }

    #[test]
    fn test_apply_delta_remove_level() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![(Decimal::new(50000, 0), Decimal::new(1, 0))],
            vec![],
            None,
            0,
        );
        ob.update_bids(&[(Decimal::new(50000, 0), Decimal::ZERO)]);
        assert!(ob.bids().is_empty());
    }

    #[test]
    fn test_set_nonce() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.set_nonce(42);
        assert_eq!(ob.nonce(), Some(42));
    }

    #[test]
    fn test_to_orderbook_snapshot() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![
                (Decimal::new(50000, 0), Decimal::new(1, 0)),
                (Decimal::new(49900, 0), Decimal::new(2, 0)),
            ],
            vec![
                (Decimal::new(50100, 0), Decimal::new(3, 0)),
                (Decimal::new(50200, 0), Decimal::new(4, 0)),
            ],
            Some(100),
            1704067200000,
        );

        let snapshot = ob.to_orderbook(None);
        assert_eq!(snapshot.symbol, "BTC/USDT");
        assert_eq!(snapshot.timestamp, 1704067200000);
        assert_eq!(snapshot.nonce, Some(100));
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);
        // Bids should be descending
        assert_eq!(snapshot.bids[0].0, Decimal::new(50000, 0));
        assert_eq!(snapshot.bids[1].0, Decimal::new(49900, 0));
        // Asks should be ascending
        assert_eq!(snapshot.asks[0].0, Decimal::new(50100, 0));
        assert_eq!(snapshot.asks[1].0, Decimal::new(50200, 0));
    }

    #[test]
    fn test_to_orderbook_with_limit() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![
                (Decimal::new(50000, 0), Decimal::new(1, 0)),
                (Decimal::new(49900, 0), Decimal::new(2, 0)),
                (Decimal::new(49800, 0), Decimal::new(3, 0)),
            ],
            vec![
                (Decimal::new(50100, 0), Decimal::new(4, 0)),
                (Decimal::new(50200, 0), Decimal::new(5, 0)),
                (Decimal::new(50300, 0), Decimal::new(6, 0)),
            ],
            None,
            1704067200000,
        );

        let snapshot = ob.to_orderbook(Some(2));
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);
        assert_eq!(snapshot.bids[0].0, Decimal::new(50000, 0));
        assert_eq!(snapshot.bids[1].0, Decimal::new(49900, 0));
        assert_eq!(snapshot.asks[0].0, Decimal::new(50100, 0));
        assert_eq!(snapshot.asks[1].0, Decimal::new(50200, 0));
    }

    #[test]
    fn test_best_bid_ask() {
        let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
        ob.reset(
            vec![
                (Decimal::new(50000, 0), Decimal::new(1, 0)),
                (Decimal::new(49900, 0), Decimal::new(2, 0)),
            ],
            vec![
                (Decimal::new(50100, 0), Decimal::new(3, 0)),
                (Decimal::new(50200, 0), Decimal::new(4, 0)),
            ],
            None,
            0,
        );

        let best_bid = ob.best_bid().unwrap();
        assert_eq!(best_bid.0, Decimal::new(50000, 0));
        assert_eq!(best_bid.1, Decimal::new(1, 0));

        let best_ask = ob.best_ask().unwrap();
        assert_eq!(best_ask.0, Decimal::new(50100, 0));
        assert_eq!(best_ask.1, Decimal::new(3, 0));

        let spread = ob.spread().unwrap();
        assert_eq!(spread, Decimal::new(100, 0));

        let mid_price = ob.mid_price().unwrap();
        assert_eq!(mid_price, Decimal::new(50050, 0));
    }

    #[test]
    fn test_best_bid_ask_empty() {
        let ob = LocalOrderBook::new("BTC/USDT".to_string());
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
        assert!(ob.spread().is_none());
        assert!(ob.mid_price().is_none());
    }
}
