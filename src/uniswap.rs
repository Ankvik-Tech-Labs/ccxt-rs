//! Uniswap V3 DEX exchange implementation
//!
//! This module provides a unified API for interacting with Uniswap V3 pools
//! across multiple chains (Ethereum, Polygon, Arbitrum).

pub mod constants;
pub mod parsers;
pub mod pools;
pub mod swap;
mod exchange;

pub use exchange::{UniswapV3, UniswapV3Builder};
