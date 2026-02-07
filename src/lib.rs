//! # ccxt-rs: Unified Rust API for Cryptocurrency Exchange Trading
//!
//! CCXT-RS provides a unified, type-safe async Rust API for trading across
//! centralized exchanges (CEX) and decentralized exchanges (DEX).
//!
//! ## Features
//!
//! - **Unified API**: Same interface for CEX (Binance, Bybit, OKX) and DEX (Uniswap, PancakeSwap, Hyperliquid)
//! - **Type Safety**: `rust_decimal::Decimal` for all financial calculations (never f64)
//! - **Async/Await**: Built on `tokio` for high-performance concurrent operations
//! - **Rate Limiting**: Built-in per-exchange rate limiting
//! - **Feature Flags**: Compile only the exchanges you need
//!
//! ## Example
//!
//! ```no_run
//! use ccxt::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create a Binance exchange instance
//!     let exchange = ccxt::binance::Binance::builder()
//!         .api_key("your-api-key")
//!         .secret("your-secret")
//!         .build()?;
//!
//!     // Fetch ticker using unified API
//!     let ticker = exchange.fetch_ticker("BTC/USDT").await?;
//!     println!("BTC/USDT: ${}", ticker.last);
//!
//!     Ok(())
//! }
//! ```

pub mod base;
pub mod types;

#[cfg(feature = "binance")]
pub mod binance;

#[cfg(feature = "bybit")]
pub mod bybit;

#[cfg(feature = "okx")]
pub mod okx;

#[cfg(any(feature = "uniswap", feature = "pancakeswap"))]
pub mod dex;

#[cfg(feature = "uniswap")]
pub mod uniswap;

#[cfg(feature = "pancakeswap")]
pub mod pancakeswap;

// #[cfg(feature = "hyperliquid")]
// pub mod hyperliquid;

/// Convenience re-exports for common usage
pub mod prelude {
    pub use crate::base::exchange::Exchange;
    pub use crate::base::errors::{CcxtError, Result};
    pub use crate::types::*;

    #[cfg(feature = "binance")]
    pub use crate::binance;

    #[cfg(feature = "bybit")]
    pub use crate::bybit;

    #[cfg(feature = "okx")]
    pub use crate::okx;

    #[cfg(feature = "uniswap")]
    pub use crate::uniswap::{self, UniswapV3, UniswapV3Builder};

    #[cfg(feature = "pancakeswap")]
    pub use crate::pancakeswap;

    // #[cfg(feature = "hyperliquid")]
    // pub use crate::hyperliquid;
}
