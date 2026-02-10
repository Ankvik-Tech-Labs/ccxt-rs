//! OKX Exchange Implementation
//!
//! OKX REST API v5
//! Docs: https://www.okx.com/docs-v5/en/
//!
//! # Example
//!
//! ```no_run
//! use ccxt::okx::Okx;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let okx = Okx::builder()
//!         .api_key("your-api-key")
//!         .secret("your-secret")
//!         .passphrase("your-passphrase")
//!         .build()?;
//!
//!     let ticker = okx.fetch_ticker("BTC/USDT").await?;
//!     println!("BTC/USDT: ${}", ticker.last.unwrap());
//!
//!     Ok(())
//! }
//! ```

pub mod parsers;
pub mod ws;

use crate::base::{
    errors::{CcxtError, Result},
    exchange::{Exchange, ExchangeFeatures, ExchangeType, Params},
    http_client::HttpClient,
    market_cache::MarketCache,
    signer::{hmac_sha256_base64, iso8601_now, timestamp_ms, timestamp_to_iso8601},
};
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// OKX exchange API endpoints
const OKX_API_URL: &str = "https://www.okx.com";
const OKX_AWS_URL: &str = "https://aws.okx.com";

/// OKX exchange client
pub struct Okx {
    /// API credentials
    api_key: Option<String>,
    secret: Option<String>,
    passphrase: Option<String>,

    /// HTTP client
    client: HttpClient,

    /// Base URL (main or AWS)
    base_url: String,

    /// Sandbox/demo mode
    sandbox: bool,

    /// Market cache with TTL
    market_cache: Arc<tokio::sync::RwLock<MarketCache>>,

    /// Exchange features
    features: ExchangeFeatures,
}

/// Builder for OKX exchange
pub struct OkxBuilder {
    api_key: Option<String>,
    secret: Option<String>,
    passphrase: Option<String>,
    use_aws: bool,
    sandbox: bool,
    timeout: Duration,
    market_cache_ttl: Duration,
}

impl OkxBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            api_key: None,
            secret: None,
            passphrase: None,
            use_aws: false,
            sandbox: false,
            timeout: Duration::from_secs(30),
            market_cache_ttl: Duration::from_secs(3600), // Default: 1 hour
        }
    }

    /// Set API key
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set secret key
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Set passphrase (OKX requires this in addition to API key/secret)
    pub fn passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Use AWS endpoint instead of main endpoint
    pub fn use_aws(mut self, enable: bool) -> Self {
        self.use_aws = enable;
        self
    }

    /// Enable sandbox/demo trading mode
    pub fn sandbox(mut self, enabled: bool) -> Self {
        self.sandbox = enabled;
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

    /// Build the OKX client
    pub fn build(self) -> Result<Okx> {
        let base_url = if self.use_aws {
            OKX_AWS_URL.to_string()
        } else {
            OKX_API_URL.to_string()
        };

        Ok(Okx {
            api_key: self.api_key,
            secret: self.secret,
            passphrase: self.passphrase,
            client: HttpClient::new(None, self.timeout)?,
            base_url,
            sandbox: self.sandbox,
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
                edit_order: true,
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
                futures_trading: true,
                swap_trading: true,
                sandbox: true,
                ..Default::default()
            },
        })
    }
}

impl Default for OkxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Okx {
    /// Create a new OKX client builder
    pub fn builder() -> OkxBuilder {
        OkxBuilder::new()
    }

    // ========================================================================
    // Authentication
    // ========================================================================

    /// Get API key or return auth error
    fn require_api_key(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .ok_or_else(|| CcxtError::AuthenticationError("API key not configured".to_string()))
    }

    /// Get secret or return auth error
    fn require_secret(&self) -> Result<&str> {
        self.secret
            .as_deref()
            .ok_or_else(|| CcxtError::AuthenticationError("Secret not configured".to_string()))
    }

    /// Get passphrase or return auth error
    fn require_passphrase(&self) -> Result<&str> {
        self.passphrase
            .as_deref()
            .ok_or_else(|| CcxtError::AuthenticationError("Passphrase not configured".to_string()))
    }

    /// Build OKX v5 authentication headers
    ///
    /// OKX sign string: `timestamp + method + requestPath + body`
    /// Signature: HMAC-SHA256-Base64(secret, sign_string)
    fn sign_headers(
        &self,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<HashMap<String, String>> {
        let api_key = self.require_api_key()?;
        let secret = self.require_secret()?;
        let passphrase = self.require_passphrase()?;

        let timestamp = iso8601_now();
        let sign_str = format!("{}{}{}{}", timestamp, method, request_path, body);
        let signature = hmac_sha256_base64(secret, &sign_str)?;

        let mut headers = HashMap::new();
        headers.insert("OK-ACCESS-KEY".to_string(), api_key.to_string());
        headers.insert("OK-ACCESS-SIGN".to_string(), signature);
        headers.insert("OK-ACCESS-TIMESTAMP".to_string(), timestamp);
        headers.insert("OK-ACCESS-PASSPHRASE".to_string(), passphrase.to_string());

        if self.sandbox {
            headers.insert("x-simulated-trading".to_string(), "1".to_string());
        }

        Ok(headers)
    }

    // ========================================================================
    // HTTP Methods
    // ========================================================================

    /// Build query string from params
    fn build_query_string(params: &HashMap<String, String>) -> String {
        let mut parts: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect();
        parts.sort();
        parts.join("&")
    }

    /// Make a public GET request
    async fn public_get(
        &self,
        path: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<Value> {
        let mut url = format!("{}{}", self.base_url, path);

        if let Some(ref params) = params {
            let query = Self::build_query_string(params);
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }
        }

        let response = self.client.get(&url, None).await?;
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private GET request (authenticated)
    ///
    /// OKX signs the full path including query string for GET requests.
    async fn private_get(
        &self,
        path: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<Value> {
        let mut request_path = path.to_string();

        if let Some(ref params) = params {
            let query = Self::build_query_string(params);
            if !query.is_empty() {
                request_path.push('?');
                request_path.push_str(&query);
            }
        }

        let headers = self.sign_headers("GET", &request_path, "")?;
        let url = format!("{}{}", self.base_url, request_path);

        let response = self.client.get(&url, Some(headers)).await?;
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private POST request (authenticated, JSON body)
    ///
    /// OKX signs path + JSON body for POST requests.
    async fn private_post(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<Value> {
        let body_str = serde_json::to_string(body)
            .map_err(|e| CcxtError::ParseError(format!("JSON serialize error: {}", e)))?;

        let mut headers = self.sign_headers("POST", path, &body_str)?;
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let url = format!("{}{}", self.base_url, path);

        let response = self
            .client
            .post(&url, Some(headers), Some(body_str))
            .await?;
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    // ========================================================================
    // Response Handling
    // ========================================================================

    /// Check OKX API response for errors
    fn check_response(&self, response: &Value) -> Result<()> {
        if let Some(code) = response.get("code").and_then(|v| v.as_str()) {
            if code != "0" {
                let msg = response
                    .get("msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");

                // Check nested sCode in data array for more specific errors
                if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
                    if let Some(first) = data.first() {
                        if let Some(s_code) = first.get("sCode").and_then(|v| v.as_str()) {
                            let s_msg = first
                                .get("sMsg")
                                .and_then(|v| v.as_str())
                                .unwrap_or(msg);
                            if let Ok(code_num) = s_code.parse::<i64>() {
                                return Err(self.map_error(code_num, s_msg));
                            }
                        }
                    }
                }

                if let Ok(code_num) = code.parse::<i64>() {
                    return Err(self.map_error(code_num, msg));
                } else {
                    return Err(CcxtError::ExchangeError(format!(
                        "OKX error {}: {}",
                        code, msg
                    )));
                }
            }
        }

        Ok(())
    }

    /// Extract data array from OKX response
    fn extract_data(response: &Value) -> Result<Vec<Value>> {
        response
            .get("data")
            .and_then(|d| d.as_array())
            .cloned()
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))
    }

    /// Map OKX error codes to CcxtError
    fn map_error(&self, code: i64, message: &str) -> CcxtError {
        match code {
            // Authentication errors
            50100..=50113 => CcxtError::AuthenticationError(message.to_string()),

            // Permission errors
            50000 => CcxtError::PermissionDenied(message.to_string()),

            // Rate limit
            50011 => CcxtError::RateLimitExceeded(message.to_string()),

            // Invalid parameters
            50001 | 50002 | 50004 | 50005 => CcxtError::BadRequest(message.to_string()),

            // Nonce / timestamp
            50114 => CcxtError::InvalidNonce(message.to_string()),

            // Symbol errors
            51001 => CcxtError::BadSymbol(message.to_string()),

            // Insufficient balance
            51008 | 51020 => CcxtError::InsufficientFunds(message.to_string()),

            // Order not found
            51603 => CcxtError::OrderNotFound(message.to_string()),

            // Margin mode already set (before general order errors range)
            51409 => CcxtError::BadRequest(format!("Margin mode already set: {}", message)),

            // Order errors
            51000 | 51002..=51007 | 51009..=51019 | 51021..=51408 | 51410..=51602 | 51604..=51999 => {
                CcxtError::InvalidOrder(message.to_string())
            }

            // System errors
            50013 | 50014 => CcxtError::ExchangeNotAvailable(message.to_string()),

            // Default
            _ => CcxtError::ExchangeError(format!("OKX error {}: {}", code, message)),
        }
    }

    /// Get OKX trade mode for a symbol
    fn td_mode_for_symbol(symbol: &str) -> &'static str {
        if parsers::is_swap_symbol(symbol) {
            "cross"
        } else {
            "cash"
        }
    }
}

// ============================================================================
// Exchange Trait Implementation
// ============================================================================

#[async_trait]
impl Exchange for Okx {
    fn id(&self) -> &str {
        "okx"
    }

    fn name(&self) -> &str {
        "OKX"
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
            if let Some(markets) = cache.get("okx") {
                return Ok(markets);
            }
        }

        // Fetch spot markets
        let mut spot_params = HashMap::new();
        spot_params.insert("instType".to_string(), "SPOT".to_string());

        let spot_response = self
            .public_get("/api/v5/public/instruments", Some(spot_params))
            .await?;
        let spot_data = Self::extract_data(&spot_response)?;

        let mut markets = Vec::with_capacity(spot_data.len());
        for item in &spot_data {
            match parsers::parse_market(item) {
                Ok(market) => markets.push(market),
                Err(e) => {
                    tracing::debug!("Failed to parse spot market: {}", e);
                    continue;
                }
            }
        }

        // Fetch swap/perpetual markets
        let mut swap_params = HashMap::new();
        swap_params.insert("instType".to_string(), "SWAP".to_string());

        match self
            .public_get("/api/v5/public/instruments", Some(swap_params))
            .await
        {
            Ok(swap_response) => {
                if let Ok(swap_data) = Self::extract_data(&swap_response) {
                    for item in &swap_data {
                        match parsers::parse_swap_market(item) {
                            Ok(market) => markets.push(market),
                            Err(e) => {
                                tracing::debug!("Failed to parse swap market: {}", e);
                                continue;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch swap markets: {}", e);
            }
        }

        // Cache the result
        {
            let mut cache = self.market_cache.write().await;
            cache.insert("okx".to_string(), markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        let json = self
            .private_get("/api/v5/asset/currencies", None)
            .await?;
        let data = Self::extract_data(&json)?;

        let mut currencies = Vec::with_capacity(data.len());
        for item in &data {
            match parsers::parse_currency(item) {
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
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);

        let response = self
            .public_get("/api/v5/market/ticker", Some(params))
            .await?;
        let data = Self::extract_data(&response)?;

        let ticker_data = data
            .first()
            .ok_or_else(|| CcxtError::BadSymbol(format!("Symbol not found: {}", symbol)))?;

        parsers::parse_ticker(ticker_data, symbol)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        // Fetch SPOT tickers
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SPOT".to_string());

        let response = self
            .public_get("/api/v5/market/tickers", Some(params))
            .await?;
        let data = Self::extract_data(&response)?;

        let mut tickers: Vec<Ticker> = data
            .iter()
            .filter_map(|item| {
                let okx_symbol = item.get("instId")?.as_str()?;
                let unified_symbol = parsers::symbol_from_okx(okx_symbol);
                parsers::parse_ticker(item, &unified_symbol).ok()
            })
            .collect();

        // Also fetch SWAP tickers
        let mut swap_params = HashMap::new();
        swap_params.insert("instType".to_string(), "SWAP".to_string());

        if let Ok(swap_response) = self
            .public_get("/api/v5/market/tickers", Some(swap_params))
            .await
        {
            if let Ok(swap_data) = Self::extract_data(&swap_response) {
                let swap_tickers: Vec<Ticker> = swap_data
                    .iter()
                    .filter_map(|item| {
                        let okx_symbol = item.get("instId")?.as_str()?;
                        let unified_symbol = parsers::symbol_from_okx(okx_symbol);
                        parsers::parse_ticker(item, &unified_symbol).ok()
                    })
                    .collect();
                tickers.extend(swap_tickers);
            }
        }

        // Filter by requested symbols if provided
        if let Some(filter_symbols) = symbols {
            let filter_set: std::collections::HashSet<_> = filter_symbols.iter().collect();
            tickers.retain(|t| filter_set.contains(&t.symbol.as_str()));
        }

        Ok(tickers)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("sz".to_string(), limit.unwrap_or(20).to_string());

        let response = self
            .public_get("/api/v5/market/books", Some(params))
            .await?;
        let data = Self::extract_data(&response)?;

        let orderbook_data = data
            .first()
            .ok_or_else(|| CcxtError::ParseError("Empty orderbook data".to_string()))?;

        parsers::parse_orderbook(orderbook_data, symbol)
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>> {
        let okx_symbol = parsers::symbol_to_okx(symbol);
        let bar = parsers::timeframe_to_okx(&timeframe);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("bar".to_string(), bar);

        if let Some(start) = since {
            params.insert("after".to_string(), start.to_string());
        }
        if let Some(l) = limit {
            params.insert("limit".to_string(), l.to_string());
        }

        let response = self
            .public_get("/api/v5/market/candles", Some(params))
            .await?;
        let data = Self::extract_data(&response)?;

        let mut ohlcv: Result<Vec<OHLCV>> = data.iter().map(parsers::parse_ohlcv).collect();

        // OKX returns newest first, reverse to oldest first (CCXT standard)
        if let Ok(ref mut candles) = ohlcv {
            candles.reverse();
        }

        ohlcv
    }

    async fn fetch_trades(
        &self,
        symbol: &str,
        _since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(100).to_string());

        let response = self
            .public_get("/api/v5/market/trades", Some(params))
            .await?;
        let data = Self::extract_data(&response)?;

        let trades: Result<Vec<Trade>> = data
            .iter()
            .map(|item| parsers::parse_trade(item, symbol))
            .collect();

        trades
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        match self.public_get("/api/v5/system/status", None).await {
            Ok(response) => {
                if let Some(data) = response
                    .get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                {
                    parsers::parse_status(data)
                } else {
                    Ok(ExchangeStatus {
                        status: "ok".to_string(),
                        updated: chrono::Utc::now().timestamp_millis(),
                        eta: None,
                        url: None,
                    })
                }
            }
            Err(_) => Ok(ExchangeStatus {
                status: "ok".to_string(),
                updated: chrono::Utc::now().timestamp_millis(),
                eta: None,
                url: None,
            }),
        }
    }

    async fn fetch_time(&self) -> Result<i64> {
        let json = self.public_get("/api/v5/public/time", None).await?;
        let data = Self::extract_data(&json)?;

        data.first()
            .and_then(|d| d.get("ts"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| CcxtError::ParseError("Missing ts in time response".to_string()))
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
        let okx_symbol = parsers::symbol_to_okx(symbol);
        let td_mode = Self::td_mode_for_symbol(symbol);

        let side_str = match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let ord_type = match order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            _ => "limit",
        };

        let mut body = serde_json::json!({
            "instId": okx_symbol,
            "tdMode": td_mode,
            "side": side_str,
            "ordType": ord_type,
            "sz": amount.to_string(),
        });

        if let Some(p) = price {
            body["px"] = serde_json::json!(p.to_string());
        }

        // Apply params overrides
        if let Some(p) = params {
            if let Some(obj) = body.as_object_mut() {
                for (k, v) in p {
                    // Map common CCXT params to OKX names
                    match k.as_str() {
                        "clientOrderId" | "clOrdId" => {
                            obj.insert("clOrdId".to_string(), v.clone());
                        }
                        "reduceOnly" => {
                            obj.insert("reduceOnly".to_string(), v.clone());
                        }
                        "postOnly" => {
                            if v.as_bool() == Some(true) {
                                obj.insert("ordType".to_string(), serde_json::json!("post_only"));
                            }
                        }
                        "timeInForce" => {
                            if let Some(tif) = v.as_str() {
                                match tif {
                                    "FOK" | "fok" => {
                                        obj.insert(
                                            "ordType".to_string(),
                                            serde_json::json!("fok"),
                                        );
                                    }
                                    "IOC" | "ioc" => {
                                        obj.insert(
                                            "ordType".to_string(),
                                            serde_json::json!("ioc"),
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "stopLossPrice" | "slTriggerPx" => {
                            obj.insert("slTriggerPx".to_string(), v.clone());
                        }
                        "takeProfitPrice" | "tpTriggerPx" => {
                            obj.insert("tpTriggerPx".to_string(), v.clone());
                        }
                        _ => {
                            obj.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }

        let json = self.private_post("/api/v5/trade/order", &body).await?;
        let data = Self::extract_data(&json)?;

        let order_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No order data in response".to_string())
        })?;

        // OKX create_order response only has ordId, so fetch full order if needed
        let order_id = order_data
            .get("ordId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Try to fetch the full order details
        match self.fetch_order(&order_id, Some(symbol)).await {
            Ok(order) => Ok(order),
            Err(_) => {
                // Fallback: build minimal order from response
                let now = timestamp_ms();
                Ok(Order {
                    id: order_id,
                    client_order_id: order_data
                        .get("clOrdId")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string()),
                    symbol: symbol.to_string(),
                    order_type,
                    side,
                    status: OrderStatus::Open,
                    timestamp: now,
                    datetime: timestamp_to_iso8601(now),
                    last_trade_timestamp: None,
                    price,
                    average: None,
                    amount,
                    filled: Some(Decimal::ZERO),
                    remaining: Some(amount),
                    cost: None,
                    fee: None,
                    time_in_force: Some(TimeInForce::Gtc),
                    post_only: None,
                    reduce_only: None,
                    stop_price: None,
                    trigger_price: None,
                    stop_loss_price: None,
                    take_profit_price: None,
                    last_update_timestamp: None,
                    trades: None,
                    info: Some(order_data.clone()),
                })
            }
        }
    }

    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired(
                "cancel_order requires a symbol for OKX".to_string(),
            )
        })?;

        let okx_symbol = parsers::symbol_to_okx(symbol);

        let body = serde_json::json!({
            "instId": okx_symbol,
            "ordId": id,
        });

        let json = self
            .private_post("/api/v5/trade/cancel-order", &body)
            .await?;
        let data = Self::extract_data(&json)?;

        let cancel_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No cancel data in response".to_string())
        })?;

        let now = timestamp_ms();
        Ok(Order {
            id: cancel_data
                .get("ordId")
                .and_then(|v| v.as_str())
                .unwrap_or(id)
                .to_string(),
            client_order_id: cancel_data
                .get("clOrdId")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            symbol: symbol.to_string(),
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            status: OrderStatus::Canceled,
            timestamp: now,
            datetime: timestamp_to_iso8601(now),
            last_trade_timestamp: None,
            price: None,
            average: None,
            amount: Decimal::ZERO,
            filled: None,
            remaining: None,
            cost: None,
            fee: None,
            time_in_force: None,
            post_only: None,
            reduce_only: None,
            stop_price: None,
            trigger_price: None,
            stop_loss_price: None,
            take_profit_price: None,
            last_update_timestamp: None,
            trades: None,
            info: Some(cancel_data.clone()),
        })
    }

    async fn edit_order(
        &self,
        id: &str,
        symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        amount: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<Order> {
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut body = serde_json::json!({
            "instId": okx_symbol,
            "ordId": id,
        });

        if let Some(new_sz) = amount {
            body["newSz"] = serde_json::json!(new_sz.to_string());
        }
        if let Some(new_px) = price {
            body["newPx"] = serde_json::json!(new_px.to_string());
        }

        let json = self
            .private_post("/api/v5/trade/amend-order", &body)
            .await?;
        let data = Self::extract_data(&json)?;

        let order_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No order data in response".to_string())
        })?;

        let order_id = order_data
            .get("ordId")
            .and_then(|v| v.as_str())
            .unwrap_or(id);

        self.fetch_order(order_id, Some(symbol)).await
    }

    async fn fetch_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired(
                "fetch_order requires a symbol for OKX".to_string(),
            )
        })?;

        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("ordId".to_string(), id.to_string());

        let json = self
            .private_get("/api/v5/trade/order", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let order_data = data.first().ok_or_else(|| {
            CcxtError::OrderNotFound(format!("Order {} not found", id))
        })?;

        parsers::parse_order(order_data, symbol)
    }

    async fn fetch_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired(
                "fetch_orders requires a symbol for OKX".to_string(),
            )
        })?;

        let okx_symbol = parsers::symbol_to_okx(symbol);
        let inst_type = parsers::inst_type_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("instType".to_string(), inst_type.to_string());
        params.insert("instId".to_string(), okx_symbol);

        if let Some(since) = since {
            params.insert("begin".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .private_get("/api/v5/trade/orders-history-archive", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut orders = Vec::with_capacity(data.len());
        for item in &data {
            match parsers::parse_order(item, symbol) {
                Ok(order) => orders.push(order),
                Err(e) => {
                    tracing::debug!("Failed to parse order: {}", e);
                    continue;
                }
            }
        }

        Ok(orders)
    }

    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let mut params = HashMap::new();

        if let Some(sym) = symbol {
            let okx_symbol = parsers::symbol_to_okx(sym);
            let inst_type = parsers::inst_type_for_symbol(sym);
            params.insert("instId".to_string(), okx_symbol);
            params.insert("instType".to_string(), inst_type.to_string());
        }

        let json = self
            .private_get("/api/v5/trade/orders-pending", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let default_symbol = symbol.unwrap_or("");
        let mut orders = Vec::with_capacity(data.len());
        for item in &data {
            let order_symbol = item
                .get("instId")
                .and_then(|v| v.as_str())
                .map(parsers::symbol_from_okx)
                .unwrap_or_else(|| default_symbol.to_string());

            match parsers::parse_order(item, &order_symbol) {
                Ok(order) => orders.push(order),
                Err(e) => {
                    tracing::debug!("Failed to parse open order: {}", e);
                    continue;
                }
            }
        }

        Ok(orders)
    }

    async fn fetch_closed_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let all_orders = self.fetch_orders(symbol, since, limit).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == OrderStatus::Closed)
            .collect())
    }

    async fn fetch_canceled_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let all_orders = self.fetch_orders(symbol, since, limit).await?;
        Ok(all_orders
            .into_iter()
            .filter(|o| o.status == OrderStatus::Canceled)
            .collect())
    }

    async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        // OKX requires cancelling by instId, so fetch open orders first
        let open_orders = self.fetch_open_orders(symbol, None, None).await?;

        let mut cancelled = Vec::new();
        for order in &open_orders {
            match self.cancel_order(&order.id, Some(&order.symbol)).await {
                Ok(o) => cancelled.push(o),
                Err(e) => {
                    tracing::debug!("Failed to cancel order {}: {}", order.id, e);
                    continue;
                }
            }
        }

        Ok(cancelled)
    }

    async fn fetch_my_trades(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired(
                "fetch_my_trades requires a symbol for OKX".to_string(),
            )
        })?;

        let okx_symbol = parsers::symbol_to_okx(symbol);
        let inst_type = parsers::inst_type_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("instType".to_string(), inst_type.to_string());
        params.insert("instId".to_string(), okx_symbol);

        if let Some(since) = since {
            params.insert("begin".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .private_get("/api/v5/trade/fills-history", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut trades = Vec::with_capacity(data.len());
        for item in &data {
            match parsers::parse_my_trade(item, symbol) {
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
        let json = self
            .private_get("/api/v5/account/balance", None)
            .await?;
        let data = Self::extract_data(&json)?;

        let balance_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No balance data in response".to_string())
        })?;

        parsers::parse_balance(balance_data)
    }

    async fn fetch_deposit_address(&self, code: &str) -> Result<DepositAddress> {
        let mut params = HashMap::new();
        params.insert("ccy".to_string(), code.to_string());

        let json = self
            .private_get("/api/v5/asset/deposit-address", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let addr_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No deposit address in response".to_string())
        })?;

        Ok(DepositAddress {
            currency: code.to_string(),
            address: addr_data
                .get("addr")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tag: addr_data
                .get("tag")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            network: addr_data
                .get("chain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            info: Some(addr_data.clone()),
        })
    }

    async fn fetch_deposits(
        &self,
        code: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Deposit>> {
        let mut params = HashMap::new();
        if let Some(code) = code {
            params.insert("ccy".to_string(), code.to_string());
        }
        if let Some(since) = since {
            params.insert("after".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .private_get("/api/v5/asset/deposit-history", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut deposits = Vec::with_capacity(data.len());
        for item in &data {
            match parsers::parse_deposit(item) {
                Ok(dep) => deposits.push(dep),
                Err(e) => {
                    tracing::debug!("Failed to parse deposit: {}", e);
                    continue;
                }
            }
        }

        Ok(deposits)
    }

    async fn fetch_withdrawals(
        &self,
        code: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>> {
        let mut params = HashMap::new();
        if let Some(code) = code {
            params.insert("ccy".to_string(), code.to_string());
        }
        if let Some(since) = since {
            params.insert("after".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .private_get("/api/v5/asset/withdrawal-history", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut withdrawals = Vec::with_capacity(data.len());
        for item in &data {
            match parsers::parse_withdrawal(item) {
                Ok(wd) => withdrawals.push(wd),
                Err(e) => {
                    tracing::debug!("Failed to parse withdrawal: {}", e);
                    continue;
                }
            }
        }

        Ok(withdrawals)
    }

    async fn withdraw(
        &self,
        code: &str,
        amount: Decimal,
        address: &str,
        tag: Option<&str>,
    ) -> Result<Withdrawal> {
        let mut body = serde_json::json!({
            "ccy": code,
            "amt": amount.to_string(),
            "dest": "4",  // 4 = on-chain withdrawal
            "toAddr": address,
        });

        if let Some(tag) = tag {
            body["tag"] = serde_json::json!(tag);
        }

        let json = self
            .private_post("/api/v5/asset/withdrawal", &body)
            .await?;
        let data = Self::extract_data(&json)?;

        let wd_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No withdrawal data in response".to_string())
        })?;

        let now = timestamp_ms();
        Ok(Withdrawal {
            id: wd_data
                .get("wdId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
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
            info: Some(wd_data.clone()),
        })
    }

    async fn transfer(
        &self,
        code: &str,
        amount: Decimal,
        from_account: &str,
        to_account: &str,
    ) -> Result<Transfer> {
        // OKX account IDs: 6=funding, 18=trading (unified)
        let from_id = match from_account {
            "funding" | "main" | "spot" => "6",
            "trading" | "futures" | "swap" => "18",
            _ => {
                return Err(CcxtError::BadRequest(format!(
                    "Unsupported from_account: {}",
                    from_account
                )))
            }
        };

        let to_id = match to_account {
            "funding" | "main" | "spot" => "6",
            "trading" | "futures" | "swap" => "18",
            _ => {
                return Err(CcxtError::BadRequest(format!(
                    "Unsupported to_account: {}",
                    to_account
                )))
            }
        };

        let body = serde_json::json!({
            "ccy": code,
            "amt": amount.to_string(),
            "from": from_id,
            "to": to_id,
        });

        let json = self
            .private_post("/api/v5/asset/transfer", &body)
            .await?;
        let data = Self::extract_data(&json)?;

        let transfer_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No transfer data in response".to_string())
        })?;

        let now = timestamp_ms();
        Ok(Transfer {
            id: transfer_data
                .get("transId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            timestamp: now,
            datetime: timestamp_to_iso8601(now),
            currency: code.to_string(),
            amount,
            from_account: from_account.to_string(),
            to_account: to_account.to_string(),
            status: TransactionStatus::Ok,
            info: Some(transfer_data.clone()),
        })
    }

    // ========================================================================
    // Fees
    // ========================================================================

    async fn fetch_trading_fee(&self, symbol: &str) -> Result<TradingFees> {
        let inst_type = parsers::inst_type_for_symbol(symbol);
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instType".to_string(), inst_type.to_string());
        params.insert("instId".to_string(), okx_symbol);

        let json = self
            .private_get("/api/v5/account/trade-fee", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let fee_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No fee data in response".to_string())
        })?;

        let maker = fee_data
            .get("maker")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .map(|d| d.abs())
            .unwrap_or(Decimal::ZERO);
        let taker = fee_data
            .get("taker")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .map(|d| d.abs())
            .unwrap_or(Decimal::ZERO);

        Ok(TradingFees {
            symbol: symbol.to_string(),
            maker,
            taker,
            percentage: Some(true),
            tier_based: Some(true),
            info: Some(fee_data.clone()),
        })
    }

    async fn fetch_trading_fees(&self) -> Result<Vec<TradingFees>> {
        // OKX returns account-level fee rates, not per-symbol
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SPOT".to_string());

        let json = self
            .private_get("/api/v5/account/trade-fee", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut fees = Vec::with_capacity(data.len());
        for fee_data in &data {
            let maker = fee_data
                .get("maker")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .map(|d| d.abs())
                .unwrap_or(Decimal::ZERO);
            let taker = fee_data
                .get("taker")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .map(|d| d.abs())
                .unwrap_or(Decimal::ZERO);

            fees.push(TradingFees {
                symbol: "".to_string(),
                maker,
                taker,
                percentage: Some(true),
                tier_based: Some(true),
                info: Some(fee_data.clone()),
            });
        }

        Ok(fees)
    }

    // ========================================================================
    // Derivatives / Futures
    // ========================================================================

    async fn fetch_positions(&self, symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SWAP".to_string());

        if let Some(syms) = symbols {
            if syms.len() == 1 {
                params.insert(
                    "instId".to_string(),
                    parsers::symbol_to_okx(syms[0]),
                );
            }
        }

        let json = self
            .private_get("/api/v5/account/positions", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut positions = Vec::new();
        for item in &data {
            // Filter out zero-size positions
            let pos = item
                .get("pos")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            if pos.is_zero() {
                continue;
            }

            match parsers::parse_position(item) {
                Ok(position) => {
                    if let Some(syms) = symbols {
                        if !syms.contains(&position.symbol.as_str()) {
                            continue;
                        }
                    }
                    positions.push(position);
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
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);

        let json = self
            .public_get("/api/v5/public/funding-rate", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let rate_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No funding rate data".to_string())
        })?;

        parsers::parse_funding_rate(rate_data, symbol)
    }

    async fn fetch_funding_rate_history(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRateHistory>> {
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);

        if let Some(since) = since {
            params.insert("before".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .public_get("/api/v5/public/funding-rate-history", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut history = Vec::with_capacity(data.len());
        for entry in &data {
            let funding_rate = entry
                .get("fundingRate")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);
            let funding_time = entry
                .get("fundingTime")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
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
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let body = serde_json::json!({
            "instId": okx_symbol,
            "lever": leverage.to_string(),
            "mgnMode": "cross",
        });

        self.private_post("/api/v5/account/set-leverage", &body)
            .await?;
        Ok(())
    }

    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()> {
        let okx_symbol = parsers::symbol_to_okx(symbol);
        let mgn_mode = match mode {
            MarginMode::Isolated => "isolated",
            MarginMode::Cross => "cross",
        };

        let body = serde_json::json!({
            "instId": okx_symbol,
            "lever": "10",  // OKX requires lever when setting mgnMode via set-leverage
            "mgnMode": mgn_mode,
        });

        match self
            .private_post("/api/v5/account/set-leverage", &body)
            .await
        {
            Ok(_) => Ok(()),
            Err(CcxtError::BadRequest(msg)) if msg.contains("Margin mode already set") || msg.contains("51409") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn set_position_mode(&self, hedged: bool, _symbol: Option<&str>) -> Result<()> {
        let pos_mode = if hedged { "long_short_mode" } else { "net_mode" };

        let body = serde_json::json!({
            "posMode": pos_mode,
        });

        self.private_post("/api/v5/account/set-position-mode", &body)
            .await?;
        Ok(())
    }

    // ========================================================================
    // Open Interest & Leverage Tiers
    // ========================================================================

    async fn fetch_open_interest(&self, symbol: &str) -> Result<OpenInterest> {
        let okx_symbol = parsers::symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SWAP".to_string());
        params.insert("instId".to_string(), okx_symbol);

        let json = self
            .public_get("/api/v5/public/open-interest", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let oi_data = data.first().ok_or_else(|| {
            CcxtError::ParseError("No open interest data".to_string())
        })?;

        let ts = oi_data
            .get("ts")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or_else(timestamp_ms);

        let oi = oi_data
            .get("oi")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        let oi_ccy = oi_data
            .get("oiCcy")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        Ok(OpenInterest {
            symbol: symbol.to_string(),
            open_interest_amount: oi,
            open_interest_value: oi_ccy,
            base_volume: None,
            quote_volume: None,
            timestamp: ts,
            datetime: timestamp_to_iso8601(ts),
            info: Some(oi_data.clone()),
        })
    }

    async fn fetch_leverage_tiers(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, Vec<LeverageTier>>> {
        // OKX: GET /api/v5/public/position-tiers?instType=SWAP&tdMode=cross
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SWAP".to_string());
        params.insert("tdMode".to_string(), "cross".to_string());

        if let Some(syms) = symbols {
            if syms.len() == 1 {
                let okx_symbol = parsers::symbol_to_okx(syms[0]);
                params.insert("instId".to_string(), okx_symbol);
            }
        }

        let json = self
            .public_get("/api/v5/public/position-tiers", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut result: HashMap<String, Vec<LeverageTier>> = HashMap::new();

        for (i, tier_json) in data.iter().enumerate() {
            let inst_id = tier_json
                .get("instId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let symbol = parsers::symbol_from_okx(inst_id);

            let max_leverage = tier_json
                .get("maxLever")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());
            let min_sz = tier_json
                .get("minSz")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());
            let max_sz = tier_json
                .get("maxSz")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());
            let mmr = tier_json
                .get("mmr")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());

            let tier = LeverageTier {
                tier: (i + 1) as u32,
                currency: Some("USDT".to_string()),
                min_notional: min_sz,
                max_notional: max_sz,
                maintenance_margin_rate: mmr,
                max_leverage,
                info: Some(tier_json.clone()),
            };

            result
                .entry(symbol)
                .or_default()
                .push(tier);
        }

        Ok(result)
    }

    // ========================================================================
    // Ledger
    // ========================================================================

    async fn fetch_ledger(
        &self,
        code: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<LedgerEntry>> {
        let mut params = HashMap::new();

        if let Some(code) = code {
            params.insert("ccy".to_string(), code.to_string());
        }
        if let Some(since) = since {
            params.insert("begin".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self
            .private_get("/api/v5/account/bills", Some(params))
            .await?;
        let data = Self::extract_data(&json)?;

        let mut entries = Vec::with_capacity(data.len());
        for item in &data {
            let bill_id = item
                .get("billId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let ts = item
                .get("ts")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);

            let ccy = item
                .get("ccy")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let bal_chg = item
                .get("balChg")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            let bal = item
                .get("bal")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());

            let direction = if bal_chg >= Decimal::ZERO {
                LedgerDirection::In
            } else {
                LedgerDirection::Out
            };

            let sub_type = item
                .get("subType")
                .and_then(|v| v.as_str())
                .map(|s| match s {
                    "1" | "2" | "3" | "4" => LedgerEntryType::Trade,
                    "5" | "22" => LedgerEntryType::Liquidation,
                    "6" | "7" => LedgerEntryType::Transfer,
                    "8" => LedgerEntryType::Margin,
                    "9" | "11" => LedgerEntryType::FundingFee,
                    "14" => LedgerEntryType::Rebate,
                    "173" => LedgerEntryType::Fee,
                    _ => LedgerEntryType::Other,
                });

            let inst_id = item
                .get("instId")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(parsers::symbol_from_okx);

            entries.push(LedgerEntry {
                id: bill_id,
                direction,
                account: Some("trading".to_string()),
                reference_id: None,
                reference_account: None,
                entry_type: sub_type,
                currency: ccy,
                amount: bal_chg.abs(),
                before: None,
                after: bal,
                fee: None,
                timestamp: ts,
                datetime: timestamp_to_iso8601(ts),
                symbol: inst_id,
                info: Some(item.clone()),
            });
        }

        Ok(entries)
    }
}
