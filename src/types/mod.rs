//! Unified data structures for all exchanges
//!
//! These types represent normalized trading data that works across all exchanges.

pub mod common;
pub mod ticker;
pub mod orderbook;
pub mod ohlcv;
pub mod trade;
pub mod order;
pub mod balance;
pub mod market;
pub mod currency;
pub mod position;
pub mod funding;
pub mod deposit;
pub mod transfer;
pub mod fee;

// Re-export all types for convenience
pub use common::*;
pub use ticker::*;
pub use orderbook::*;
pub use ohlcv::*;
pub use trade::*;
pub use order::*;
pub use balance::*;
pub use market::*;
// Avoid re-exporting MinMax from currency (already exported from market)
pub use currency::{Currency, CurrencyLimits, Network};
pub use position::*;
pub use funding::*;
pub use deposit::*;
pub use transfer::*;
// Avoid re-exporting TransactionFee from fee (already exported from deposit)
pub use fee::TradingFees;
