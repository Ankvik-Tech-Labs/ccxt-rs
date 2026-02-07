//! OKX Exchange Implementation
//!
//! OKX REST API v5
//! Docs: https://www.okx.com/docs-v5/en/

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

/// OKX exchange API endpoints
const OKX_API_URL: &str = "https://www.okx.com";
const OKX_AWS_URL: &str = "https://aws.okx.com";

/// OKX exchange client
pub struct Okx {
    /// API credentials (for future private API support)
    #[allow(dead_code)]
    api_key: Option<String>,
    #[allow(dead_code)]
    secret: Option<String>,
    #[allow(dead_code)]
    passphrase: Option<String>,

    /// HTTP client
    client: HttpClient,

    /// Base URL (main or AWS)
    base_url: String,

    /// Cached markets
    markets: std::sync::RwLock<Option<Vec<Market>>>,

    /// Exchange features
    features: ExchangeFeatures,
}

impl Okx {
    /// Create a new OKX client builder
    pub fn builder() -> OkxBuilder {
        OkxBuilder::default()
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

        // Check for OKX API errors
        self.check_response(&json)?;

        Ok(json)
    }

    /// Check OKX API response for errors
    fn check_response(&self, response: &Value) -> Result<()> {
        // OKX v5 response format:
        // {
        //   "code": "0",      // "0" = success
        //   "msg": "",
        //   "data": [...]
        // }

        if let Some(code) = response.get("code").and_then(|v| v.as_str()) {
            if code != "0" {
                let msg = response
                    .get("msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");

                // Try to parse code as i64 for error mapping
                if let Ok(code_num) = code.parse::<i64>() {
                    return Err(self.map_error(code_num, msg));
                } else {
                    return Err(CcxtError::ExchangeError(format!("OKX error {}: {}", code, msg)));
                }
            }
        }

        Ok(())
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

            // Symbol errors (before order errors range)
            51001 => CcxtError::BadSymbol(message.to_string()),

            // Insufficient balance (before order errors range)
            51020 => CcxtError::InsufficientFunds(message.to_string()),

            // Order errors (excluding 51001 and 51020)
            51000 | 51002..=51019 | 51021..=51999 => CcxtError::InvalidOrder(message.to_string()),

            // System errors
            50013 | 50014 => CcxtError::ExchangeNotAvailable(message.to_string()),

            // Default
            _ => CcxtError::ExchangeError(format!("OKX error {}: {}", code, message)),
        }
    }
}

/// Builder for OKX exchange
#[derive(Default)]
pub struct OkxBuilder {
    api_key: Option<String>,
    secret: Option<String>,
    passphrase: Option<String>,
    use_aws: bool,
}

impl OkxBuilder {
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
        // OKX v5 endpoint: /api/v5/public/instruments
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SPOT".to_string());

        let response = self.public_get("/api/v5/public/instruments", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

        let markets: Result<Vec<Market>> = data
            .iter()
            .map(parsers::parse_market)
            .collect();

        markets
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(CcxtError::NotSupported("fetch_currencies not implemented for OKX".to_string()))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let okx_symbol = parsers::convert_symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);

        let response = self.public_get("/api/v5/market/ticker", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

        let ticker_data = data
            .first()
            .ok_or_else(|| CcxtError::BadSymbol(format!("Symbol not found: {}", symbol)))?;

        parsers::parse_ticker(ticker_data, symbol)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        let mut params = HashMap::new();
        params.insert("instType".to_string(), "SPOT".to_string());

        let response = self.public_get("/api/v5/market/tickers", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

        let all_tickers: Vec<Ticker> = data
            .iter()
            .filter_map(|item| {
                // Parse ticker and convert symbol back to unified format
                let okx_symbol = item.get("instId")?.as_str()?;
                let unified_symbol = parsers::convert_symbol_from_okx(okx_symbol);
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
        let okx_symbol = parsers::convert_symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("sz".to_string(), limit.unwrap_or(20).to_string());

        let response = self.public_get("/api/v5/market/books", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

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
        let okx_symbol = parsers::convert_symbol_to_okx(symbol);
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

        let response = self.public_get("/api/v5/market/candles", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

        let mut ohlcv: Result<Vec<OHLCV>> = data
            .iter()
            .map(parsers::parse_ohlcv)
            .collect();

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
        let okx_symbol = parsers::convert_symbol_to_okx(symbol);

        let mut params = HashMap::new();
        params.insert("instId".to_string(), okx_symbol);
        params.insert("limit".to_string(), limit.unwrap_or(100).to_string());

        let response = self.public_get("/api/v5/market/trades", Some(params)).await?;

        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing data in response".to_string()))?;

        let trades: Result<Vec<Trade>> = data
            .iter()
            .map(|item| parsers::parse_trade(item, symbol))
            .collect();

        trades
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        // OKX has a system status endpoint
        match self.public_get("/api/v5/system/status", None).await {
            Ok(response) => {
                if let Some(data) = response
                    .get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                {
                    parsers::parse_status(data)
                } else {
                    // If no data, assume exchange is operating normally
                    Ok(ExchangeStatus {
                        status: "ok".to_string(),
                        updated: chrono::Utc::now().timestamp_millis(),
                        eta: None,
                        url: None,
                    })
                }
            }
            Err(_) => {
                // If status endpoint fails, assume ok (endpoint might not be available)
                Ok(ExchangeStatus {
                    status: "ok".to_string(),
                    updated: chrono::Utc::now().timestamp_millis(),
                    eta: None,
                    url: None,
                })
            }
        }
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
        Err(CcxtError::NotSupported("create_order not implemented yet for OKX".to_string()))
    }

    async fn cancel_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("cancel_order not implemented yet for OKX".to_string()))
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
        Err(CcxtError::NotSupported("edit_order not implemented yet for OKX".to_string()))
    }

    async fn fetch_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported("fetch_order not implemented yet for OKX".to_string()))
    }

    async fn fetch_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_orders not implemented yet for OKX".to_string()))
    }

    async fn fetch_open_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_open_orders not implemented yet for OKX".to_string()))
    }

    async fn fetch_closed_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_closed_orders not implemented yet for OKX".to_string()))
    }

    async fn fetch_my_trades(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        Err(CcxtError::NotSupported("fetch_my_trades not implemented yet for OKX".to_string()))
    }

    async fn fetch_balance(&self) -> Result<Balances> {
        Err(CcxtError::NotSupported("fetch_balance not implemented yet for OKX".to_string()))
    }

    async fn fetch_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported("fetch_deposit_address not implemented yet for OKX".to_string()))
    }

    async fn fetch_deposits(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Deposit>> {
        Err(CcxtError::NotSupported("fetch_deposits not implemented yet for OKX".to_string()))
    }

    async fn fetch_withdrawals(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>> {
        Err(CcxtError::NotSupported("fetch_withdrawals not implemented yet for OKX".to_string()))
    }

    async fn withdraw(
        &self,
        _code: &str,
        _amount: Decimal,
        _address: &str,
        _tag: Option<&str>,
    ) -> Result<Withdrawal> {
        Err(CcxtError::NotSupported("withdraw not implemented yet for OKX".to_string()))
    }

    async fn transfer(
        &self,
        _code: &str,
        _amount: Decimal,
        _from_account: &str,
        _to_account: &str,
    ) -> Result<Transfer> {
        Err(CcxtError::NotSupported("transfer not implemented yet for OKX".to_string()))
    }

    async fn fetch_positions(&self, _symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        Err(CcxtError::NotSupported("fetch_positions not implemented yet for OKX".to_string()))
    }

    async fn fetch_funding_rate(&self, _symbol: &str) -> Result<FundingRate> {
        Err(CcxtError::NotSupported("fetch_funding_rate not implemented yet for OKX".to_string()))
    }

    async fn set_leverage(&self, _leverage: u32, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_leverage not implemented yet for OKX".to_string()))
    }

    async fn set_margin_mode(&self, _mode: MarginMode, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported("set_margin_mode not implemented yet for OKX".to_string()))
    }
}
