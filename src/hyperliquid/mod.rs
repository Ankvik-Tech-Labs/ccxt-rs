//! Hyperliquid perpetual DEX exchange implementation
//!
//! Hyperliquid is a high-performance perpetual futures DEX with a central limit
//! order book (CLOB) architecture. All perps are USDC-margined linear contracts.
//!
//! # Example
//!
//! ```no_run
//! use ccxt::hyperliquid::Hyperliquid;
//! use ccxt::base::exchange::Exchange;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let exchange = Hyperliquid::builder()
//!         .sandbox(true) // use testnet
//!         .build()?;
//!
//!     let ticker = exchange.fetch_ticker("BTC/USD:USDC").await?;
//!     println!("BTC price: ${}", ticker.last.unwrap());
//!
//!     Ok(())
//! }
//! ```

pub mod constants;
pub mod types;
pub mod client;
pub mod signer;
pub mod parsers;
pub mod ws;
mod exchange;

pub use exchange::{Hyperliquid, HyperliquidBuilder};
