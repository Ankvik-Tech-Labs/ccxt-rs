//! Bybit Exchange Implementation
//!
//! Bybit Unified Trading API v5
//! Docs: https://bybit-exchange.github.io/docs/v5/intro

mod parsers;

use crate::base::errors::{CcxtError, Result};
use crate::base::exchange::{Exchange, ExchangeFeatures, ExchangeType, Params};
use crate::base::http_client::HttpClient;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Bybit exchange API endpoints
const BYBIT_API_URL: &str = "https://api.bybit.com";
const BYBIT_TESTNET_URL: &str = "https://api-testnet.bybit.com";

/// Bybit exchange client
pub struct Bybit {
    /// API credentials (for future private API support)
    #[allow(dead_code)]
    api_key: Option<String>,
    #[allow(dead_code)]
    secret: Option<String>,

    /// HTTP client
    client: HttpClient,

    /// Base URL (mainnet or testnet)
    base_url: String,

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

    /// Make a public GET request
    async fn public_get(&self, path: &str, params: Option<HashMap<String, String>>) -> Result<Value> {
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

        let response = self.client.get(&url, None).await?;
        let json = response.json::<serde_json::Value>().await?;

        // Check for Bybit API errors
        self.check_response(&json)?;

        Ok(json)
    }

    /// Check Bybit API response for errors
    fn check_response(&self, response: &Value) -> Result<()> {
        // Bybit v5 response format:
        // {
        //   "retCode": 0,      // 0 = success
        //   "retMsg": "OK",
        //   "result": {...},
        //   "time": 1234567890
        // }

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

            // Permission errors
            10006 => CcxtError::PermissionDenied(message.to_string()),

            // Rate limit
            10018 => CcxtError::RateLimitExceeded(message.to_string()),

            // Invalid parameters
            10001 | 10016 | 10017 => CcxtError::BadRequest(message.to_string()),

            // Insufficient balance
            110037 => CcxtError::InsufficientFunds(message.to_string()),

            // Order errors (excluding 110037 which is handled above)
            110001..=110036 | 110038..=110099 => CcxtError::InvalidOrder(message.to_string()),

            // Symbol errors
            10002 => CcxtError::BadSymbol(message.to_string()),

            // Default
            _ => CcxtError::ExchangeError(format!("Bybit error {}: {}", code, message)),
        }
    }
}

/// Builder for Bybit exchange
#[derive(Default)]
pub struct BybitBuilder {
    api_key: Option<String>,
    secret: Option<String>,
    sandbox: bool,
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
            markets: std::sync::RwLock::new(None),
            features: ExchangeFeatures {
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_trades: true,
                fetch_markets: true,
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

    async fn load_markets(&self) -> Result<Vec<Market>> {
        // Check cache first
        {
            let markets = self.markets.read().unwrap();
            if let Some(ref cached) = *markets {
                return Ok(cached.clone());
            }
        }

        // Fetch fresh markets
        let markets = self.fetch_markets().await?;

        // Cache them
        {
            let mut cache = self.markets.write().unwrap();
            *cache = Some(markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        // Bybit v5 endpoint: /v5/market/instruments-info
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());

        let response = self.public_get("/v5/market/instruments-info", Some(params)).await?;

        let list = response
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))?;

        let markets: Result<Vec<Market>> = list
            .iter()
            .map(parsers::parse_market)
            .collect();

        markets
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(CcxtError::NotSupported("fetch_currencies not implemented for Bybit".to_string()))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let bybit_symbol = parsers::convert_symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), bybit_symbol.clone());

        let response = self.public_get("/v5/market/tickers", Some(params)).await?;

        let list = response
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))?;

        let ticker_data = list
            .first()
            .ok_or_else(|| CcxtError::BadSymbol(format!("Symbol not found: {}", symbol)))?;

        parsers::parse_ticker(ticker_data, symbol)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());

        let response = self.public_get("/v5/market/tickers", Some(params)).await?;

        let list = response
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))?;

        let all_tickers: Vec<Ticker> = list
            .iter()
            .filter_map(|item| {
                // Parse ticker and convert symbol back to unified format
                let bybit_symbol = item.get("symbol")?.as_str()?;
                let unified_symbol = parsers::convert_symbol_from_bybit(bybit_symbol);
                parsers::parse_ticker(item, &unified_symbol).ok()
            })
            .collect();

        let mut tickers = all_tickers;

        // Filter by requested symbols if provided
        if let Some(filter_symbols) = symbols {
            let filter_set: std::collections::HashSet<_> = filter_symbols.iter().collect();
            tickers.retain(|t| filter_set.contains(&t.symbol.as_str()));
        }

        Ok(tickers)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let bybit_symbol = parsers::convert_symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(25).to_string());

        let response = self.public_get("/v5/market/orderbook", Some(params)).await?;

        let result = response
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
        let bybit_symbol = parsers::convert_symbol_to_bybit(symbol);
        let interval = parsers::timeframe_to_bybit(&timeframe);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("interval".to_string(), interval);

        if let Some(start) = since {
            params.insert("start".to_string(), start.to_string());
        }

        if let Some(l) = limit {
            params.insert("limit".to_string(), l.to_string());
        }

        let response = self.public_get("/v5/market/kline", Some(params)).await?;

        let list = response
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))?;

        let ohlcv: Result<Vec<OHLCV>> = list
            .iter()
            .map(parsers::parse_ohlcv)
            .collect();

        ohlcv
    }

    async fn fetch_trades(
        &self,
        symbol: &str,
        _since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let bybit_symbol = parsers::convert_symbol_to_bybit(symbol);

        let mut params = HashMap::new();
        params.insert("category".to_string(), "spot".to_string());
        params.insert("symbol".to_string(), bybit_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(60).to_string());

        let response = self.public_get("/v5/market/recent-trade", Some(params)).await?;

        let list = response
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing result.list in response".to_string()))?;

        let trades: Result<Vec<Trade>> = list
            .iter()
            .map(|item| parsers::parse_trade(item, symbol))
            .collect();

        trades
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        Ok(ExchangeStatus {
            status: "ok".to_string(),
            updated: chrono::Utc::now().timestamp_millis(),
            eta: None,
            url: None,
        })
    }

    // === Private API methods (not implemented yet) ===

    async fn create_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_order not implemented yet for Bybit".to_string()))
    }

    async fn cancel_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("cancel_order not implemented yet for Bybit".to_string()))
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
        Err(CcxtError::NotSupported("edit_order not implemented yet for Bybit".to_string()))
    }

    async fn fetch_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("fetch_order not implemented yet for Bybit".to_string()))
    }

    async fn fetch_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_orders not implemented yet for Bybit".to_string()))
    }

    async fn fetch_open_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_open_orders not implemented yet for Bybit".to_string()))
    }

    async fn fetch_closed_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_closed_orders not implemented yet for Bybit".to_string()))
    }

    async fn fetch_my_trades(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        Err(CcxtError::NotSupported("fetch_my_trades not implemented yet for Bybit".to_string()))
    }

    async fn fetch_balance(&self) -> Result<Balances> {
        Err(CcxtError::NotSupported("fetch_balance not implemented yet for Bybit".to_string()))
    }

    async fn fetch_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported("fetch_deposit_address not implemented yet for Bybit".to_string()))
    }

    async fn fetch_deposits(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Deposit>> {
        Err(CcxtError::NotSupported("fetch_deposits not implemented yet for Bybit".to_string()))
    }

    async fn fetch_withdrawals(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>> {
        Err(CcxtError::NotSupported("fetch_withdrawals not implemented yet for Bybit".to_string()))
    }

    async fn withdraw(
        &self,
        _code: &str,
        _amount: Decimal,
        _address: &str,
        _tag: Option<&str>,
    ) -> Result<Withdrawal> {
        Err(CcxtError::NotSupported("withdraw not implemented yet for Bybit".to_string()))
    }

    async fn transfer(
        &self,
        _code: &str,
        _amount: Decimal,
        _from_account: &str,
        _to_account: &str,
    ) -> Result<Transfer> {
        Err(CcxtError::NotSupported("transfer not implemented yet for Bybit".to_string()))
    }

    async fn fetch_positions(&self, _symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        Err(CcxtError::NotSupported("fetch_positions not implemented yet for Bybit".to_string()))
    }

    async fn fetch_funding_rate(&self, _symbol: &str) -> Result<FundingRate> {
        Err(CcxtError::NotSupported("fetch_funding_rate not implemented yet for Bybit".to_string()))
    }

    async fn set_leverage(&self, _leverage: u32, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_leverage not implemented yet for Bybit".to_string()))
    }

    async fn set_margin_mode(&self, _mode: MarginMode, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_margin_mode not implemented yet for Bybit".to_string()))
    }
}
