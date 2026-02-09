//! Bybit Exchange Implementation
//!
//! Bybit Unified Trading API v5
//! Docs: https://bybit-exchange.github.io/docs/v5/intro

pub mod parsers;
pub mod ws;

use crate::base::{
    errors::{CcxtError, Result},
    exchange::{Exchange, ExchangeFeatures, ExchangeType, Params},
    http_client::HttpClient,
    signer::{hmac_sha256, timestamp_ms, timestamp_to_iso8601},
};
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

/// Bybit exchange API endpoints
const BYBIT_API_URL: &str = "https://api.bybit.com";
const BYBIT_TESTNET_URL: &str = "https://api-testnet.bybit.com";

/// Default recv_window (5000ms)
const DEFAULT_RECV_WINDOW: &str = "5000";

/// Bybit exchange client
pub struct Bybit {
    /// API credentials
    api_key: Option<String>,
    secret: Option<String>,

    /// HTTP client
    client: HttpClient,

    /// Base URL (mainnet or testnet)
    base_url: String,

    /// Recv window for private requests
    recv_window: String,

    /// Cached markets
    markets: std::sync::RwLock<Option<Vec<Market>>>,

    /// Exchange features
    features: ExchangeFeatures,
}

impl Bybit {
    /// Create a new Bybit client builder
    pub fn builder() -> BybitBuilder {
        BybitBuilder::default()
    }

    /// Check if a unified symbol is a derivatives symbol (contains ':')
    fn is_linear_symbol(symbol: &str) -> bool {
        symbol.contains(':')
    }

    /// Get the category for a symbol ("spot" or "linear")
    fn category_for_symbol(symbol: &str) -> &'static str {
        if Self::is_linear_symbol(symbol) {
            "linear"
        } else {
            "spot"
        }
    }

    /// Get API key or return auth error
    fn require_api_key(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .ok_or_else(|| CcxtError::AuthenticationError("API key not configured".to_string()))
    }

    /// Sign a request using HMAC-SHA256
    /// Bybit v5 signing: HMAC-SHA256(timestamp + api_key + recv_window + payload)
    fn sign(&self, timestamp: &str, payload: &str) -> Result<String> {
        let api_key = self.require_api_key()?;
        let secret = self
            .secret
            .as_ref()
            .ok_or_else(|| CcxtError::AuthenticationError("Secret not configured".to_string()))?;

        let sign_str = format!("{}{}{}{}", timestamp, api_key, self.recv_window, payload);
        hmac_sha256(secret, &sign_str)
    }

    /// Build auth headers for private requests
    fn auth_headers(&self, timestamp: &str, signature: &str) -> Result<HashMap<String, String>> {
        let api_key = self.require_api_key()?;
        let mut headers = HashMap::new();
        headers.insert("X-BAPI-API-KEY".to_string(), api_key.to_string());
        headers.insert("X-BAPI-SIGN".to_string(), signature.to_string());
        headers.insert("X-BAPI-TIMESTAMP".to_string(), timestamp.to_string());
        headers.insert("X-BAPI-RECV-WINDOW".to_string(), self.recv_window.clone());
        Ok(headers)
    }

    /// Build a sorted query string from params
    fn build_query_string(params: &HashMap<String, String>) -> String {
        let mut pairs: Vec<_> = params.iter().collect();
        pairs.sort_by_key(|(k, _)| (*k).clone());
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    // ========================================================================
    // HTTP Methods
    // ========================================================================

    /// Make a public GET request
    async fn public_get(&self, path: &str, params: Option<&HashMap<String, String>>) -> Result<Value> {
        let mut url = format!("{}{}", self.base_url, path);

        if let Some(params) = params {
            let query = Self::build_query_string(params);
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }
        }

        let response = self.client.get(&url, None).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private GET request (signed)
    async fn private_get(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<Value> {
        let params = params.unwrap_or_default();
        let query_string = Self::build_query_string(&params);
        let timestamp = timestamp_ms().to_string();
        let signature = self.sign(&timestamp, &query_string)?;
        let headers = self.auth_headers(&timestamp, &signature)?;

        let mut url = format!("{}{}", self.base_url, path);
        if !query_string.is_empty() {
            url.push('?');
            url.push_str(&query_string);
        }

        let response = self.client.get(&url, Some(headers)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Make a private POST request (signed, JSON body)
    async fn private_post(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<Value> {
        let body = if let Some(params) = params {
            serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        };

        let timestamp = timestamp_ms().to_string();
        let signature = self.sign(&timestamp, &body)?;
        let mut headers = self.auth_headers(&timestamp, &signature)?;
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let url = format!("{}{}", self.base_url, path);

        let response = self.client.post(&url, Some(headers), Some(body)).await?;
        let text = response.text().await.map_err(|e| CcxtError::NetworkError(e.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("JSON parse error: {} - {}", e, text)))?;

        self.check_response(&json)?;
        Ok(json)
    }

    /// Check Bybit API response for errors
    fn check_response(&self, response: &Value) -> Result<()> {
        if let Some(ret_code) = response.get("retCode").and_then(|v| v.as_i64()) {
            if ret_code != 0 {
                let ret_msg = response
                    .get("retMsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");

                return Err(self.map_error(ret_code, ret_msg));
            }
        }

        Ok(())
    }

    /// Map Bybit error codes to CcxtError
    fn map_error(&self, code: i64, message: &str) -> CcxtError {
        match code {
            // Authentication errors
            10003..=10005 => CcxtError::AuthenticationError(message.to_string()),
            33004 => CcxtError::AuthenticationError(message.to_string()),

            // Permission errors
            10006 => CcxtError::PermissionDenied(message.to_string()),

            // Rate limit
            10018 | 10016 => CcxtError::RateLimitExceeded(message.to_string()),

            // Invalid parameters
            10001 | 10017 => CcxtError::BadRequest(message.to_string()),

            // Symbol errors
            10002 => CcxtError::BadSymbol(message.to_string()),

            // Insufficient balance
            110037 | 110007 => CcxtError::InsufficientFunds(message.to_string()),

            // Order not found
            110001 => CcxtError::OrderNotFound(message.to_string()),

            // Margin mode already set
            110026 => CcxtError::BadRequest(format!("Margin mode already set: {}", message)),

            // Position mode already set
            110025 => CcxtError::BadRequest(format!("Position mode already set: {}", message)),

            // Other order errors
            110002..=110024 | 110027..=110036 | 110038..=110099 => CcxtError::InvalidOrder(message.to_string()),

            // Default
            _ => CcxtError::ExchangeError(format!("Bybit error {}: {}", code, message)),
        }
    }

    /// Extract result.list from a Bybit v5 response
    fn extract_list(json: &Value) -> Result<&Vec<Value>> {
        json.get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))
    }
}

/// Builder for Bybit exchange
#[derive(Default)]
pub struct BybitBuilder {
    api_key: Option<String>,
    secret: Option<String>,
    sandbox: bool,
    recv_window: Option<String>,
}

impl BybitBuilder {
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

    /// Enable sandbox/testnet mode
    pub fn sandbox(mut self, enable: bool) -> Self {
        self.sandbox = enable;
        self
    }

    /// Set recv_window (default: 5000)
    pub fn recv_window(mut self, window: u32) -> Self {
        self.recv_window = Some(window.to_string());
        self
    }

    /// Build the Bybit client
    pub fn build(self) -> Result<Bybit> {
        let base_url = if self.sandbox {
            BYBIT_TESTNET_URL.to_string()
        } else {
            BYBIT_API_URL.to_string()
        };

        Ok(Bybit {
            api_key: self.api_key,
            secret: self.secret,
            client: HttpClient::new(None, Duration::from_secs(30))?,
            base_url,
            recv_window: self.recv_window.unwrap_or_else(|| DEFAULT_RECV_WINDOW.to_string()),
            markets: std::sync::RwLock::new(None),
            features: ExchangeFeatures {
                // Market Data
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_trades: true,
                fetch_markets: true,
                fetch_currencies: true,
                fetch_status: true,
                fetch_time: true,
                // Trading
                create_order: true,
                create_market_order: true,
                create_limit_order: true,
                cancel_order: true,
                cancel_all_orders: true,
                edit_order: true,
                // Advanced Order Types
                create_stop_order: true,
                create_stop_limit_order: true,
                create_stop_market_order: true,
                create_stop_loss_order: true,
                create_take_profit_order: true,
                create_trigger_order: true,
                create_post_only_order: true,
                create_reduce_only_order: true,
                // Order Queries
                fetch_order: true,
                fetch_orders: true,
                fetch_open_orders: true,
                fetch_closed_orders: true,
                fetch_canceled_orders: true,
                fetch_my_trades: true,
                // Account
                fetch_balance: true,
                fetch_deposit_address: true,
                fetch_deposits: true,
                fetch_withdrawals: true,
                withdraw: true,
                transfer: true,
                // Fees
                fetch_trading_fee: true,
                fetch_trading_fees: true,
                // Derivatives
                fetch_positions: true,
                fetch_funding_rate: true,
                fetch_funding_rate_history: true,
                set_leverage: true,
                set_margin_mode: true,
                set_position_mode: true,
                fetch_open_interest: true,
                fetch_leverage_tiers: true,
                // Features
                margin_trading: true,
                futures_trading: true,
                swap_trading: true,
                sandbox: true,
                ..Default::default()
            },
        })
    }
}

#[async_trait]
impl Exchange for Bybit {
    fn id(&self) -> &str {
        "bybit"
    }

    fn name(&self) -> &str {
        "Bybit"
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
        {
            let markets = self.markets.read().unwrap();
            if let Some(ref cached) = *markets {
                return Ok(cached.clone());
            }
        }

        let markets = self.fetch_markets().await?;

        {
            let mut cache = self.markets.write().unwrap();
            *cache = Some(markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        // Fetch spot markets
        let mut spot_params = HashMap::new();
        spot_params.insert("category".to_string(), "spot".to_string());

        let spot_response = self.public_get("/v5/market/instruments-info", Some(&spot_params)).await?;

        let spot_list = Self::extract_list(&spot_response)?;
        let mut markets = Vec::with_capacity(spot_list.len());

        for item in spot_list {
            match parsers::parse_market(item) {
                Ok(market) => markets.push(market),
                Err(e) => {
                    tracing::debug!("Failed to parse spot market: {}", e);
                    continue;
                }
            }
        }

        // Fetch linear derivatives markets
        let mut linear_params = HashMap::new();
        linear_params.insert("category".to_string(), "linear".to_string());

        match self.public_get("/v5/market/instruments-info", Some(&linear_params)).await {
            Ok(linear_response) => {
                if let Ok(linear_list) = Self::extract_list(&linear_response) {
                    for item in linear_list {
                        match parsers::parse_linear_market(item) {
                            Ok(market) => markets.push(market),
                            Err(e) => {
                                tracing::debug!("Failed to parse linear market: {}", e);
                                continue;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch linear markets: {}", e);
            }
        }

        Ok(markets)
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        let json = self.private_get("/v5/asset/coin/query-info", None).await?;

        let rows = json
            .get("result")
            .and_then(|r| r.get("rows"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.rows in response".to_string()))?;

        let mut currencies = Vec::with_capacity(rows.len());
        for item in rows {
            match parsers::parse_currency(item) {
                Ok(c) => currencies.push(c),
                Err(e) => {
                    tracing::debug!("Failed to parse currency: {}", e);
                    continue;
                }
            }
        }

        Ok(currencies)
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        let json = self.public_get("/v5/market/tickers", Some(&params)).await?;

        let list = Self::extract_list(&json)?;
        let ticker_data = list
            .first()
            .ok_or_else(|| CcxtError::BadSymbol(format!("Symbol not found: {}", symbol)))?;

        parsers::parse_ticker(ticker_data, symbol)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());

        let json = self.public_get("/v5/market/tickers", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let mut tickers: Vec<Ticker> = list
            .iter()
            .filter_map(|item| {
                let bybit_symbol = item.get("symbol")?.as_str()?;
                let unified_symbol = parsers::symbol_from_bybit(bybit_symbol);
                parsers::parse_ticker(item, &unified_symbol).ok()
            })
            .collect();

        if let Some(filter_symbols) = symbols {
            let filter_set: std::collections::HashSet<_> = filter_symbols.iter().collect();
            tickers.retain(|t| filter_set.contains(&t.symbol.as_str()));
        }

        Ok(tickers)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(25).to_string());

        let json = self.public_get("/v5/market/orderbook", Some(&params)).await?;

        let result = json
            .get("result")
            .ok_or_else(|| CcxtError::ParseError("Missing result in response".to_string()))?;

        parsers::parse_orderbook(result, symbol)
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);
        let interval = parsers::timeframe_to_bybit(&timeframe);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("interval".to_string(), interval);

        if let Some(start) = since {
            params.insert("start".to_string(), start.to_string());
        }
        if let Some(l) = limit {
            params.insert("limit".to_string(), l.to_string());
        }

        let json = self.public_get("/v5/market/kline", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let mut ohlcv: Result<Vec<OHLCV>> = list.iter().map(parsers::parse_ohlcv).collect();

        // Bybit returns newest first, reverse to oldest first
        if let Ok(ref mut candles) = ohlcv {
            candles.reverse();
        }

        ohlcv
    }

    async fn fetch_trades(&self, symbol: &str, _since: Option<i64>, limit: Option<u32>) -> Result<Vec<Trade>> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(60).to_string());

        let json = self.public_get("/v5/market/recent-trade", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let trades: Result<Vec<Trade>> = list
            .iter()
            .map(|item| parsers::parse_trade(item, symbol))
            .collect();

        trades
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        Ok(ExchangeStatus {
            status: "ok".to_string(),
            updated: timestamp_ms(),
            eta: None,
            url: None,
        })
    }

    async fn fetch_time(&self) -> Result<i64> {
        let json = self.public_get("/v5/market/time", None).await?;
        json.get("result")
            .and_then(|r| r.get("timeSecond"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .map(|s| s * 1000)
            .or_else(|| {
                json.get("result")
                    .and_then(|r| r.get("timeNano"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|ns| ns / 1_000_000)
            })
            .ok_or_else(|| CcxtError::ParseError("Missing time in response".to_string()))
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
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let side_str = match side {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        };

        let type_str = match order_type {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
            _ => "Limit",
        };

        let mut request_params = HashMap::new();
        request_params.insert("category".to_string(), category.to_string());
        request_params.insert("symbol".to_string(), bybit_symbol);
        request_params.insert("side".to_string(), side_str.to_string());
        request_params.insert("orderType".to_string(), type_str.to_string());
        request_params.insert("qty".to_string(), amount.to_string());

        if let Some(p) = price {
            request_params.insert("price".to_string(), p.to_string());
        }

        // For Limit orders, set timeInForce=GTC unless overridden
        if order_type == OrderType::Limit
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

        let json = self.private_post("/v5/order/create", Some(request_params)).await?;

        let result = json
            .get("result")
            .ok_or_else(|| CcxtError::ParseError("Missing result in order response".to_string()))?;

        parsers::parse_order(result, symbol)
    }

    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("cancel_order requires a symbol for Bybit".to_string())
        })?;

        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("orderId".to_string(), id.to_string());

        let json = self.private_post("/v5/order/cancel", Some(params)).await?;

        let result = json
            .get("result")
            .ok_or_else(|| CcxtError::ParseError("Missing result in cancel response".to_string()))?;

        parsers::parse_order(result, symbol)
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
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("orderId".to_string(), id.to_string());

        if let Some(qty) = amount {
            params.insert("qty".to_string(), qty.to_string());
        }
        if let Some(p) = price {
            params.insert("price".to_string(), p.to_string());
        }

        let json = self.private_post("/v5/order/amend", Some(params)).await?;

        let result = json
            .get("result")
            .ok_or_else(|| CcxtError::ParseError("Missing result in amend response".to_string()))?;

        parsers::parse_order(result, symbol)
    }

    async fn fetch_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_order requires a symbol for Bybit".to_string())
        })?;

        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("orderId".to_string(), id.to_string());

        let json = self.private_get("/v5/order/realtime", Some(params)).await?;

        let list = Self::extract_list(&json)?;
        let order_data = list
            .first()
            .ok_or_else(|| CcxtError::OrderNotFound(format!("Order not found: {}", id)))?;

        parsers::parse_order(order_data, symbol)
    }

    async fn fetch_orders(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Order>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_orders requires a symbol for Bybit".to_string())
        })?;

        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/v5/order/history", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let mut orders = Vec::with_capacity(list.len());
        for item in list {
            match parsers::parse_order(item, symbol) {
                Ok(order) => {
                    if let Some(since_ts) = since {
                        if order.timestamp < since_ts {
                            continue;
                        }
                    }
                    orders.push(order);
                }
                Err(e) => {
                    tracing::debug!("Failed to parse order: {}", e);
                    continue;
                }
            }
        }

        Ok(orders)
    }

    async fn fetch_open_orders(&self, symbol: Option<&str>, _since: Option<i64>, limit: Option<u32>) -> Result<Vec<Order>> {
        let category = symbol
            .map(Self::category_for_symbol)
            .unwrap_or("spot");

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());

        if let Some(sym) = symbol {
            params.insert("symbol".to_string(), parsers::symbol_to_bybit(sym));
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/v5/order/realtime", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let default_symbol = symbol.unwrap_or("");
        let mut orders = Vec::with_capacity(list.len());
        for item in list {
            let order_symbol = item
                .get("symbol")
                .and_then(|v| v.as_str())
                .map(|s| {
                    if category == "linear" {
                        parsers::symbol_from_bybit_linear(s)
                    } else {
                        parsers::symbol_from_bybit(s)
                    }
                })
                .unwrap_or_else(|| default_symbol.to_string());

            match parsers::parse_order(item, &order_symbol) {
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
            CcxtError::ArgumentsRequired("cancel_all_orders requires a symbol for Bybit".to_string())
        })?;

        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        let json = self.private_post("/v5/order/cancel-all", Some(params)).await?;

        // Response has result.list with cancelled order IDs
        if let Some(list) = json.get("result").and_then(|r| r.get("list")).and_then(|l| l.as_array()) {
            let mut orders = Vec::with_capacity(list.len());
            for item in list {
                match parsers::parse_order(item, symbol) {
                    Ok(order) => orders.push(order),
                    Err(_) => continue,
                }
            }
            Ok(orders)
        } else {
            Ok(vec![])
        }
    }

    async fn fetch_my_trades(&self, symbol: Option<&str>, since: Option<i64>, limit: Option<u32>) -> Result<Vec<Trade>> {
        let symbol = symbol.ok_or_else(|| {
            CcxtError::ArgumentsRequired("fetch_my_trades requires a symbol for Bybit".to_string())
        })?;

        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.private_get("/v5/execution/list", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let mut trades = Vec::with_capacity(list.len());
        for item in list {
            match parsers::parse_my_trade(item, symbol) {
                Ok(trade) => {
                    if let Some(since_ts) = since {
                        if trade.timestamp < since_ts {
                            continue;
                        }
                    }
                    trades.push(trade);
                }
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
        // Bybit v5 unified account
        let mut params = HashMap::new();
        params.insert("accountType".to_string(), "UNIFIED".to_string());

        let json = self.private_get("/v5/account/wallet-balance", Some(params)).await?;

        let list = Self::extract_list(&json)?;
        let account = list
            .first()
            .ok_or_else(|| CcxtError::ParseError("No account data".to_string()))?;

        parsers::parse_balance(account)
    }

    async fn fetch_deposit_address(&self, code: &str) -> Result<DepositAddress> {
        let mut params = HashMap::new();
        params.insert("coin".to_string(), code.to_string());

        let json = self.private_get("/v5/asset/deposit/query-addr", Some(params)).await?;

        let chains = json
            .get("result")
            .and_then(|r| r.get("chains"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.chains".to_string()))?;

        let first = chains
            .first()
            .ok_or_else(|| CcxtError::ParseError("No deposit address found".to_string()))?;

        Ok(DepositAddress {
            currency: code.to_string(),
            address: first.get("addressDeposit").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            tag: first.get("tagDeposit").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()),
            network: first.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string()),
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

        let json = self.private_get("/v5/asset/deposit/query-record", Some(params)).await?;

        let rows = json
            .get("result")
            .and_then(|r| r.get("rows"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.rows".to_string()))?;

        let mut deposits = Vec::with_capacity(rows.len());
        for item in rows {
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

        let json = self.private_get("/v5/asset/withdraw/query-record", Some(params)).await?;

        let rows = json
            .get("result")
            .and_then(|r| r.get("rows"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.rows".to_string()))?;

        let mut withdrawals = Vec::with_capacity(rows.len());
        for item in rows {
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

    async fn withdraw(&self, code: &str, amount: Decimal, address: &str, tag: Option<&str>) -> Result<Withdrawal> {
        let mut params = HashMap::new();
        params.insert("coin".to_string(), code.to_string());
        params.insert("address".to_string(), address.to_string());
        params.insert("amount".to_string(), amount.to_string());
        params.insert("timestamp".to_string(), timestamp_ms().to_string());
        params.insert("forceChain".to_string(), "0".to_string());
        params.insert("accountType".to_string(), "FUND".to_string());

        if let Some(tag) = tag {
            params.insert("tag".to_string(), tag.to_string());
        }

        let json = self.private_post("/v5/asset/withdraw/create", Some(params)).await?;
        let now = timestamp_ms();

        let id = json
            .get("result")
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Withdrawal {
            id,
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
        // Bybit account types: UNIFIED, SPOT, CONTRACT, FUND
        let from_type = match from_account {
            "spot" => "SPOT",
            "funding" | "fund" => "FUND",
            "unified" | "trading" => "UNIFIED",
            "contract" | "futures" | "derivatives" => "CONTRACT",
            _ => return Err(CcxtError::BadRequest(format!("Unknown account type: {}", from_account))),
        };
        let to_type = match to_account {
            "spot" => "SPOT",
            "funding" | "fund" => "FUND",
            "unified" | "trading" => "UNIFIED",
            "contract" | "futures" | "derivatives" => "CONTRACT",
            _ => return Err(CcxtError::BadRequest(format!("Unknown account type: {}", to_account))),
        };

        let transfer_id = format!("{}", timestamp_ms());

        let mut params = HashMap::new();
        params.insert("transferId".to_string(), transfer_id.clone());
        params.insert("coin".to_string(), code.to_string());
        params.insert("amount".to_string(), amount.to_string());
        params.insert("fromAccountType".to_string(), from_type.to_string());
        params.insert("toAccountType".to_string(), to_type.to_string());

        let json = self.private_post("/v5/asset/transfer/inter-transfer", Some(params)).await?;
        let now = timestamp_ms();

        let id = json
            .get("result")
            .and_then(|r| r.get("transferId"))
            .and_then(|v| v.as_str())
            .unwrap_or(&transfer_id)
            .to_string();

        Ok(Transfer {
            id,
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
        params.insert("category".to_string(), "linear".to_string());
        params.insert("settleCoin".to_string(), "USDT".to_string());

        if let Some(syms) = symbols {
            if syms.len() == 1 {
                params.insert("symbol".to_string(), parsers::symbol_to_bybit(syms[0]));
            }
        }

        let json = self.private_get("/v5/position/list", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let mut positions = Vec::new();
        for item in list {
            let size = item
                .get("size")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            if size.is_zero() {
                continue;
            }

            match parsers::parse_position(item) {
                Ok(pos) => {
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
        let bybit_symbol = parsers::symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        let json = self.public_get("/v5/market/tickers", Some(&params)).await?;

        let list = Self::extract_list(&json)?;
        let data = list
            .first()
            .ok_or_else(|| CcxtError::BadSymbol(format!("Symbol not found: {}", symbol)))?;

        parsers::parse_funding_rate(data, symbol)
    }

    async fn fetch_funding_rate_history(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRateHistory>> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        if let Some(since) = since {
            params.insert("startTime".to_string(), since.to_string());
        }
        if let Some(limit) = limit {
            params.insert("limit".to_string(), limit.to_string());
        }

        let json = self.public_get("/v5/market/funding/history", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let mut history = Vec::with_capacity(list.len());
        for item in list {
            let funding_rate = item
                .get("fundingRate")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);
            let funding_time = item
                .get("fundingRateTimestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);

            history.push(FundingRateHistory {
                symbol: symbol.to_string(),
                funding_rate,
                timestamp: funding_time,
                datetime: timestamp_to_iso8601(funding_time),
                info: Some(item.clone()),
            });
        }

        Ok(history)
    }

    async fn set_leverage(&self, leverage: u32, symbol: &str) -> Result<()> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("buyLeverage".to_string(), leverage.to_string());
        params.insert("sellLeverage".to_string(), leverage.to_string());

        self.private_post("/v5/position/set-leverage", Some(params)).await?;
        Ok(())
    }

    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let trade_mode = match mode {
            MarginMode::Isolated => "1",
            MarginMode::Cross => "0",
        };

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("tradeMode".to_string(), trade_mode.to_string());
        params.insert("buyLeverage".to_string(), "10".to_string());
        params.insert("sellLeverage".to_string(), "10".to_string());

        match self.private_post("/v5/position/switch-isolated", Some(params)).await {
            Ok(_) => Ok(()),
            Err(CcxtError::BadRequest(msg)) if msg.contains("Margin mode already set") || msg.contains("110026") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn set_position_mode(&self, hedged: bool, _symbol: Option<&str>) -> Result<()> {
        let mode = if hedged { "3" } else { "0" }; // 3=BothSide, 0=MergedSingle

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("mode".to_string(), mode.to_string());

        match self.private_post("/v5/position/switch-mode", Some(params)).await {
            Ok(_) => Ok(()),
            Err(CcxtError::BadRequest(msg)) if msg.contains("Position mode already set") || msg.contains("110025") => Ok(()),
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // Fees
    // ========================================================================

    async fn fetch_trading_fee(&self, symbol: &str) -> Result<TradingFees> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);
        let category = Self::category_for_symbol(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), category.to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        let json = self.private_get("/v5/account/fee-rate", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let fee_data = list
            .first()
            .ok_or_else(|| CcxtError::ParseError("No fee data returned".to_string()))?;

        let maker = fee_data
            .get("makerFeeRate")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);
        let taker = fee_data
            .get("takerFeeRate")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
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
        // Bybit doesn't have a bulk fee endpoint, fetch for spot category
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());

        let json = self.private_get("/v5/account/fee-rate", Some(params)).await?;
        let list = Self::extract_list(&json)?;

        let mut fees = Vec::with_capacity(list.len());
        for item in list {
            let bybit_symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            let symbol = parsers::symbol_from_bybit(bybit_symbol);

            let maker = item
                .get("makerFeeRate")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);
            let taker = item
                .get("takerFeeRate")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ZERO);

            fees.push(TradingFees {
                symbol,
                maker,
                taker,
                percentage: Some(true),
                tier_based: Some(true),
                info: Some(item.clone()),
            });
        }

        Ok(fees)
    }

    // ========================================================================
    // Open Interest & Leverage Tiers
    // ========================================================================

    async fn fetch_open_interest(&self, symbol: &str) -> Result<OpenInterest> {
        let bybit_symbol = parsers::symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());
        params.insert("symbol".to_string(), bybit_symbol);

        let json = self.public_get("/v5/market/open-interest", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let data = list
            .first()
            .ok_or_else(|| CcxtError::ParseError("No open interest data".to_string()))?;

        let oi = data
            .get("openInterest")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());
        let ts = data
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or_else(timestamp_ms);

        Ok(OpenInterest {
            symbol: symbol.to_string(),
            open_interest_amount: oi,
            open_interest_value: None,
            base_volume: None,
            quote_volume: None,
            timestamp: ts,
            datetime: timestamp_to_iso8601(ts),
            info: Some(data.clone()),
        })
    }

    async fn fetch_leverage_tiers(&self, symbols: Option<&[&str]>) -> Result<HashMap<String, Vec<LeverageTier>>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), "linear".to_string());

        if let Some(syms) = symbols {
            if syms.len() == 1 {
                params.insert("symbol".to_string(), parsers::symbol_to_bybit(syms[0]));
            }
        }

        let json = self.public_get("/v5/market/risk-limit", Some(&params)).await?;
        let list = Self::extract_list(&json)?;

        let mut result: HashMap<String, Vec<LeverageTier>> = HashMap::new();

        for (i, item) in list.iter().enumerate() {
            let bybit_symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            let symbol = parsers::symbol_from_bybit_linear(bybit_symbol);

            let max_leverage = item
                .get("maxLeverage")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());
            let limit_val = item
                .get("riskLimitValue")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());
            let mmr = item
                .get("maintenanceMargin")
                .and_then(|v| v.as_str())
                .and_then(|s| Decimal::from_str(s).ok());

            let tier = LeverageTier {
                tier: (i + 1) as u32,
                currency: Some("USDT".to_string()),
                min_notional: None,
                max_notional: limit_val,
                maintenance_margin_rate: mmr,
                max_leverage,
                info: Some(item.clone()),
            };

            result.entry(symbol).or_default().push(tier);
        }

        Ok(result)
    }
}
