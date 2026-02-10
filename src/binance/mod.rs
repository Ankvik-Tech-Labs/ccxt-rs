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
pub mod ws;

use crate::base::{
    errors::{CcxtError, Result},
    exchange::{Exchange, ExchangeFeatures, ExchangeType, Params},
    http_client::HttpClient,
    market_cache::MarketCache,
    rate_limiter::RateLimiter,
    signer::{hmac_sha256, timestamp_ms, timestamp_to_iso8601},
};
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
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

    /// Base URL (spot)
    base_url: String,

    /// Futures API base URL
    fapi_url: String,

    /// Market cache with TTL
    market_cache: Arc<tokio::sync::RwLock<MarketCache>>,

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
    market_cache_ttl: Duration,
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
            market_cache_ttl: Duration::from_secs(3600), // Default: 1 hour
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

    /// Set market cache TTL (default: 1 hour)
    ///
    /// Use `Duration::ZERO` to disable caching.
    pub fn market_cache_ttl(mut self, ttl: Duration) -> Self {
        self.market_cache_ttl = ttl;
        self
    }

    /// Build the Binance client
    pub fn build(self) -> Result<Binance> {
        let base_url = if self.sandbox {
            "https://testnet.binance.vision".to_string()
        } else {
            "https://api.binance.com".to_string()
        };

        let fapi_url = if self.sandbox {
            "https://testnet.binancefuture.com".to_string()
        } else {
            "https://fapi.binance.com".to_string()
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
            fapi_url,
            market_cache: Arc::new(tokio::sync::RwLock::new(MarketCache::new(self.market_cache_ttl))),
            features: ExchangeFeatures {
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_trades: true,
                fetch_markets: true,
                fetch_currencies: true,
                fetch_status: true,
                fetch_time: true,
                create_order: true,
                create_market_order: true,
                create_limit_order: true,
                cancel_order: true,
                cancel_all_orders: true,
                fetch_order: true,
                fetch_orders: true,
                fetch_open_orders: true,
                fetch_closed_orders: true,
                fetch_canceled_orders: true,
                fetch_my_trades: true,
                fetch_balance: true,
                fetch_positions: true,
                fetch_funding_rate: true,
                fetch_funding_rate_history: true,
                set_leverage: true,
                set_margin_mode: true,
                set_position_mode: true,
                fetch_deposit_address: true,
                fetch_deposits: true,
                fetch_withdrawals: true,
                withdraw: true,
                transfer: true,
                fetch_trading_fee: true,
                fetch_trading_fees: true,
                fetch_open_interest: true,
                fetch_leverage_tiers: true,
                fetch_ledger: true,
                create_stop_order: true,
                create_stop_limit_order: true,
                create_stop_market_order: true,
                create_stop_loss_order: true,
                create_take_profit_order: true,
                create_trigger_order: true,
                create_post_only_order: true,
                create_reduce_only_order: true,
                margin_trading: true,
                futures_trading: true,
                swap_trading: true,
                sandbox: true,
                ..Default::default()
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

    /// Check if a unified symbol is a futures symbol (contains ':')
    fn is_futures_symbol(symbol: &str) -> bool {
        symbol.contains(':')
    }

    /// Sign a request (for private endpoints)
    fn sign_request(&self, query_string: &str) -> Result<String> {
        let secret = self
            .secret
            .as_ref()
            .ok_or_else(|| CcxtError::AuthenticationError("Secret not configured".to_string()))?;

        hmac_sha256(secret, query_string)
    }

    /// Get API key or return auth error
    fn require_api_key(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .ok_or_else(|| CcxtError::AuthenticationError("API key not configured".to_string()))
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

    /// Build auth headers
    fn auth_headers(&self) -> Result<HashMap<String, String>> {
        let api_key = self.require_api_key()?;
        let mut headers = HashMap::new();
        headers.insert("X-MBX-APIKEY".to_string(), api_key.to_string());
        Ok(headers)
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

    /// Build a query string from params
    fn build_query_string(params: &HashMap<String, String>) -> String {
        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();
        query.join("&")
    }

    // ========================================================================
    // HTTP Methods - Spot
    // ========================================================================

    /// Make a public GET request (spot)
    async fn public_get(&self, path: &str, params: Option<&HashMap<String, String>>) -> Result<serde_json::Value> {
        let mut url = format!("{}{}", self.base_url, path);

        if let Some(params) = params {
            let query = Self::build_query_string(params);
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }
        }

        let response = self.http_client.get(&url, None).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private GET request (spot, requires authentication)
    async fn private_get(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let headers = self.auth_headers()?;
        let params = params.unwrap_or_default();
        let query_string = self.build_signed_query(params)?;
        let url = format!("{}{}?{}", self.base_url, path, query_string);

        let response = self.http_client.get(&url, Some(headers)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private POST request (spot, form-encoded body)
    async fn private_post(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let mut headers = self.auth_headers()?;
        headers.insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());

        let params = params.unwrap_or_default();
        let body = self.build_signed_query(params)?;
        let url = format!("{}{}", self.base_url, path);

        let response = self.http_client.post(&url, Some(headers), Some(body)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private DELETE request (spot, query string)
    async fn private_delete(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let headers = self.auth_headers()?;
        let params = params.unwrap_or_default();
        let query_string = self.build_signed_query(params)?;
        let url = format!("{}{}?{}", self.base_url, path, query_string);

        let response = self.http_client.delete(&url, Some(headers)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    // ========================================================================
    // HTTP Methods - Futures (fapi)
    // ========================================================================

    /// Make a public GET request against fapi
    async fn public_get_fapi(&self, path: &str, params: Option<&HashMap<String, String>>) -> Result<serde_json::Value> {
        let mut url = format!("{}{}", self.fapi_url, path);

        if let Some(params) = params {
            let query = Self::build_query_string(params);
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }
        }

        let response = self.http_client.get(&url, None).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private GET request against fapi
    async fn private_get_fapi(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let headers = self.auth_headers()?;
        let params = params.unwrap_or_default();
        let query_string = self.build_signed_query(params)?;
        let url = format!("{}{}?{}", self.fapi_url, path, query_string);

        let response = self.http_client.get(&url, Some(headers)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private POST request against fapi
    async fn private_post_fapi(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let mut headers = self.auth_headers()?;
        headers.insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());

        let params = params.unwrap_or_default();
        let body = self.build_signed_query(params)?;
        let url = format!("{}{}", self.fapi_url, path);

        let response = self.http_client.post(&url, Some(headers), Some(body)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private DELETE request against fapi
    async fn private_delete_fapi(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<serde_json::Value> {
        let headers = self.auth_headers()?;
        let params = params.unwrap_or_default();
        let query_string = self.build_signed_query(params)?;
        let url = format!("{}{}?{}", self.fapi_url, path, query_string);

        let response = self.http_client.delete(&url, Some(headers)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

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

            // Margin mode already set
            -4046 => CcxtError::BadRequest(format!("Margin mode already set: {}", msg)),

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

    // ========================================================================
    // Market Data (Public)
    // ========================================================================

    async fn load_markets(&self) -> Result<Vec<Market>> {
        self.fetch_markets().await
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        // Check cache first
        {
            let cache = self.market_cache.read().await;
            if let Some(markets) = cache.get("binance") {
                return Ok(markets);
            }
        }

        // Fetch spot markets
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

        // Fetch futures markets
        match self.public_get_fapi("/fapi/v1/exchangeInfo", None).await {
            Ok(fapi_json) => {
                if let Some(fapi_symbols) = fapi_json.get("symbols").and_then(|v| v.as_array()) {
                    for symbol_json in fapi_symbols {
                        match parsers::parse_futures_market(symbol_json) {
                            Ok(market) => markets.push(market),
                            Err(e) => {
                                tracing::debug!("Failed to parse futures market: {}", e);
                                continue;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch futures markets: {}", e);
            }
        }

        // Cache the result
        {
            let mut cache = self.market_cache.write().await;
            cache.insert("binance".to_string(), markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        let json = self.private_get("/sapi/v1/capital/config/getall", None).await?;

        let coins = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of currencies".to_string()))?;

        let mut currencies = Vec::with_capacity(coins.len());
        for coin_json in coins {
            match parsers::parse_currency(coin_json) {
                Ok(currency) => currencies.push(currency),
                Err(e) => {
                    tracing::debug!("Failed to parse currency: {}", e);
                    continue;
                }
            }
        }

        Ok(currencies)
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if Self::is_futures_symbol(symbol) {
            let json = self.public_get_fapi("/fapi/v1/ticker/24hr", Some(&params)).await?;
            parsers::parse_ticker(&json, symbol)
        } else {
            let json = self.public_get("/api/v3/ticker/24hr", Some(&params)).await?;
            parsers::parse_ticker(&json, symbol)
        }
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        let json = self.public_get("/api/v3/ticker/24hr", None).await?;

        let ticker_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of tickers".to_string()))?;

        let mut tickers = Vec::with_capacity(ticker_array.len());
        for ticker_json in ticker_array {
            let binance_symbol = match ticker_json.get("symbol").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };

            let symbol = parsers::symbol_from_binance(binance_symbol);

            if let Some(filter_symbols) = symbols {
                if !filter_symbols.contains(&symbol.as_str()) {
                    continue;
                }
            }

            match parsers::parse_ticker(ticker_json, &symbol) {
                Ok(ticker) => tickers.push(ticker),
                Err(e) => {
                    tracing::debug!("Failed to parse ticker for {}: {}", symbol, e);
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

        if Self::is_futures_symbol(symbol) {
            let json = self.public_get_fapi("/fapi/v1/depth", Some(&params)).await?;
            parsers::parse_order_book(&json, symbol)
        } else {
            let json = self.public_get("/api/v3/depth", Some(&params)).await?;
            parsers::parse_order_book(&json, symbol)
        }
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

        let json = if Self::is_futures_symbol(symbol) {
            self.public_get_fapi("/fapi/v1/klines", Some(&params)).await?
        } else {
            self.public_get("/api/v3/klines", Some(&params)).await?
        };

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

        let json = if Self::is_futures_symbol(symbol) {
            self.public_get_fapi("/fapi/v1/trades", Some(&params)).await?
        } else {
            self.public_get("/api/v3/trades", Some(&params)).await?
        };

        let trades_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of trades".to_string()))?;

        let mut trades = Vec::with_capacity(trades_array.len());
        for trade_json in trades_array {
            if let Ok(trade) = parsers::parse_trade(trade_json, symbol) {
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
        let json = self.public_get("/sapi/v1/system/status", None).await?;
        let status_code = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
        let status = if status_code == 0 { "ok" } else { "maintenance" };
        let msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
        let now = timestamp_ms();

        Ok(ExchangeStatus {
            status: status.to_string(),
            updated: now,
            eta: if msg.is_empty() { None } else { Some(msg.to_string()) },
            url: None,
        })
    }

    async fn fetch_time(&self) -> Result<i64> {
        let json = self.public_get("/api/v3/time", None).await?;
        json.get("serverTime")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| CcxtError::ParseError("Missing serverTime".to_string()))
    }

    // ========================================================================
    // Trading (Private)
    // ========================================================================

    async fn create_order(
        &self,
        symbol: &str,
        order_type: OrderType,
        side: OrderSide,
        amount: Decimal,
        price: Option<Decimal>,
        params: Option<&Params>,
    ) -> Result<Order> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };

        let type_str = match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopLoss => "STOP_LOSS",
            OrderType::StopLossLimit => "STOP_LOSS_LIMIT",
            OrderType::TakeProfit => "TAKE_PROFIT",
            OrderType::TakeProfitLimit => "TAKE_PROFIT_LIMIT",
            OrderType::TrailingStop => "TRAILING_STOP_MARKET",
        };

        let mut request_params = HashMap::new();
        request_params.insert("symbol".to_string(), binance_symbol);
        request_params.insert("side".to_string(), side_str.to_string());
        request_params.insert("type".to_string(), type_str.to_string());
        request_params.insert("quantity".to_string(), amount.to_string());

        if let Some(p) = price {
            request_params.insert("price".to_string(), p.to_string());
        }

        // For LIMIT orders, set timeInForce=GTC unless overridden
        if (order_type == OrderType::Limit || order_type == OrderType::StopLossLimit || order_type == OrderType::TakeProfitLimit)
            && params.is_none_or(|p| !p.contains_key("timeInForce"))
        {
            request_params.insert("timeInForce".to_string(), "GTC".to_string());
        }

        // Apply params overrides
        if let Some(p) = params {
            for (k, v) in p {
                if let Some(s) = v.as_str() {
                    request_params.insert(k.clone(), s.to_string());
                } else {
                    request_params.insert(k.clone(), v.to_string());
                }
            }
        }

        let json = if is_futures {
            self.private_post_fapi("/fapi/v1/order", Some(request_params)).await?
        } else {
            self.private_post("/api/v3/order", Some(request_params)).await?
        };

        parsers::parse_order(&json, symbol, is_futures)
    }

    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("cancel_order requires a symbol for Binance".to_string())
        })?;

        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);
        params.insert("orderId".to_string(), id.to_string());

        let json = if is_futures {
            self.private_delete_fapi("/fapi/v1/order", Some(params)).await?
        } else {
            self.private_delete("/api/v3/order", Some(params)).await?
        };

        parsers::parse_order(&json, symbol, is_futures)
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
        Err(CcxtError::NotSupported("edit_order not supported by Binance spot API".to_string()))
    }

    async fn fetch_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_order requires a symbol for Binance".to_string())
        })?;

        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);
        params.insert("orderId".to_string(), id.to_string());

        let json = if is_futures {
            self.private_get_fapi("/fapi/v1/order", Some(params)).await?
        } else {
            self.private_get("/api/v3/order", Some(params)).await?
        };

        parsers::parse_order(&json, symbol, is_futures)
    }

    async fn fetch_orders(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Order>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_orders requires a symbol for Binance".to_string())
        })?;

        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = if is_futures {
            self.private_get_fapi("/fapi/v1/allOrders", Some(params)).await?
        } else {
            self.private_get("/api/v3/allOrders", Some(params)).await?
        };

        let orders_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of orders".to_string()))?;

        let mut orders = Vec::with_capacity(orders_array.len());
        for order_json in orders_array {
            match parsers::parse_order(order_json, symbol, is_futures) {
                Ok(order) => orders.push(order),
                Err(e) => {
                    tracing::debug!("Failed to parse order: {}", e);
                    continue;
                }
            }
        }

        Ok(orders)
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>, _since: Option<i64>, _limit: Option<u32>) -> Result<Vec<Order>> {
        let is_futures = symbol.is_some_and(Self::is_futures_symbol);

        let mut params = HashMap::new();
        if let Some(sym) = symbol {
            params.insert("symbol".to_string(), parsers::symbol_to_binance(sym));
        }

        let json = if is_futures {
            self.private_get_fapi("/fapi/v1/openOrders", Some(params)).await?
        } else {
            self.private_get("/api/v3/openOrders", Some(params)).await?
        };

        let orders_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of orders".to_string()))?;

        let default_symbol = symbol.unwrap_or("");
        let mut orders = Vec::with_capacity(orders_array.len());
        for order_json in orders_array {
            // Try to get symbol from order JSON for when no symbol filter is applied
            let order_symbol = order_json
                .get("symbol")
                .and_then(|v| v.as_str())
                .map(|s| {
                    if is_futures {
                        parsers::symbol_from_binance_futures(s)
                    } else {
                        parsers::symbol_from_binance(s)
                    }
                })
                .unwrap_or_else(|| default_symbol.to_string());

            match parsers::parse_order(order_json, &order_symbol, is_futures) {
                Ok(order) => orders.push(order),
                Err(e) => {
                    tracing::debug!("Failed to parse order: {}", e);
                    continue;
                }
            }
        }

        Ok(orders)
    }

    async fn fetch_closed_orders(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Order>> {
        let all_orders = self.fetch_orders(symbol, since, limit).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == OrderStatus::Closed)
            .collect())
    }

    async fn fetch_canceled_orders(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Order>> {
        let all_orders = self.fetch_orders(symbol, since, limit).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == OrderStatus::Canceled)
            .collect())
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("cancel_all_orders requires a symbol for Binance".to_string())
        })?;

        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        let json = if is_futures {
            self.private_delete_fapi("/fapi/v1/allOpenOrders", Some(params)).await?
        } else {
            self.private_delete("/api/v3/openOrders", Some(params)).await?
        };

        // Response may be an array of cancelled orders or a success message
        if let Some(orders_array) = json.as_array() {
            let mut orders = Vec::with_capacity(orders_array.len());
            for order_json in orders_array {
                match parsers::parse_order(order_json, symbol, is_futures) {
                    Ok(order) => orders.push(order),
                    Err(e) => {
                        tracing::debug!("Failed to parse cancelled order: {}", e);
                        continue;
                    }
                }
            }
            Ok(orders)
        } else {
            Ok(vec![])
        }
    }

    async fn fetch_my_trades(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Trade>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_my_trades requires a symbol for Binance".to_string())
        })?;

        let binance_symbol = parsers::symbol_to_binance(symbol);
        let is_futures = Self::is_futures_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = if is_futures {
            self.private_get_fapi("/fapi/v1/userTrades", Some(params)).await?
        } else {
            self.private_get("/api/v3/myTrades", Some(params)).await?
        };

        let trades_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of trades".to_string()))?;

        let mut trades = Vec::with_capacity(trades_array.len());
        for trade_json in trades_array {
            match parsers::parse_my_trade(trade_json, symbol) {
                Ok(trade) => trades.push(trade),
                Err(e) => {
                    tracing::debug!("Failed to parse trade: {}", e);
                    continue;
                }
            }
        }

        Ok(trades)
    }

    // ========================================================================
    // Account
    // ========================================================================

    async fn fetch_balance(&self) -> Result<Balances> {
        // Default to spot balance
        let json = self.private_get("/api/v3/account", None).await?;
        parsers::parse_balance_spot(&json)
    }

    async fn fetch_deposit_address(&self, code: &str) -> Result<DepositAddress> {
        let mut params = HashMap::new();
        params.insert("coin".to_string(), code.to_string());

        let json = self.private_get("/sapi/v1/capital/deposit/address", Some(params)).await?;

        Ok(DepositAddress {
            currency: code.to_string(),
            address: json.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            tag: json.get("tag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            network: json.get("coin").and_then(|v| v.as_str()).map(|s| s.to_string()),
            info: Some(json),
        })
    }

    async fn fetch_deposits(&self, code: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Deposit>> {
        let mut params = HashMap::new();
        if let Some(code) = code {
            params.insert("coin".to_string(), code.to_string());
        }
        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/sapi/v1/capital/deposit/hisrec", Some(params)).await?;

        let deposits_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of deposits".to_string()))?;

        let mut deposits = Vec::with_capacity(deposits_array.len());
        for dep_json in deposits_array {
            match parsers::parse_deposit(dep_json) {
                Ok(dep) => deposits.push(dep),
                Err(e) => {
                    tracing::debug!("Failed to parse deposit: {}", e);
                    continue;
                }
            }
        }

        Ok(deposits)
    }

    async fn fetch_withdrawals(&self, code: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Withdrawal>> {
        let mut params = HashMap::new();
        if let Some(code) = code {
            params.insert("coin".to_string(), code.to_string());
        }
        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/sapi/v1/capital/withdraw/history", Some(params)).await?;

        let withdrawals_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of withdrawals".to_string()))?;

        let mut withdrawals = Vec::with_capacity(withdrawals_array.len());
        for wd_json in withdrawals_array {
            match parsers::parse_withdrawal(wd_json) {
                Ok(wd) => withdrawals.push(wd),
                Err(e) => {
                    tracing::debug!("Failed to parse withdrawal: {}", e);
                    continue;
                }
            }
        }

        Ok(withdrawals)
    }

    async fn withdraw(&self, code: &str, amount: Decimal, address: &str, tag: Option<&str>) -> Result<Withdrawal> {
        let mut params = HashMap::new();
        params.insert("coin".to_string(), code.to_string());
        params.insert("address".to_string(), address.to_string());
        params.insert("amount".to_string(), amount.to_string());

        if let Some(tag) = tag {
            params.insert("addressTag".to_string(), tag.to_string());
        }

        let json = self.private_post("/sapi/v1/capital/withdraw/apply", Some(params)).await?;
        let now = timestamp_ms();

        Ok(Withdrawal {
            id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            txid: None,
            timestamp: now,
            datetime: timestamp_to_iso8601(now),
            network: None,
            address: address.to_string(),
            tag: tag.map(|t| t.to_string()),
            transaction_type: TransactionType::Withdrawal,
            amount,
            currency: code.to_string(),
            status: TransactionStatus::Pending,
            updated: None,
            fee: None,
            info: Some(json),
        })
    }

    async fn transfer(&self, code: &str, amount: Decimal, from_account: &str, to_account: &str) -> Result<Transfer> {
        let transfer_type = match (from_account, to_account) {
            ("spot", "futures") | ("spot", "future") => "MAIN_UMFUTURE",
            ("futures", "spot") | ("future", "spot") => "UMFUTURE_MAIN",
            ("spot", "margin") => "MAIN_MARGIN",
            ("margin", "spot") => "MARGIN_MAIN",
            _ => return Err(CcxtError::BadRequest(format!(
                "Unsupported transfer: {} -> {}", from_account, to_account
            ))),
        };

        let mut params = HashMap::new();
        params.insert("type".to_string(), transfer_type.to_string());
        params.insert("asset".to_string(), code.to_string());
        params.insert("amount".to_string(), amount.to_string());

        let json = self.private_post("/sapi/v1/asset/transfer", Some(params)).await?;
        let now = timestamp_ms();

        Ok(Transfer {
            id: json.get("tranId").and_then(|v| v.as_i64()).map(|i| i.to_string()).unwrap_or_default(),
            timestamp: now,
            datetime: timestamp_to_iso8601(now),
            currency: code.to_string(),
            amount,
            from_account: from_account.to_string(),
            to_account: to_account.to_string(),
            status: TransactionStatus::Ok,
            info: Some(json),
        })
    }

    // ========================================================================
    // Derivatives / Futures
    // ========================================================================

    async fn fetch_positions(&self, symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        let mut params = HashMap::new();
        if let Some(syms) = symbols {
            if syms.len() == 1 {
                params.insert("symbol".to_string(), parsers::symbol_to_binance(syms[0]));
            }
        }

        let json = self.private_get_fapi("/fapi/v2/positionRisk", Some(params)).await?;

        let positions_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of positions".to_string()))?;

        let mut positions = Vec::new();
        for pos_json in positions_array {
            // Filter out zero-size positions
            let position_amt = pos_json.get("positionAmt")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            if position_amt.is_zero() {
                continue;
            }

            match parsers::parse_position(pos_json) {
                Ok(pos) => {
                    // Filter by symbols if specified
                    if let Some(syms) = symbols {
                        if !syms.contains(&pos.symbol.as_str()) {
                            continue;
                        }
                    }
                    positions.push(pos);
                }
                Err(e) => {
                    tracing::debug!("Failed to parse position: {}", e);
                    continue;
                }
            }
        }

        Ok(positions)
    }

    async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        let json = self.public_get_fapi("/fapi/v1/premiumIndex", Some(&params)).await?;
        parsers::parse_funding_rate(&json, symbol)
    }

    async fn fetch_funding_rate_history(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRateHistory>> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.public_get_fapi("/fapi/v1/fundingRate", Some(&params)).await?;

        let entries = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of funding rates".to_string()))?;

        let mut history = Vec::with_capacity(entries.len());
        for entry in entries {
            let funding_rate = entry.get("fundingRate")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);
            let funding_time = entry.get("fundingTime")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            history.push(FundingRateHistory {
                symbol: symbol.to_string(),
                funding_rate,
                timestamp: funding_time,
                datetime: timestamp_to_iso8601(funding_time),
                info: Some(entry.clone()),
            });
        }

        Ok(history)
    }

    async fn set_leverage(&self, leverage: u32, symbol: &str) -> Result<()> {
        let binance_symbol = parsers::symbol_to_binance(symbol);

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);
        params.insert("leverage".to_string(), leverage.to_string());

        self.private_post_fapi("/fapi/v1/leverage", Some(params)).await?;
        Ok(())
    }

    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let margin_type = match mode {
            MarginMode::Isolated => "ISOLATED",
            MarginMode::Cross => "CROSSED",
        };

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);
        params.insert("marginType".to_string(), margin_type.to_string());

        match self.private_post_fapi("/fapi/v1/marginType", Some(params)).await {
            Ok(_) => Ok(()),
            Err(CcxtError::BadRequest(msg)) if msg.contains("Margin mode already set") || msg.contains("-4046") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn set_position_mode(&self, hedged: bool, _symbol: Option<&str>) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("dualSidePosition".to_string(), hedged.to_string());

        self.private_post_fapi("/fapi/v1/positionSide/dual", Some(params)).await?;
        Ok(())
    }

    // ========================================================================
    // Fees
    // ========================================================================

    async fn fetch_trading_fee(&self, symbol: &str) -> Result<TradingFees> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        let json = self.private_get("/sapi/v1/asset/tradeFee", Some(params)).await?;

        let fees_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of fees".to_string()))?;

        let fee_json = fees_array.first()
            .ok_or_else(|| CcxtError::ParseError("No fee data returned".to_string()))?;

        let maker = fee_json.get("makerCommission")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);
        let taker = fee_json.get("takerCommission")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);

        Ok(TradingFees {
            symbol: symbol.to_string(),
            maker,
            taker,
            percentage: Some(true),
            tier_based: Some(true),
            info: Some(fee_json.clone()),
        })
    }

    async fn fetch_trading_fees(&self) -> Result<Vec<TradingFees>> {
        let json = self.private_get("/sapi/v1/asset/tradeFee", None).await?;

        let fees_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of fees".to_string()))?;

        let mut fees = Vec::with_capacity(fees_array.len());
        for fee_json in fees_array {
            let binance_symbol = fee_json.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            let symbol = parsers::symbol_from_binance(binance_symbol);

            let maker = fee_json.get("makerCommission")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);
            let taker = fee_json.get("takerCommission")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            fees.push(TradingFees {
                symbol,
                maker,
                taker,
                percentage: Some(true),
                tier_based: Some(true),
                info: Some(fee_json.clone()),
            });
        }

        Ok(fees)
    }

    // ========================================================================
    // Open Interest & Leverage Tiers
    // ========================================================================

    async fn fetch_open_interest(&self, symbol: &str) -> Result<OpenInterest> {
        let binance_symbol = parsers::symbol_to_binance(symbol);
        let mut params = HashMap::new();
        params.insert("symbol".to_string(), binance_symbol);

        let json = self.public_get_fapi("/fapi/v1/openInterest", Some(&params)).await?;
        let now = timestamp_ms();

        let oi = json.get("openInterest")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        Ok(OpenInterest {
            symbol: symbol.to_string(),
            open_interest_amount: oi,
            open_interest_value: None,
            base_volume: None,
            quote_volume: None,
            timestamp: json.get("time").and_then(|v| v.as_i64()).unwrap_or(now),
            datetime: timestamp_to_iso8601(json.get("time").and_then(|v| v.as_i64()).unwrap_or(now)),
            info: Some(json),
        })
    }

    async fn fetch_leverage_tiers(&self, symbols: Option<&[&str]>) -> Result<HashMap<String, Vec<LeverageTier>>> {
        let mut params = HashMap::new();
        if let Some(syms) = symbols {
            if syms.len() == 1 {
                params.insert("symbol".to_string(), parsers::symbol_to_binance(syms[0]));
            }
        }

        let json = self.public_get_fapi("/fapi/v1/leverageBracket", Some(&params)).await?;

        let brackets_array = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Expected array of leverage brackets".to_string()))?;

        let mut result: HashMap<String, Vec<LeverageTier>> = HashMap::new();

        for bracket_json in brackets_array {
            let binance_symbol = bracket_json.get("symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let symbol = parsers::symbol_from_binance_futures(binance_symbol);

            let brackets = bracket_json.get("brackets")
                .and_then(|v| v.as_array());

            if let Some(brackets) = brackets {
                let mut tiers = Vec::with_capacity(brackets.len());
                for (i, tier_json) in brackets.iter().enumerate() {
                    let max_leverage = tier_json.get("initialLeverage")
                        .and_then(|v| v.as_i64())
                        .map(Decimal::from);
                    let max_notional = tier_json.get("notionalCap")
                        .and_then(|v| v.as_i64())
                        .map(Decimal::from);
                    let min_notional = tier_json.get("notionalFloor")
                        .and_then(|v| v.as_i64())
                        .map(Decimal::from);
                    let mmr = tier_json.get("maintMarginRatio")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Decimal::from_str(s).ok());

                    tiers.push(LeverageTier {
                        tier: (i + 1) as u32,
                        currency: Some("USDT".to_string()),
                        min_notional,
                        max_notional,
                        maintenance_margin_rate: mmr,
                        max_leverage,
                        info: Some(tier_json.clone()),
                    });
                }
                result.insert(symbol, tiers);
            }
        }

        Ok(result)
    }

    // ========================================================================
    // Ledger
    // ========================================================================

    async fn fetch_ledger(&self, code: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<LedgerEntry>> {
        let account_type = "SPOT";
        let mut params = HashMap::new();
        params.insert("type".to_string(), account_type.to_string());

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/sapi/v1/accountSnapshot", Some(params)).await?;

        let snapshots = json.get("snapshotVos")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing snapshotVos".to_string()))?;

        let mut entries = Vec::new();
        for (i, snap) in snapshots.iter().enumerate() {
            let update_time = snap.get("updateTime").and_then(|v| v.as_i64()).unwrap_or(0);
            let data = snap.get("data");

            if let Some(data) = data {
                if let Some(balances) = data.get("balances").and_then(|v| v.as_array()) {
                    for bal in balances {
                        let asset = bal.get("asset").and_then(|v| v.as_str()).unwrap_or("");
                        let free_val = bal.get("free").and_then(|v| v.as_str())
                            .and_then(|s| Decimal::from_str(s).ok())
                            .unwrap_or(Decimal::ZERO);

                        // Skip if filtered by code
                        if let Some(c) = code {
                            if asset != c {
                                continue;
                            }
                        }

                        if free_val.is_zero() {
                            continue;
                        }

                        entries.push(LedgerEntry {
                            id: format!("{}-{}", update_time, i),
                            direction: if free_val > Decimal::ZERO { LedgerDirection::In } else { LedgerDirection::Out },
                            account: Some("spot".to_string()),
                            reference_id: None,
                            reference_account: None,
                            entry_type: None,
                            currency: asset.to_string(),
                            amount: free_val.abs(),
                            before: None,
                            after: Some(free_val),
                            fee: None,
                            timestamp: update_time,
                            datetime: timestamp_to_iso8601(update_time),
                            symbol: None,
                            info: Some(bal.clone()),
                        });
                    }
                }
            }
        }

        Ok(entries)
    }
}
