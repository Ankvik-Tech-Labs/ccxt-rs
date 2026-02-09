//! Hyperliquid exchange implementation
//!
//! Implements the Exchange trait for Hyperliquid, a high-performance
//! perpetual DEX with a central limit order book architecture.
//!
//! # Example
//!
//! ```no_run
//! use ccxt::hyperliquid::Hyperliquid;
//! use ccxt::base::exchange::Exchange;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let hl = Hyperliquid::builder()
//!         .sandbox(true)
//!         .build()?;
//!
//!     let ticker = hl.fetch_ticker("BTC/USD:USDC").await?;
//!     println!("BTC: ${}", ticker.last.unwrap());
//!     Ok(())
//! }
//! ```

use crate::base::errors::{CcxtError, Result};
use crate::base::exchange::{Exchange, ExchangeFeatures, ExchangeType, Params};
use crate::base::rate_limiter::RateLimiter;
use crate::base::signer::timestamp_ms;
use crate::hyperliquid::client::HyperliquidClient;
use crate::hyperliquid::constants;
use crate::hyperliquid::parsers;
use crate::hyperliquid::signer::{float_to_wire, HyperliquidSigner};
use crate::hyperliquid::types::*;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Builder for Hyperliquid exchange.
pub struct HyperliquidBuilder {
    private_key: Option<String>,
    sandbox: bool,
    rate_limit: bool,
    timeout: Duration,
    vault_address: Option<String>,
}

impl HyperliquidBuilder {
    pub fn new() -> Self {
        Self {
            private_key: None,
            sandbox: false,
            rate_limit: true,
            timeout: Duration::from_secs(30),
            vault_address: None,
        }
    }

    /// Set the EVM private key for signing (hex, with or without 0x prefix).
    pub fn private_key(mut self, key: impl Into<String>) -> Self {
        self.private_key = Some(key.into());
        self
    }

    /// Enable sandbox/testnet mode.
    pub fn sandbox(mut self, enabled: bool) -> Self {
        self.sandbox = enabled;
        self
    }

    /// Enable rate limiting (default: true).
    pub fn rate_limit(mut self, enabled: bool) -> Self {
        self.rate_limit = enabled;
        self
    }

    /// Set request timeout (default: 30s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set vault address for vault trading.
    pub fn vault_address(mut self, addr: impl Into<String>) -> Self {
        self.vault_address = Some(addr.into());
        self
    }

    /// Build the Hyperliquid exchange client.
    pub fn build(self) -> Result<Hyperliquid> {
        let base_url = if self.sandbox {
            constants::TESTNET_API_URL
        } else {
            constants::MAINNET_API_URL
        };

        let rate_limiter = if self.rate_limit {
            Some(Arc::new(RateLimiter::new(
                constants::DEFAULT_RATE_LIMIT_PER_SECOND,
            )))
        } else {
            None
        };

        let client = HyperliquidClient::new(base_url, rate_limiter, self.timeout)?;

        let signer = match &self.private_key {
            Some(key) => Some(HyperliquidSigner::new(key, !self.sandbox)?),
            None => None,
        };

        Ok(Hyperliquid {
            client,
            signer,
            vault_address: self.vault_address,
            markets: Arc::new(RwLock::new(None)),
            asset_index: Arc::new(RwLock::new(HashMap::new())),
            asset_names: Arc::new(RwLock::new(HashMap::new())),
            meta: Arc::new(RwLock::new(None)),
            features: ExchangeFeatures {
                fetch_ticker: true,
                fetch_tickers: true,
                fetch_order_book: true,
                fetch_ohlcv: true,
                fetch_trades: true,
                fetch_markets: true,
                fetch_status: true,
                create_order: true,
                create_market_order: true,
                create_limit_order: true,
                create_stop_order: true,
                create_stop_loss_order: true,
                create_take_profit_order: true,
                create_trigger_order: true,
                cancel_order: true,
                cancel_all_orders: true,
                create_orders: true,
                cancel_orders: true,
                edit_order: true,
                fetch_order: true,
                fetch_orders: true,
                fetch_open_orders: true,
                fetch_closed_orders: true,
                fetch_my_trades: true,
                fetch_balance: true,
                withdraw: true,
                transfer: true,
                fetch_positions: true,
                fetch_funding_rate: true,
                fetch_funding_rate_history: true,
                fetch_leverage_tiers: true,
                set_leverage: true,
                set_margin_mode: true,
                swap_trading: true,
                ..Default::default()
            },
        })
    }
}

impl Default for HyperliquidBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Hyperliquid perpetual DEX exchange client.
pub struct Hyperliquid {
    client: HyperliquidClient,
    signer: Option<HyperliquidSigner>,
    vault_address: Option<String>,
    markets: Arc<RwLock<Option<Vec<Market>>>>,
    asset_index: Arc<RwLock<HashMap<String, u32>>>,
    asset_names: Arc<RwLock<HashMap<u32, String>>>,
    meta: Arc<RwLock<Option<HlMeta>>>,
    features: ExchangeFeatures,
}

impl Hyperliquid {
    /// Create a new builder.
    pub fn builder() -> HyperliquidBuilder {
        HyperliquidBuilder::new()
    }

    /// Get the signer, returning an error if not configured.
    fn require_signer(&self) -> Result<&HyperliquidSigner> {
        self.signer.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("Private key not configured".to_string())
        })
    }

    /// Get the user's wallet address (requires signer).
    fn user_address(&self) -> Result<String> {
        Ok(self.require_signer()?.address_hex())
    }

    /// Resolve a unified symbol to the Hyperliquid internal name and asset index.
    async fn resolve_symbol(&self, symbol: &str) -> Result<(String, u32)> {
        let hl_name = parsers::symbol_to_hyperliquid(symbol)?;

        let index_guard = self.asset_index.read().await;
        if let Some(&idx) = index_guard.get(&hl_name) {
            return Ok((hl_name, idx));
        }
        drop(index_guard);

        // Markets not loaded yet — load them
        self.load_markets().await?;

        let index_guard = self.asset_index.read().await;
        let idx = index_guard.get(&hl_name).copied().ok_or_else(|| {
            CcxtError::BadSymbol(format!("Symbol {} not found on Hyperliquid", symbol))
        })?;

        Ok((hl_name, idx))
    }

    /// Load and cache meta if not already cached.
    async fn ensure_meta(&self) -> Result<()> {
        let guard = self.meta.read().await;
        if guard.is_some() {
            return Ok(());
        }
        drop(guard);

        self.load_markets().await?;
        Ok(())
    }

    /// Build the order wire for Hyperliquid.
    #[allow(clippy::too_many_arguments)]
    fn build_order_wire(
        &self,
        asset_index: u32,
        is_buy: bool,
        price: Decimal,
        size: Decimal,
        order_type: OrderType,
        reduce_only: bool,
        params: Option<&Params>,
    ) -> HlOrderWire {
        // Check if this is a trigger order (stop-loss, take-profit)
        let stop_price = params
            .and_then(|p| p.get("stopPrice"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                params
                    .and_then(|p| p.get("triggerPrice"))
                    .and_then(|v| v.as_str())
            });

        let order_type_wire = if let Some(trigger_px) = stop_price {
            let tpsl = params
                .and_then(|p| p.get("triggerType"))
                .and_then(|v| v.as_str())
                .unwrap_or("sl");
            let is_market = params
                .and_then(|p| p.get("triggerIsMarket"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            HlOrderTypeWire::Trigger {
                trigger: HlTriggerOrder {
                    is_market,
                    trigger_px: float_to_wire(trigger_px),
                    tpsl: tpsl.to_string(),
                },
            }
        } else {
            let tif = params
                .and_then(|p| p.get("timeInForce"))
                .and_then(|v| v.as_str())
                .unwrap_or(match order_type {
                    OrderType::Market => "Ioc",
                    _ => "Gtc",
                });

            HlOrderTypeWire::Limit {
                limit: HlLimitOrder {
                    tif: tif.to_string(),
                },
            }
        };

        HlOrderWire {
            a: asset_index,
            b: is_buy,
            p: float_to_wire(&price.to_string()),
            s: float_to_wire(&size.to_string()),
            r: reduce_only,
            t: order_type_wire,
            c: params
                .and_then(|p| p.get("clientOrderId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl Exchange for Hyperliquid {
    fn id(&self) -> &str {
        "hyperliquid"
    }

    fn name(&self) -> &str {
        "Hyperliquid"
    }

    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Dex
    }

    fn has(&self) -> &ExchangeFeatures {
        &self.features
    }

    async fn load_markets(&self) -> Result<Vec<Market>> {
        {
            let guard = self.markets.read().await;
            if let Some(markets) = guard.as_ref() {
                return Ok(markets.clone());
            }
        }

        let markets = self.fetch_markets().await?;

        {
            let mut guard = self.markets.write().await;
            *guard = Some(markets.clone());
        }

        Ok(markets)
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        let json = self.client.info_request("metaAndAssetCtxs", None).await?;

        let arr = json
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("metaAndAssetCtxs: expected array".to_string()))?;

        if arr.len() < 2 {
            return Err(CcxtError::ParseError(
                "metaAndAssetCtxs: expected 2 elements".to_string(),
            ));
        }

        let meta: HlMeta = serde_json::from_value(arr[0].clone())?;
        let asset_ctxs: Vec<HlAssetCtx> = serde_json::from_value(arr[1].clone())?;

        // Update caches
        {
            let mut index_guard = self.asset_index.write().await;
            *index_guard = parsers::build_asset_index(&meta);
        }
        {
            let mut names_guard = self.asset_names.write().await;
            *names_guard = parsers::build_asset_names(&meta);
        }

        let markets = parsers::parse_markets(&meta, Some(&asset_ctxs))?;

        // Cache meta
        {
            let mut meta_guard = self.meta.write().await;
            *meta_guard = Some(meta);
        }

        Ok(markets)
    }

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(CcxtError::NotSupported(
            "fetch_currencies not supported by Hyperliquid (single USDC collateral)".to_string(),
        ))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;

        let mids = self.client.info_request("allMids", None).await?;
        parsers::parse_ticker(&mids, &hl_name)
    }

    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        self.ensure_meta().await?;

        let meta_guard = self.meta.read().await;
        let meta = meta_guard
            .as_ref()
            .ok_or_else(|| CcxtError::ParseError("Meta not loaded".to_string()))?;

        let mids = self.client.info_request("allMids", None).await?;
        let mut tickers = parsers::parse_tickers(&mids, meta)?;

        if let Some(filter) = symbols {
            tickers.retain(|t| filter.contains(&t.symbol.as_str()));
        }

        Ok(tickers)
    }

    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;

        let mut extra = serde_json::json!({ "coin": hl_name });
        if let Some(_limit) = limit {
            // Hyperliquid uses nSigFigs for aggregation, not limit
            // Return raw book (up to 20 levels per side)
            extra["nSigFigs"] = serde_json::Value::Null;
        }

        let json = self
            .client
            .info_request("l2Book", Some(extra))
            .await?;

        let book: HlL2Book = serde_json::from_value(json)?;
        parsers::parse_order_book(&book, &hl_name)
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;
        let interval = parsers::timeframe_to_hyperliquid(timeframe)?;

        let now_ms = timestamp_ms();
        let start_time = since.unwrap_or_else(|| {
            let candle_ms = timeframe.to_milliseconds();
            let count = limit.unwrap_or(100) as i64;
            now_ms - candle_ms * count
        });

        let req = serde_json::json!({
            "req": {
                "coin": hl_name,
                "interval": interval,
                "startTime": start_time,
                "endTime": now_ms,
            }
        });

        let json = self
            .client
            .info_request("candleSnapshot", Some(req))
            .await?;

        let candles: Vec<HlCandle> = serde_json::from_value(json)?;

        let mut ohlcv = parsers::parse_ohlcv(&candles)?;

        if let Some(limit) = limit {
            if ohlcv.len() > limit as usize {
                ohlcv.truncate(limit as usize);
            }
        }

        Ok(ohlcv)
    }

    async fn fetch_trades(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;
        let unified = parsers::symbol_from_hyperliquid(&hl_name);

        let extra = serde_json::json!({ "coin": hl_name });
        let json = self
            .client
            .info_request("recentTrades", Some(extra))
            .await?;

        let raw_trades: Vec<HlRecentTrade> = serde_json::from_value(json)?;
        let mut trades = parsers::parse_trades(&raw_trades, &unified)?;

        if let Some(since_ts) = since {
            trades.retain(|t| t.timestamp >= since_ts);
        }

        if let Some(limit) = limit {
            if trades.len() > limit as usize {
                trades.truncate(limit as usize);
            }
        }

        Ok(trades)
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        // Use meta endpoint as a health check
        let _json = self.client.info_request("meta", None).await?;
        let now = timestamp_ms();

        Ok(ExchangeStatus {
            status: "ok".to_string(),
            updated: now,
            eta: None,
            url: None,
        })
    }

    async fn create_order(
        &self,
        symbol: &str,
        order_type: OrderType,
        side: OrderSide,
        amount: Decimal,
        price: Option<Decimal>,
        params: Option<&Params>,
    ) -> Result<Order> {
        let signer = self.require_signer()?;
        let (hl_name, asset_idx) = self.resolve_symbol(symbol).await?;

        let is_buy = matches!(side, OrderSide::Buy);
        let reduce_only = params
            .and_then(|p| p.get("reduceOnly"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // For market orders, simulate with aggressive IOC limit price
        let effective_price = match order_type {
            OrderType::Market => {
                // Get mid price for slippage calculation
                let mids = self.client.info_request("allMids", None).await?;
                let mid_str = mids
                    .get(&hl_name)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        CcxtError::BadSymbol(format!("No mid price for {}", hl_name))
                    })?;
                let mid = Decimal::from_str(mid_str).map_err(|e| {
                    CcxtError::ParseError(format!("Invalid mid price: {}", e))
                })?;

                // 5% slippage for market orders
                let slippage = Decimal::from_str("0.05").unwrap();
                if is_buy {
                    mid * (Decimal::ONE + slippage)
                } else {
                    mid * (Decimal::ONE - slippage)
                }
            }
            _ => price.ok_or_else(|| {
                CcxtError::InvalidOrder("Price required for limit orders".to_string())
            })?,
        };

        let order_wire =
            self.build_order_wire(asset_idx, is_buy, effective_price, amount, order_type, reduce_only, params);

        let action = serde_json::json!({
            "type": "order",
            "orders": [order_wire],
            "grouping": "na",
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        // Parse the response status
        let statuses = response
            .get("response")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.get("statuses"))
            .and_then(|s| s.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing statuses in order response".to_string()))?;

        if statuses.is_empty() {
            return Err(CcxtError::ParseError(
                "Empty statuses in order response".to_string(),
            ));
        }

        let status_entry: HlOrderStatusEntry = serde_json::from_value(statuses[0].clone())?;
        let unified_symbol = parsers::symbol_from_hyperliquid(&hl_name);
        parsers::parse_order_response(&status_entry, &unified_symbol, side, order_type, amount, price)
    }

    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order> {
        let signer = self.require_signer()?;

        let symbol = symbol.ok_or_else(|| {
            CcxtError::BadRequest("Symbol required for cancel_order on Hyperliquid".to_string())
        })?;

        let (_hl_name, asset_idx) = self.resolve_symbol(symbol).await?;
        let oid: u64 = id
            .parse()
            .map_err(|_| CcxtError::OrderNotFound(format!("Invalid order ID: {}", id)))?;

        let action = serde_json::json!({
            "type": "cancel",
            "cancels": [{ "a": asset_idx, "o": oid }],
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        // Check for errors in statuses
        let statuses = response
            .get("response")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.get("statuses"))
            .and_then(|s| s.as_array());

        if let Some(statuses) = statuses {
            if let Some(first) = statuses.first() {
                if let Some(err_obj) = first.as_object() {
                    if let Some(err) = err_obj.get("error").and_then(|e| e.as_str()) {
                        return Err(CcxtError::OrderNotFound(err.to_string()));
                    }
                }
            }
        }

        let now = timestamp_ms();
        Ok(Order {
            id: id.to_string(),
            client_order_id: None,
            symbol: symbol.to_string(),
            order_type: OrderType::Limit,
            side: OrderSide::Buy, // Unknown for cancel
            status: OrderStatus::Canceled,
            timestamp: now,
            datetime: crate::base::signer::timestamp_to_iso8601(now),
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
            info: None,
        })
    }

    async fn edit_order(
        &self,
        id: &str,
        symbol: &str,
        order_type: OrderType,
        side: OrderSide,
        amount: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<Order> {
        let signer = self.require_signer()?;
        let (_hl_name, asset_idx) = self.resolve_symbol(symbol).await?;

        let oid: u64 = id
            .parse()
            .map_err(|_| CcxtError::OrderNotFound(format!("Invalid order ID: {}", id)))?;

        let is_buy = matches!(side, OrderSide::Buy);
        let amt = amount.unwrap_or(Decimal::ZERO);
        let px = price.unwrap_or(Decimal::ZERO);

        let order_wire = self.build_order_wire(asset_idx, is_buy, px, amt, order_type, false, None);

        let action = serde_json::json!({
            "type": "modify",
            "oid": oid,
            "order": order_wire,
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        // Check for error
        if response.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = response
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Unknown modify error");
            return Err(CcxtError::InvalidOrder(msg.to_string()));
        }

        let now = timestamp_ms();
        Ok(Order {
            id: id.to_string(),
            client_order_id: None,
            symbol: symbol.to_string(),
            order_type,
            side,
            status: OrderStatus::Open,
            timestamp: now,
            datetime: crate::base::signer::timestamp_to_iso8601(now),
            last_trade_timestamp: None,
            price,
            average: None,
            amount: amt,
            filled: None,
            remaining: amount,
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
            info: None,
        })
    }

    async fn fetch_order(&self, id: &str, _symbol: Option<&str>) -> Result<Order> {
        let user = self.user_address()?;

        let oid: u64 = id
            .parse()
            .map_err(|_| CcxtError::OrderNotFound(format!("Invalid order ID: {}", id)))?;

        let extra = serde_json::json!({
            "user": user,
            "oid": oid,
        });

        let json = self
            .client
            .info_request("orderStatus", Some(extra))
            .await?;

        let status_resp: HlOrderStatusResponse = serde_json::from_value(json)?;
        parsers::parse_order_status(&status_resp)
    }

    async fn fetch_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let user = self.user_address()?;

        let extra = serde_json::json!({ "user": user });
        let json = self
            .client
            .info_request("frontendOpenOrders", Some(extra))
            .await?;

        let raw_orders: Vec<HlFrontendOpenOrder> = serde_json::from_value(json)?;
        let mut orders = parsers::parse_frontend_orders(&raw_orders)?;

        if let Some(sym) = symbol {
            orders.retain(|o| o.symbol == sym);
        }

        if let Some(since_ts) = since {
            orders.retain(|o| o.timestamp >= since_ts);
        }

        if let Some(limit) = limit {
            if orders.len() > limit as usize {
                orders.truncate(limit as usize);
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
        let user = self.user_address()?;

        let extra = serde_json::json!({ "user": user });
        let json = self
            .client
            .info_request("openOrders", Some(extra))
            .await?;

        let raw_orders: Vec<HlOpenOrder> = serde_json::from_value(json)?;
        let mut orders = parsers::parse_open_orders(&raw_orders)?;

        if let Some(sym) = symbol {
            orders.retain(|o| o.symbol == sym);
        }

        Ok(orders)
    }

    async fn fetch_closed_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        let user = self.user_address()?;

        let extra = serde_json::json!({
            "user": user,
            "aggregateByTime": false,
        });

        let json = self
            .client
            .info_request("userFills", Some(extra))
            .await?;

        let raw_fills: Vec<HlUserFill> = serde_json::from_value(json)?;
        let mut orders = parsers::parse_closed_orders_from_fills(&raw_fills)?;

        if let Some(sym) = symbol {
            orders.retain(|o| o.symbol == sym);
        }

        if let Some(since_ts) = since {
            orders.retain(|o| o.timestamp >= since_ts);
        }

        if let Some(limit) = limit {
            if orders.len() > limit as usize {
                orders.truncate(limit as usize);
            }
        }

        Ok(orders)
    }

    async fn fetch_my_trades(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let user = self.user_address()?;

        let extra = serde_json::json!({
            "user": user,
            "aggregateByTime": false,
        });

        let json = self
            .client
            .info_request("userFills", Some(extra))
            .await?;

        let raw_fills: Vec<HlUserFill> = serde_json::from_value(json)?;
        let mut trades = parsers::parse_user_fills(&raw_fills)?;

        if let Some(sym) = symbol {
            trades.retain(|t| t.symbol == sym);
        }

        if let Some(since_ts) = since {
            trades.retain(|t| t.timestamp >= since_ts);
        }

        if let Some(limit) = limit {
            if trades.len() > limit as usize {
                trades.truncate(limit as usize);
            }
        }

        Ok(trades)
    }

    async fn fetch_balance(&self) -> Result<Balances> {
        let user = self.user_address()?;
        let extra = serde_json::json!({ "user": user });

        let json = self
            .client
            .info_request("clearinghouseState", Some(extra))
            .await?;

        let state: HlClearinghouseState = serde_json::from_value(json)?;
        parsers::parse_balances(&state)
    }

    async fn fetch_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported(
            "fetch_deposit_address not supported by Hyperliquid".to_string(),
        ))
    }

    async fn fetch_deposits(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Deposit>> {
        Err(CcxtError::NotSupported(
            "fetch_deposits not supported by Hyperliquid".to_string(),
        ))
    }

    async fn fetch_withdrawals(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>> {
        Err(CcxtError::NotSupported(
            "fetch_withdrawals not supported by Hyperliquid".to_string(),
        ))
    }

    async fn withdraw(
        &self,
        _code: &str,
        amount: Decimal,
        address: &str,
        _tag: Option<&str>,
    ) -> Result<Withdrawal> {
        let signer = self.require_signer()?;
        let nonce = timestamp_ms() as u64;

        // Hyperliquid withdraw3 action
        let action = serde_json::json!({
            "type": "withdraw3",
            "hyperliquidChain": if signer.address_hex().is_empty() { "Testnet" } else { "Mainnet" },
            "signatureChainId": format!("0x{:x}", constants::USER_SIGNED_CHAIN_ID),
            "destination": address,
            "amount": amount.to_string(),
            "time": nonce,
        });

        let sig = signer.sign_withdraw(address, &amount.to_string(), nonce).await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        if response.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = response
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Withdrawal failed");
            return Err(CcxtError::ExchangeError(msg.to_string()));
        }

        let now = timestamp_ms();
        Ok(Withdrawal {
            id: nonce.to_string(),
            txid: None,
            timestamp: now,
            datetime: crate::base::signer::timestamp_to_iso8601(now),
            network: Some("Hyperliquid".to_string()),
            address: address.to_string(),
            tag: None,
            transaction_type: TransactionType::Withdrawal,
            amount,
            currency: "USDC".to_string(),
            status: TransactionStatus::Pending,
            updated: None,
            fee: None,
            info: None,
        })
    }

    async fn transfer(
        &self,
        _code: &str,
        amount: Decimal,
        _from_account: &str,
        to_account: &str,
    ) -> Result<Transfer> {
        let signer = self.require_signer()?;
        let nonce = timestamp_ms() as u64;

        let action = serde_json::json!({
            "type": "usdClassTransfer",
            "hyperliquidChain": "Mainnet",
            "signatureChainId": format!("0x{:x}", constants::USER_SIGNED_CHAIN_ID),
            "destination": to_account,
            "amount": amount.to_string(),
            "time": nonce,
        });

        let sig = signer
            .sign_usd_transfer(to_account, &amount.to_string(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        if response.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = response
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Transfer failed");
            return Err(CcxtError::ExchangeError(msg.to_string()));
        }

        let now = timestamp_ms();
        Ok(Transfer {
            id: nonce.to_string(),
            timestamp: now,
            datetime: crate::base::signer::timestamp_to_iso8601(now),
            currency: "USDC".to_string(),
            amount,
            from_account: "perp".to_string(),
            to_account: to_account.to_string(),
            status: TransactionStatus::Ok,
            info: None,
        })
    }

    async fn fetch_positions(&self, symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        let user = self.user_address()?;
        let extra = serde_json::json!({ "user": user });

        let json = self
            .client
            .info_request("clearinghouseState", Some(extra))
            .await?;

        let state: HlClearinghouseState = serde_json::from_value(json)?;
        let mut positions = parsers::parse_positions(&state)?;

        if let Some(filter) = symbols {
            positions.retain(|p| filter.contains(&p.symbol.as_str()));
        }

        Ok(positions)
    }

    async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;
        let unified = parsers::symbol_from_hyperliquid(&hl_name);

        // Try to get from metaAndAssetCtxs first (live data)
        let json = self.client.info_request("metaAndAssetCtxs", None).await?;
        let arr = json.as_array().ok_or_else(|| {
            CcxtError::ParseError("metaAndAssetCtxs: expected array".to_string())
        })?;

        if arr.len() >= 2 {
            let meta: HlMeta = serde_json::from_value(arr[0].clone())?;
            let ctxs: Vec<HlAssetCtx> = serde_json::from_value(arr[1].clone())?;

            for (i, asset) in meta.universe.iter().enumerate() {
                if asset.name == hl_name {
                    if let Some(ctx) = ctxs.get(i) {
                        return parsers::parse_funding_rate_from_ctx(ctx, &unified);
                    }
                }
            }
        }

        // Fallback to fundingHistory
        let now_ms = timestamp_ms();
        let extra = serde_json::json!({
            "coin": hl_name,
            "startTime": now_ms - 86_400_000,
            "endTime": now_ms,
        });

        let json = self
            .client
            .info_request("fundingHistory", Some(extra))
            .await?;

        let entries: Vec<HlFundingEntry> = serde_json::from_value(json)?;
        parsers::parse_funding_rate(&entries, &unified)
    }

    async fn fetch_funding_rate_history(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRateHistory>> {
        let (hl_name, _) = self.resolve_symbol(symbol).await?;
        let unified = parsers::symbol_from_hyperliquid(&hl_name);

        let now_ms = timestamp_ms();
        let start_time = since.unwrap_or(now_ms - 7 * 86_400_000); // Default: 7 days

        let extra = serde_json::json!({
            "coin": hl_name,
            "startTime": start_time,
            "endTime": now_ms,
        });

        let json = self
            .client
            .info_request("fundingHistory", Some(extra))
            .await?;

        let entries: Vec<HlFundingEntry> = serde_json::from_value(json)?;
        let mut history = parsers::parse_funding_rate_history(&entries, &unified)?;

        if let Some(limit) = limit {
            if history.len() > limit as usize {
                // Keep the most recent entries
                let start = history.len() - limit as usize;
                history = history.split_off(start);
            }
        }

        Ok(history)
    }

    async fn set_leverage(&self, leverage: u32, symbol: &str) -> Result<()> {
        let signer = self.require_signer()?;
        let (_hl_name, asset_idx) = self.resolve_symbol(symbol).await?;

        let action = serde_json::json!({
            "type": "updateLeverage",
            "asset": asset_idx,
            "isCross": true,
            "leverage": leverage,
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        if response.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = response
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Set leverage failed");
            return Err(CcxtError::ExchangeError(msg.to_string()));
        }

        Ok(())
    }

    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()> {
        let signer = self.require_signer()?;
        let (_hl_name, asset_idx) = self.resolve_symbol(symbol).await?;

        let is_cross = matches!(mode, MarginMode::Cross);

        // Hyperliquid combines margin mode with leverage in updateLeverage
        // We need to fetch current leverage to preserve it
        let action = serde_json::json!({
            "type": "updateLeverage",
            "asset": asset_idx,
            "isCross": is_cross,
            "leverage": 10, // Default, user should call set_leverage separately
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        if response.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = response
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Set margin mode failed");
            return Err(CcxtError::ExchangeError(msg.to_string()));
        }

        Ok(())
    }

    async fn create_orders(&self, orders: &[OrderRequest]) -> Result<Vec<Order>> {
        let signer = self.require_signer()?;

        let mut order_wires = Vec::with_capacity(orders.len());
        let mut order_metas = Vec::with_capacity(orders.len());

        for req in orders {
            let (hl_name, asset_idx) = self.resolve_symbol(&req.symbol).await?;
            let is_buy = matches!(req.side, OrderSide::Buy);
            let reduce_only = req
                .params
                .as_ref()
                .and_then(|p| p.get("reduceOnly"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let effective_price = match req.order_type {
                OrderType::Market => {
                    let mids = self.client.info_request("allMids", None).await?;
                    let mid_str = mids
                        .get(&hl_name)
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| CcxtError::BadSymbol(format!("No mid price for {}", hl_name)))?;
                    let mid = Decimal::from_str(mid_str)
                        .map_err(|e| CcxtError::ParseError(format!("Invalid mid price: {}", e)))?;
                    let slippage = Decimal::from_str("0.05").unwrap();
                    if is_buy {
                        mid * (Decimal::ONE + slippage)
                    } else {
                        mid * (Decimal::ONE - slippage)
                    }
                }
                _ => req.price.ok_or_else(|| {
                    CcxtError::InvalidOrder("Price required for limit orders".to_string())
                })?,
            };

            let wire = self.build_order_wire(
                asset_idx,
                is_buy,
                effective_price,
                req.amount,
                req.order_type,
                reduce_only,
                req.params.as_ref(),
            );
            order_wires.push(wire);
            order_metas.push((req.symbol.clone(), req.side, req.order_type, req.amount, req.price));
        }

        let action = serde_json::json!({
            "type": "order",
            "orders": order_wires,
            "grouping": "na",
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        let statuses = response
            .get("response")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.get("statuses"))
            .and_then(|s| s.as_array())
            .ok_or_else(|| CcxtError::ParseError("Missing statuses in batch order response".to_string()))?;

        let mut result = Vec::with_capacity(statuses.len());
        for (i, status_val) in statuses.iter().enumerate() {
            let status_entry: HlOrderStatusEntry = serde_json::from_value(status_val.clone())?;
            let (ref symbol, side, order_type, amount, price) = order_metas[i];
            let order =
                parsers::parse_order_response(&status_entry, symbol, side, order_type, amount, price)?;
            result.push(order);
        }

        Ok(result)
    }

    async fn cancel_orders(
        &self,
        ids: &[&str],
        symbol: Option<&str>,
    ) -> Result<Vec<Order>> {
        let signer = self.require_signer()?;
        let symbol = symbol.ok_or_else(|| {
            CcxtError::BadRequest("Symbol required for cancel_orders on Hyperliquid".to_string())
        })?;
        let (_hl_name, asset_idx) = self.resolve_symbol(symbol).await?;

        let cancels: Vec<HlCancelWire> = ids
            .iter()
            .map(|id| {
                let oid: u64 = id
                    .parse()
                    .map_err(|_| CcxtError::OrderNotFound(format!("Invalid order ID: {}", id)))?;
                Ok(HlCancelWire { a: asset_idx, o: oid })
            })
            .collect::<Result<Vec<_>>>()?;

        let action = serde_json::json!({
            "type": "cancel",
            "cancels": cancels,
        });

        let nonce = timestamp_ms() as u64;
        let sig = signer
            .sign_l1_action(&action, self.vault_address.as_deref(), nonce)
            .await?;

        let _response = self
            .client
            .exchange_request(action, nonce, sig.to_json(), self.vault_address.as_deref())
            .await?;

        let now = timestamp_ms();
        Ok(ids
            .iter()
            .map(|id| Order {
                id: id.to_string(),
                client_order_id: None,
                symbol: symbol.to_string(),
                order_type: OrderType::Limit,
                side: OrderSide::Buy,
                status: OrderStatus::Canceled,
                timestamp: now,
                datetime: crate::base::signer::timestamp_to_iso8601(now),
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
                info: None,
            })
            .collect())
    }

    async fn create_stop_loss_order(
        &self,
        symbol: &str,
        _order_type: OrderType,
        side: OrderSide,
        amount: Decimal,
        _price: Option<Decimal>,
        stop_loss_price: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        let mut trigger_params = params.cloned().unwrap_or_default();
        trigger_params.insert(
            "stopPrice".to_string(),
            serde_json::Value::String(stop_loss_price.to_string()),
        );
        trigger_params.insert(
            "triggerType".to_string(),
            serde_json::Value::String("sl".to_string()),
        );

        self.create_order(
            symbol,
            OrderType::Limit,
            side,
            amount,
            Some(stop_loss_price),
            Some(&trigger_params),
        )
        .await
    }

    async fn create_take_profit_order(
        &self,
        symbol: &str,
        _order_type: OrderType,
        side: OrderSide,
        amount: Decimal,
        _price: Option<Decimal>,
        take_profit_price: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        let mut trigger_params = params.cloned().unwrap_or_default();
        trigger_params.insert(
            "stopPrice".to_string(),
            serde_json::Value::String(take_profit_price.to_string()),
        );
        trigger_params.insert(
            "triggerType".to_string(),
            serde_json::Value::String("tp".to_string()),
        );

        self.create_order(
            symbol,
            OrderType::Limit,
            side,
            amount,
            Some(take_profit_price),
            Some(&trigger_params),
        )
        .await
    }

    async fn fetch_leverage_tiers(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, Vec<LeverageTier>>> {
        self.ensure_meta().await?;

        let meta_guard = self.meta.read().await;
        let meta = meta_guard
            .as_ref()
            .ok_or_else(|| CcxtError::ParseError("Meta not loaded".to_string()))?;

        let mut tiers = parsers::parse_leverage_tiers(meta);

        if let Some(filter) = symbols {
            tiers.retain(|k, _| filter.contains(&k.as_str()));
        }

        Ok(tiers)
    }
}
