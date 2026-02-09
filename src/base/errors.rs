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

    /// Account is not enabled for the requested operation
    #[error("account not enabled: {0}")]
    AccountNotEnabled(String),

    /// Account has been suspended
    #[error("account suspended: {0}")]
    AccountSuspended(String),

    // === Order Errors ===
    /// Order parameters invalid (price, amount, type mismatch, etc.)
    #[error("invalid order: {0}")]
    InvalidOrder(String),

    /// Order ID not found
    #[error("order not found: {0}")]
    OrderNotFound(String),

    /// Order would be immediately fillable (e.g., post-only rejected)
    #[error("order immediately fillable: {0}")]
    OrderImmediatelyFillable(String),

    /// Order cannot be filled (e.g., price too far from market)
    #[error("order not fillable: {0}")]
    OrderNotFillable(String),

    /// Duplicate order ID
    #[error("duplicate order id: {0}")]
    DuplicateOrderId(String),

    // === Request Errors ===
    /// Required arguments are missing
    #[error("arguments required: {0}")]
    ArgumentsRequired(String),

    /// Malformed request or invalid parameters
    #[error("bad request: {0}")]
    BadRequest(String),

    /// Invalid or unsupported trading symbol
    #[error("bad symbol: {0}")]
    BadSymbol(String),

    /// Invalid address (for deposits/withdrawals)
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// Address is pending generation
    #[error("address pending: {0}")]
    AddressPending(String),

    /// Operation not supported by this exchange
    #[error("not supported: {0}")]
    NotSupported(String),

    /// Operation was rejected by the exchange
    #[error("operation rejected: {0}")]
    OperationRejected(String),

    /// No change resulted from the operation (e.g., setting same value)
    #[error("no change: {0}")]
    NoChange(String),

    /// Margin mode is already set to the requested value
    #[error("margin mode already set: {0}")]
    MarginModeAlreadySet(String),

    /// Market is closed or not trading
    #[error("market closed: {0}")]
    MarketClosed(String),

    /// Contract is unavailable
    #[error("contract unavailable: {0}")]
    ContractUnavailable(String),

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

    /// DDoS protection triggered
    #[error("ddos protection: {0}")]
    DDoSProtection(String),

    /// Exchange service unavailable or under maintenance
    #[error("exchange not available: {0}")]
    ExchangeNotAvailable(String),

    /// Exchange is undergoing maintenance
    #[error("on maintenance: {0}")]
    OnMaintenance(String),

    /// Request timed out
    #[error("request timeout")]
    RequestTimeout,

    // === Response Errors ===
    /// Bad response from exchange (malformed, unexpected format)
    #[error("bad response: {0}")]
    BadResponse(String),

    /// Null or empty response from exchange
    #[error("null response: {0}")]
    NullResponse(String),

    // === WebSocket Errors ===
    /// WebSocket connection error
    #[error("ws connection error: {0}")]
    WsConnectionError(String),

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
    #[cfg(any(feature = "uniswap", feature = "pancakeswap", feature = "hyperliquid"))]
    #[error("blockchain error: {0}")]
    AlloyError(String),
}

/// Result type alias using CcxtError
pub type Result<T> = std::result::Result<T, CcxtError>;

impl CcxtError {
    /// Check if error is retryable (network/timeout/rate limit/maintenance)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CcxtError::NetworkError(_)
                | CcxtError::RequestTimeout
                | CcxtError::ExchangeNotAvailable(_)
                | CcxtError::OnMaintenance(_)
                | CcxtError::DDoSProtection(_)
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
