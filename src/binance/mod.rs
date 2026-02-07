//! Binance exchange implementation
//!
//! This module implements the Exchange trait for Binance, providing access to:
//! - Spot markets
//! - Futures markets
//! - Margin trading
//!
//! # Example
//!
//! ```no_run
//! use ccxt::binance::Binance;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let binance = Binance::builder()
//!         .api_key("your-api-key")
//!         .secret("your-secret")
//!         .sandbox(true)
//!         .build()?;
//!
//!     let ticker = binance.fetch_ticker("BTC/USDT").await?;
//!     println!("BTC/USDT: ${}", ticker.last.unwrap());
//!
//!     Ok(())
//! }
//! ```

pub mod types;
pub mod parsers;

use crate::base::{
    errors::{CcxtError, Result},
    exchange::{Exchange, ExchangeFeatures, ExchangeType, Params},
    http_client::HttpClient,
    rate_limiter::RateLimiter,
    signer::{hmac_sha256, timestamp_ms},
};
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub use parsers::*;
pub use types::*;

/// Binance exchange client
pub struct Binance {
    /// API credentials
    api_key: Option<String>,
    secret: Option<String>,

    /// HTTP client
    http_client: HttpClient,

    /// Base URL
    base_url: String,

    /// Cached markets
    markets: Arc<tokio::sync::RwLock<Option<Vec<Market>>>>,

    /// Exchange features
    features: ExchangeFeatures,
}

/// Builder for Binance exchange
pub struct BinanceBuilder {
    api_key: Option<String>,
    secret: Option<String>,
    sandbox: bool,
    rate_limit: bool,
    timeout: Duration,
}

impl BinanceBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            api_key: None,
            secret: None,
            sandbox: false,
            rate_limit: true,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set API key
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set API secret
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Enable sandbox/testnet mode
    pub fn sandbox(mut self, enabled: bool) -> Self {
        self.sandbox = enabled;
        self
    }

    /// Enable rate limiting (default: true)
    pub fn rate_limit(mut self, enabled: bool) -> Self {
        self.rate_limit = enabled;
        self
    }

    /// Set request timeout (default: 30s)
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the Binance client
    pub fn build(self) -> Result<Binance> {
        let base_url = if self.sandbox {
            "https://testnet.binance.vision".to_string()
        } else {
            "https://api.binance.com".to_string()
        };

        let rate_limiter = if self.rate_limit {
            // Binance: 1200 requests per minute = 20 requests per second
            Some(Arc::new(RateLimiter::new(20)))
        } else {
            None
        };

        let http_client = HttpClient::new(rate_limiter, self.timeout)?;

        Ok(Binance {
            api_key: self.api_key,
            secret: self.secret,
            http_client,
            base_url,
            markets: Arc::new(tokio::sync::RwLock::new(None)),
            features: ExchangeFeatures {
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_trades: true,
                fetch_markets: true,
                fetch_currencies: false,
                fetch_status: false,
                create_order: true,
                create_market_order: true,
                create_limit_order: true,
                cancel_order: true,
                cancel_all_orders: true,
                edit_order: false,
                fetch_order: true,
                fetch_orders: true,
                fetch_open_orders: true,
                fetch_closed_orders: false,
                fetch_my_trades: true,
                fetch_balance: true,
                fetch_deposit_address: false,
                fetch_deposits: false,
                fetch_withdrawals: false,
                withdraw: false,
                transfer: false,
                fetch_positions: true,
                fetch_position: false,
                fetch_funding_rate: true,
                fetch_funding_rates: false,
                fetch_funding_rate_history: false,
                set_leverage: true,
                set_margin_mode: true,
                add_margin: false,
                reduce_margin: false,
                margin_trading: true,
                futures_trading: true,
                options_trading: false,
                swap_trading: true,
            },
        })
    }
}

impl Default for BinanceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Binance {
    /// Create a new builder
    pub fn builder() -> BinanceBuilder {
        BinanceBuilder::new()
    }

    /// Sign a request (for private endpoints)
    fn sign_request(&self, query_string: &str) -> Result<String> {
        let secret = self
            .secret
            .as_ref()
            .ok_or_else(|| CcxtError::AuthenticationError("Secret not configured".to_string()))?;

        hmac_sha256(secret, query_string)
    }

    /// Build query string with signature
    fn build_signed_query(&self, mut params: HashMap<String, String>) -> Result<String> {
        // Add timestamp
        params.insert("timestamp".to_string(), timestamp_ms().to_string());

        // Build query string
        let mut query_parts: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();
        query_parts.sort();
        let query_string = query_parts.join("&");

        // Sign
        let signature = self.sign_request(&query_string)?;

        // Append signature
        Ok(format!("{}&signature={}", query_string, signature))
    }

    /// Check response for Binance API errors
    fn check_response(&self, json: &serde_json::Value) -> Result<()> {
        // Binance uses negative error codes for failures
        // Positive codes (like 200) are success responses on some endpoints
        if let Some(code) = json.get("code").and_then(|v| v.as_i64()) {
            if code < 0 {
                let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                return Err(self.map_error_code(code, msg));
            }
        }
        Ok(())
    }

    /// Make a public GET request
    async fn public_get(&self, path: &str, params: Option<&HashMap<String, String>>) -> Result<serde_json::Value> {
        let mut url = format!("{}{}", self.base_url, path);

        if let Some(params) = params {
            let query: Vec<String> = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                .collect();
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query.join("&"));
            }
        }

        let response = self.http_client.get(&url, None).await?;
        let json = response.json::<serde_json::Value>().await?;

        // Check for Binance error
        self.check_response(&json)?;

        Ok(json)
    }

    /// Make a private GET request (requires authentication)
    async fn private_get(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| CcxtError::AuthenticationError("API key not configured".to_string()))?;

        let params = params.unwrap_or_default();
        let query_string = self.build_signed_query(params)?;
        let url = format!("{}{}?{}", self.base_url, path, query_string);

        let mut headers = HashMap::new();
        headers.insert("X-MBX-APIKEY".to_string(), api_key.clone());

        let response = self.http_client.get(&url, Some(headers)).await?;
        let json = response.json::<serde_json::Value>().await?;

        // Check for error
        self.check_response(&json)?;

        Ok(json)
    }

    /// Map Binance error code to CcxtError
    fn map_error_code(&self, code: i64, msg: &str) -> CcxtError {
        match code {
            // Exchange availability errors
            -1000 => CcxtError::ExchangeNotAvailable(format!("Invalid request: {}", msg)),
            -1001 => CcxtError::ExchangeNotAvailable(format!("Disconnected: {}", msg)),

            // Authentication errors
            -1002 => CcxtError::AuthenticationError(format!("Unauthorized: {}", msg)),
            -1022 => CcxtError::AuthenticationError(format!("Invalid signature: {}", msg)),
            -3001 => CcxtError::AuthenticationError(format!("Invalid account: {}", msg)),

            // Rate limiting
            -1003 => CcxtError::RateLimitExceeded(msg.to_string()),

            // Nonce errors
            -1021 => CcxtError::InvalidNonce(format!("Timestamp outside recv window: {}", msg)),

            // Request validation errors
            -1100 => CcxtError::BadRequest(format!("Illegal characters: {}", msg)),
            -1101 => CcxtError::BadRequest(format!("Too many parameters: {}", msg)),
            -1102 => CcxtError::BadRequest(format!("Mandatory parameter missing: {}", msg)),
            -1104 => CcxtError::BadRequest(format!("Not all sent parameters were read: {}", msg)),
            -1105 => CcxtError::BadRequest(format!("Parameter must be sent in body: {}", msg)),
            -1106 => CcxtError::BadRequest(format!("Parameter must be sent as query string: {}", msg)),

            // Symbol errors
            -1121 => CcxtError::BadSymbol(format!("Invalid symbol: {}", msg)),

            // Order errors
            -1004 => CcxtError::InvalidOrder(format!("Duplicate order ID: {}", msg)),
            -1005 => CcxtError::OrderNotFound(format!("Order does not exist: {}", msg)),
            -1110 => CcxtError::InvalidOrder(format!("Invalid time in force: {}", msg)),
            -1112 => CcxtError::OrderNotFound(format!("Order does not exist: {}", msg)),
            -1114 => CcxtError::InvalidOrder(format!("Time in force must be GTC: {}", msg)),
            -2011 => CcxtError::OrderNotFound(msg.to_string()),
            -2013 => CcxtError::InvalidOrder(format!("Order type not supported: {}", msg)),

            // Account/permission errors
            -2010 => CcxtError::InsufficientFunds(msg.to_string()),
            -2014 => CcxtError::PermissionDenied(format!("Account disabled for quoting: {}", msg)),
            -2015 => CcxtError::InsufficientFunds(format!("Account trading limit exceeded: {}", msg)),
            -2016 => CcxtError::PermissionDenied(format!("User in liquidation mode: {}", msg)),
            -2017 => CcxtError::PermissionDenied(format!("Account pending delisting: {}", msg)),
            -3024 => CcxtError::InsufficientFunds(format!("Insufficient balance: {}", msg)),

            // Fallback for unmapped errors
            _ => CcxtError::BadRequest(format!("Binance error {}: {}", code, msg)),
        }
    }
}

#[async_trait]
impl Exchange for Binance {
    fn id(&self) -> &str {
        "binance"
    }

    fn name(&self) -> &str {
        "Binance"
    }

    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Cex
    }

    fn has(&self) -> &ExchangeFeatures {
        &self.features
    }

    async fn load_markets(&self) -> Result<Vec<Market>> {
        // Check cache first
        {
            let markets_guard = self.markets.read().await;
            if let Some(markets) = markets_guard.as_ref() {
                return Ok(markets.clone());
            }
        }

        // Fetch from API
        let markets = self.fetch_markets().await?;

        // Cache
        {
            let mut markets_guard = self.markets.write().await;
            *markets_guard = Some(markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        let json = self.public_get("/api/v3/exchangeInfo", None).await?;

        let symbols = json
            .get("symbols")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing symbols in exchangeInfo".to_string()))?;

        let mut markets = Vec::with_capacity(symbols.len());
        for symbol_json in symbols {
            match parsers::parse_market(symbol_json) {
                Ok(market) => markets.push(market),
                Err(e) => {
                    tracing::debug!("Failed to parse market: {}", e);
                    continue;
                }
            }
        }

        Ok(markets)
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(CcxtError::NotSupported("fetch_currencies not implemented for Binance".to_string()))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        let json = self.public_get("/api/v3/ticker/24hr", Some(&params)).await?;
        parsers::parse_ticker(&json, symbol)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        let json = self.public_get("/api/v3/ticker/24hr", None).await?;

        let ticker_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of tickers".to_string()))?;

        let mut tickers = Vec::with_capacity(ticker_array.len());
        for ticker_json in ticker_array {
            // Skip malformed entries (missing symbol) instead of failing the entire call
            let binance_symbol = match ticker_json.get("symbol").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    tracing::warn!("Skipping ticker entry with missing symbol");
                    continue;
                }
            };

            // Convert to unified symbol (e.g., "BTCUSDT" -> "BTC/USDT")
            let symbol = parsers::symbol_from_binance(binance_symbol);

            // Filter if symbols specified
            if let Some(filter_symbols) = symbols {
                if !filter_symbols.contains(&symbol.as_str()) {
                    continue;
                }
            }

            // Skip entries that fail to parse instead of failing the entire call
            match parsers::parse_ticker(ticker_json, &symbol) {
                Ok(ticker) => tickers.push(ticker),
                Err(e) => {
                    tracing::warn!("Failed to parse ticker for {}: {}", symbol, e);
                    continue;
                }
            }
        }

        Ok(tickers)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.public_get("/api/v3/depth", Some(&params)).await?;
        parsers::parse_order_book(&json, symbol)
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let interval = parsers::timeframe_to_binance(timeframe);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);
        params.insert("interval".to_string(), interval.to_string());

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }

        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.public_get("/api/v3/klines", Some(&params)).await?;

        let klines = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of klines".to_string()))?;

        let mut ohlcv_list = Vec::with_capacity(klines.len());
        for kline in klines {
            match parsers::parse_ohlcv(kline) {
                Ok(ohlcv) => ohlcv_list.push(ohlcv),
                Err(e) => {
                    tracing::debug!("Failed to parse OHLCV candle: {}", e);
                    continue;
                }
            }
        }

        Ok(ohlcv_list)
    }

    async fn fetch_trades(&self, symbol: &str, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Trade>> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        // NOTE: The /api/v3/trades endpoint does not support time-based filtering (startTime).
        // For historical trades with time filtering, use /api/v3/aggTrades or /api/v3/historicalTrades.
        // Currently we fetch recent trades and filter client-side.

        let json = self.public_get("/api/v3/trades", Some(&params)).await?;

        let trades_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of trades".to_string()))?;

        let mut trades = Vec::with_capacity(trades_array.len());
        for trade_json in trades_array {
            if let Ok(trade) = parsers::parse_trade(trade_json, symbol) {
                // Client-side filter by since timestamp if specified
                // Note: This only filters the recent trades returned by the API
                if let Some(since_ts) = since {
                    if trade.timestamp < since_ts {
                        continue;
                    }
                }
                trades.push(trade);
            }
        }

        Ok(trades)
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        Err(CcxtError::NotSupported("fetch_status not implemented for Binance".to_string()))
    }

    // Private methods (require authentication) - stubs for now
    async fn create_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_order will be implemented next".to_string()))
    }

    async fn cancel_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("cancel_order will be implemented next".to_string()))
    }

    async fn edit_order(
        &self,
        _id: &str,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Option<Decimal>,
        _price: Option<Decimal>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("edit_order not supported by Binance".to_string()))
    }

    async fn fetch_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("fetch_order will be implemented next".to_string()))
    }

    async fn fetch_orders(&self, _symbol: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_orders will be implemented next".to_string()))
    }

    async fn fetch_open_orders(&self, _symbol: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_open_orders will be implemented next".to_string()))
    }

    async fn fetch_closed_orders(&self, _symbol: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_closed_orders not supported by Binance API v3".to_string()))
    }

    async fn fetch_my_trades(&self, _symbol: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Trade>> {
        Err(CcxtError::NotSupported("fetch_my_trades will be implemented next".to_string()))
    }

    async fn fetch_balance(&self) -> Result<Balances> {
        Err(CcxtError::NotSupported("fetch_balance will be implemented next".to_string()))
    }

    async fn fetch_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported("fetch_deposit_address not implemented".to_string()))
    }

    async fn fetch_deposits(&self, _code: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Deposit>> {
        Err(CcxtError::NotSupported("fetch_deposits not implemented".to_string()))
    }

    async fn fetch_withdrawals(&self, _code: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Withdrawal>> {
        Err(CcxtError::NotSupported("fetch_withdrawals not implemented".to_string()))
    }

    async fn withdraw(&self, _code: &str, _amount: Decimal, _address: &str, _tag: Option<&str>) -> Result<Withdrawal> {
        Err(CcxtError::NotSupported("withdraw not implemented".to_string()))
    }

    async fn transfer(&self, _code: &str, _amount: Decimal, _from_account: &str, _to_account: &str) -> Result<Transfer> {
        Err(CcxtError::NotSupported("transfer not implemented".to_string()))
    }

    async fn fetch_positions(&self, _symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        Err(CcxtError::NotSupported("fetch_positions will be implemented next".to_string()))
    }

    async fn fetch_funding_rate(&self, _symbol: &str) -> Result<FundingRate> {
        Err(CcxtError::NotSupported("fetch_funding_rate will be implemented next".to_string()))
    }

    async fn set_leverage(&self, _leverage: u32, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_leverage will be implemented next".to_string()))
    }

    async fn set_margin_mode(&self, _mode: MarginMode, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_margin_mode will be implemented next".to_string()))
    }
}
