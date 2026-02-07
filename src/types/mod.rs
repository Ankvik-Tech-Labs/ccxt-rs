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
pub mod ledger;
pub mod greeks;
pub mod option;
pub mod open_interest;
pub mod liquidation;
pub mod long_short;
pub mod leverage;
pub mod margin_mod;
pub mod conversion;
pub mod funding_history;
pub mod borrow;
pub mod last_price;
pub mod deposit_withdraw_fee;
pub mod account;
pub mod leverage_tier;

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
pub use ledger::*;
pub use greeks::*;
// Selective re-export from option to avoid conflicts with the `option` keyword
pub use option::{OptionContract, OptionType};
pub use open_interest::*;
pub use liquidation::*;
pub use long_short::*;
pub use leverage::*;
pub use margin_mod::*;
pub use conversion::*;
pub use funding_history::*;
pub use borrow::*;
pub use last_price::*;
pub use deposit_withdraw_fee::*;
pub use account::{Account, AccountType};
pub use leverage_tier::*;
