//! Error types for CCXT-RS
//!
//! This module defines the error hierarchy matching CCXT's Python/JS error classes.

use thiserror::Error;

/// Main error type for all CCXT operations
#[derive(Debug, Error)]
pub enum CcxtError {
    // === Authentication Errors ===
    /// Authentication credentials invalid or missing
    #[error("authentication error: {0}")]
    AuthenticationError(String),

    /// API key lacks required permissions
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid nonce (timestamp too old or reused)
    #[error("invalid nonce: {0}")]
    InvalidNonce(String),

    // === Account Errors ===
    /// Account has insufficient funds for operation
    #[error("insufficient funds: {0}")]
    InsufficientFunds(String),

    // === Order Errors ===
    /// Order parameters invalid (price, amount, type mismatch, etc.)
    #[error("invalid order: {0}")]
    InvalidOrder(String),

    /// Order ID not found
    #[error("order not found: {0}")]
    OrderNotFound(String),

    // === Request Errors ===
    /// Malformed request or invalid parameters
    #[error("bad request: {0}")]
    BadRequest(String),

    /// Invalid or unsupported trading symbol
    #[error("bad symbol: {0}")]
    BadSymbol(String),

    /// Operation not supported by this exchange
    #[error("not supported: {0}")]
    NotSupported(String),

    /// Generic exchange error (for exchange-specific error codes)
    #[error("exchange error: {0}")]
    ExchangeError(String),

    // === Network/Service Errors ===
    /// Network connectivity issue
    #[error("network error: {0}")]
    NetworkError(String),

    /// Rate limit exceeded (too many requests)
    #[error("rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Exchange service unavailable or under maintenance
    #[error("exchange not available: {0}")]
    ExchangeNotAvailable(String),

    /// Request timed out
    #[error("request timeout")]
    RequestTimeout,

    // === Internal Errors ===
    /// Failed to parse exchange response
    #[error("parse error: {0}")]
    ParseError(String),

    /// Configuration error (invalid settings)
    #[error("config error: {0}")]
    ConfigError(String),

    // === Passthrough Errors ===
    /// HTTP client error
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    /// Decimal parsing error
    #[error(transparent)]
    DecimalError(#[from] rust_decimal::Error),

    /// Alloy transport error (DEX only)
    #[cfg(any(feature = "uniswap", feature = "pancakeswap"))]
    #[error("blockchain error: {0}")]
    AlloyError(String),
}

/// Result type alias using CcxtError
pub type Result<T> = std::result::Result<T, CcxtError>;

impl CcxtError {
    /// Check if error is retryable (network/timeout/rate limit)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CcxtError::NetworkError(_)
                | CcxtError::RequestTimeout
                | CcxtError::ExchangeNotAvailable(_)
        )
    }

    /// Check if error is due to rate limiting
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, CcxtError::RateLimitExceeded(_))
    }

    /// Check if error is authentication-related
    pub fn is_auth_error(&self) -> bool {
        matches!(
            self,
            CcxtError::AuthenticationError(_)
                | CcxtError::PermissionDenied(_)
                | CcxtError::InvalidNonce(_)
        )
    }
}
