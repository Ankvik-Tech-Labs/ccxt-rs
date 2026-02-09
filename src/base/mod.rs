//! Core infrastructure for all exchanges
//!
//! This module contains the fundamental building blocks used by all exchange implementations:
//! - Exchange trait
//! - Error types
//! - HTTP client with rate limiting
//! - Authentication/signing utilities
//! - Decimal formatting helpers

pub mod errors;
pub mod exchange;
pub mod http_client;
pub mod signer;
pub mod rate_limiter;
pub mod decimal;
pub mod precise;
pub mod ws;
pub mod ws_connection;

pub use errors::{CcxtError, Result};
pub use exchange::{Exchange, ExchangeFeatures, ExchangeType};
