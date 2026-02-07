//! Shared DEX infrastructure (for Uniswap, PancakeSwap)
//!
//! This module provides common utilities for DEX exchanges using alloy-rs
//! for EVM blockchain interactions.

#![cfg(any(feature = "uniswap", feature = "pancakeswap"))]

pub mod provider;
pub mod wallet;
pub mod erc20;

#[cfg(any(feature = "uniswap", feature = "pancakeswap"))]
pub mod subgraph;

pub use provider::*;
pub use wallet::*;
pub use erc20::*;

#[cfg(any(feature = "uniswap", feature = "pancakeswap"))]
pub use subgraph::*;
