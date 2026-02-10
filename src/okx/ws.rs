//! OKX WebSocket implementation
//!
//! Public: `wss://ws.okx.com:8443/ws/v5/public`
//! Private: `wss://ws.okx.com:8443/ws/v5/private`
//!
//! Auth: `{"op":"login","args":[{"apiKey":"...","passphrase":"...","timestamp":"...","sign":"..."}]}`
//! Ping: plain text "ping" / "pong"
//! Subscribe: `{"op":"subscribe","args":[{"channel":"tickers","instId":"BTC-USDT"}]}`
//!
//! ## Orderbook Checksum Validation & Recovery
//!
//! OKX sends a `checksum` field with each orderbook update. This implementation
//! validates the local orderbook state against the checksum to detect out-of-sync conditions.
//!
//! When a checksum mismatch is detected:
//! 1. If `WsConfig::auto_recovery_enabled` is true (default), automatic recovery is triggered
//! 2. The implementation unsubscribes and re-subscribes to the orderbook stream
//! 3. The exchange sends a fresh snapshot, resetting the local state
//! 4. Exponential backoff is used for retry delays (1s, 2s, 4s, 8s, max 30s)
//! 5. Recovery attempts are limited by `WsConfig::max_recovery_attempts` (default: 5)
//!
//! Recovery can be disabled by setting `config.auto_recovery_enabled = false`, in which
//! case checksum mismatches are only logged as warnings.

use crate::base::errors::{CcxtError, Result};
use crate::base::local_orderbook::LocalOrderBook;
use crate::base::orderbook_recovery::OrderbookRecovery;
use crate::base::signer::{hmac_sha256_base64, timestamp_ms, timestamp_s, timestamp_to_iso8601};
use crate::base::ws::{ExchangeWs, NowOrNever, SubscriptionId, WsConfig, WsConnectionState, WsStream};
use crate::base::ws_connection::{WsConnectionManager, MessageHandler};
use crate::okx::parsers;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::Instant;

const OKX_WS_PUBLIC: &str = "wss://ws.okx.com:8443/ws/v5/public";
const OKX_WS_PRIVATE: &str = "wss://ws.okx.com:8443/ws/v5/private";
const OKX_WS_PUBLIC_SANDBOX: &str = "wss://wspap.okx.com:8443/ws/v5/public?brokerId=9999";
const OKX_WS_PRIVATE_SANDBOX: &str = "wss://wspap.okx.com:8443/ws/v5/private?brokerId=9999";

/// Parse price levels from OKX orderbook JSON array
fn parse_levels(value: Option<&serde_json::Value>) -> Vec<(Decimal, Decimal)> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let level = item.as_array()?;
                    let price = level.first()?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                    let amount = level.get(1)?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                    Some((price, amount))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// OKX checksum format: interleave ask and bid levels (top 25)
/// "bid1_price:bid1_amount:ask1_price:ask1_amount:..."
fn okx_checksum_format(lob: &LocalOrderBook) -> String {
    let bids = lob.bids();
    let asks = lob.asks();
    let mut parts = Vec::new();
    for i in 0..25 {
        if let Some((p, a)) = bids.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
        if let Some((p, a)) = asks.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
    }
    parts.join(":")
}

/// OKX WebSocket client
pub struct OkxWs {
    public_conn: Arc<WsConnectionManager>,
    private_conn: Arc<RwLock<Option<WsConnectionManager>>>,

    ticker_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Ticker>>>>,
    orderbook_senders: Arc<RwLock<HashMap<String, broadcast::Sender<OrderBook>>>>,
    trade_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Trade>>>>,
    ohlcv_senders: Arc<RwLock<HashMap<String, broadcast::Sender<OHLCV>>>>,

    order_sender: broadcast::Sender<Order>,
    balance_sender: broadcast::Sender<Balances>,
    position_sender: broadcast::Sender<Vec<Position>>,
    my_trade_sender: broadcast::Sender<Trade>,

    local_orderbooks: Arc<RwLock<HashMap<String, Arc<RwLock<LocalOrderBook>>>>>,
    recovery_state: Arc<RwLock<HashMap<String, OrderbookRecovery>>>,

    config: WsConfig,
    sandbox: bool,
    api_key: Option<String>,
    secret: Option<String>,
    passphrase: Option<String>,
}

impl OkxWs {
    /// Create a new OKX WebSocket client
    pub fn new(sandbox: bool, config: WsConfig) -> Self {
        let ws_url = if sandbox {
            OKX_WS_PUBLIC_SANDBOX
        } else {
            OKX_WS_PUBLIC
        };

        // OKX uses plain text "ping" for keepalive
        let public_conn = WsConnectionManager::new(ws_url, config.clone())
            .with_ping_message("ping");

        let (order_tx, _) = broadcast::channel(config.channel_capacity);
        let (balance_tx, _) = broadcast::channel(config.channel_capacity);
        let (position_tx, _) = broadcast::channel(config.channel_capacity);
        let (my_trade_tx, _) = broadcast::channel(config.channel_capacity);

        Self {
            public_conn: Arc::new(public_conn),
            private_conn: Arc::new(RwLock::new(None)),
            ticker_senders: Arc::new(RwLock::new(HashMap::new())),
            orderbook_senders: Arc::new(RwLock::new(HashMap::new())),
            trade_senders: Arc::new(RwLock::new(HashMap::new())),
            ohlcv_senders: Arc::new(RwLock::new(HashMap::new())),
            order_sender: order_tx,
            balance_sender: balance_tx,
            position_sender: position_tx,
            my_trade_sender: my_trade_tx,
            local_orderbooks: Arc::new(RwLock::new(HashMap::new())),
            recovery_state: Arc::new(RwLock::new(HashMap::new())),
            config,
            sandbox,
            api_key: None,
            secret: None,
            passphrase: None,
        }
    }

    /// Set API credentials for private streams
    pub fn with_credentials(mut self, api_key: String, secret: String, passphrase: String) -> Self {
        self.api_key = Some(api_key);
        self.secret = Some(secret);
        self.passphrase = Some(passphrase);
        self
    }

    /// Convert unified symbol to OKX instId format
    fn inst_id(symbol: &str) -> String {
        parsers::symbol_to_okx(symbol)
    }

    /// Build subscribe message
    fn subscribe_msg(channel: &str, inst_id: &str) -> String {
        format!(
            r#"{{"op":"subscribe","args":[{{"channel":"{}","instId":"{}"}}]}}"#,
            channel, inst_id
        )
    }

    /// Build unsubscribe message
    fn unsubscribe_msg(channel: &str, inst_id: &str) -> String {
        format!(
            r#"{{"op":"unsubscribe","args":[{{"channel":"{}","instId":"{}"}}]}}"#,
            channel, inst_id
        )
    }

    /// Build auth/login message
    fn build_login_message(api_key: &str, secret: &str, passphrase: &str) -> Result<String> {
        let timestamp = timestamp_s().to_string();
        let sign_str = format!("{}GET/users/self/verify", timestamp);
        let signature = hmac_sha256_base64(secret, &sign_str)?;
        Ok(format!(
            r#"{{"op":"login","args":[{{"apiKey":"{}","passphrase":"{}","timestamp":"{}","sign":"{}"}}]}}"#,
            api_key, passphrase, timestamp, signature
        ))
    }

    /// Ensure the private WebSocket connection is established.
    async fn ensure_private_connection(&self) -> Result<()> {
        {
            let guard = self.private_conn.read().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        let api_key = self.api_key.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("API key required for private streams".to_string())
        })?;
        let secret = self.secret.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("Secret required for private streams".to_string())
        })?;
        let passphrase = self.passphrase.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("Passphrase required for private streams".to_string())
        })?;

        let private_url = if self.sandbox {
            OKX_WS_PRIVATE_SANDBOX
        } else {
            OKX_WS_PRIVATE
        };

        let auth_msg = Self::build_login_message(api_key, secret, passphrase)?;

        let private_conn = WsConnectionManager::new(private_url, self.config.clone())
            .with_ping_message("ping");

        // Set auth message so it's sent on connect and reconnect
        private_conn.set_auth_message(auth_msg).await;

        // Set up private handler
        self.setup_private_handler(&private_conn).await;

        // Connect (will auto-send auth)
        private_conn.connect().await?;

        // Subscribe to private channels
        let sub_msg = format!(
            r#"{{"op":"subscribe","args":[{{"channel":"orders","instType":"ANY"}},{{"channel":"account"}},{{"channel":"positions","instType":"ANY"}},{{"channel":"orders-algo","instType":"ANY"}}]}}"#
        );
        private_conn.send_raw(sub_msg).await?;

        {
            let mut guard = self.private_conn.write().await;
            *guard = Some(private_conn);
        }

        Ok(())
    }

    /// Set up the message handler for the private connection.
    async fn setup_private_handler(&self, conn: &WsConnectionManager) {
        let order_sender = self.order_sender.clone();
        let balance_sender = self.balance_sender.clone();
        let position_sender = self.position_sender.clone();
        let my_trade_sender = self.my_trade_sender.clone();
        let last_pong = conn.last_pong_handle();

        let handler: MessageHandler = Arc::new(move |text: String| {
            // OKX sends "pong" as plain text response to "ping"
            if text == "pong" {
                *last_pong.blocking_write() = Some(Instant::now());
                return;
            }

            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            // Check for data array
            let data = match json.get("data").and_then(|v| v.as_array()) {
                Some(d) => d,
                None => return,
            };

            let channel = json
                .get("arg")
                .and_then(|a| a.get("channel"))
                .and_then(|c| c.as_str())
                .unwrap_or("");

            match channel {
                "orders" | "orders-algo" => {
                    for item in data {
                        let inst_id = item.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                        let symbol = parsers::symbol_from_okx(inst_id);
                        if let Ok(order) = parsers::parse_order(item, &symbol) {
                            let _ = order_sender.send(order);
                        }
                    }
                }
                "account" => {
                    let now = timestamp_ms();
                    let mut balances_map = HashMap::new();
                    let mut free_map = HashMap::new();
                    let mut used_map = HashMap::new();
                    let mut total_map = HashMap::new();

                    for item in data {
                        if let Some(details) = item.get("details").and_then(|v| v.as_array()) {
                            for detail in details {
                                let ccy = detail.get("ccy").and_then(|v| v.as_str()).unwrap_or("");
                                let available = detail.get("availBal").and_then(|v| v.as_str())
                                    .and_then(|s| Decimal::from_str(s).ok())
                                    .unwrap_or(Decimal::ZERO);
                                let frozen = detail.get("frozenBal").and_then(|v| v.as_str())
                                    .and_then(|s| Decimal::from_str(s).ok())
                                    .unwrap_or(Decimal::ZERO);
                                let eq = detail.get("eq").and_then(|v| v.as_str())
                                    .and_then(|s| Decimal::from_str(s).ok())
                                    .unwrap_or(available + frozen);

                                balances_map.insert(
                                    ccy.to_string(),
                                    Balance::new(ccy.to_string(), available, frozen),
                                );
                                free_map.insert(ccy.to_string(), available);
                                used_map.insert(ccy.to_string(), frozen);
                                total_map.insert(ccy.to_string(), eq);
                            }
                        }
                    }

                    if !balances_map.is_empty() {
                        let _ = balance_sender.send(Balances {
                            timestamp: now,
                            datetime: timestamp_to_iso8601(now),
                            balances: balances_map,
                            free: free_map,
                            used: used_map,
                            total: total_map,
                            info: None,
                        });
                    }
                }
                "positions" => {
                    let mut positions = Vec::new();
                    for item in data {
                        if let Ok(pos) = parsers::parse_position(item) {
                            positions.push(pos);
                        }
                    }
                    if !positions.is_empty() {
                        let _ = position_sender.send(positions);
                    }
                }
                _ => {}
            }

            // Also check for fills within order updates (execution events)
            if channel == "orders" {
                for item in data {
                    // If fillSz > 0, this is a fill event → send as my_trade
                    let fill_sz = item.get("fillSz").and_then(|v| v.as_str())
                        .and_then(|s| Decimal::from_str(s).ok())
                        .unwrap_or(Decimal::ZERO);
                    if fill_sz > Decimal::ZERO {
                        let inst_id = item.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                        let symbol = parsers::symbol_from_okx(inst_id);
                        if let Ok(trade) = parsers::parse_my_trade(item, &symbol) {
                            let _ = my_trade_sender.send(trade);
                        }
                    }
                }
            }
        });

        conn.set_message_handler(handler).await;
    }

    /// Setup public message handler
    async fn setup_public_handler(&self) {
        let ticker_senders = self.ticker_senders.clone();
        let orderbook_senders = self.orderbook_senders.clone();
        let trade_senders = self.trade_senders.clone();
        let local_orderbooks = self.local_orderbooks.clone();
        let recovery_state = self.recovery_state.clone();
        let config = self.config.clone();
        let public_conn = self.public_conn.clone();
        let last_pong = self.public_conn.last_pong_handle();

        let handler: MessageHandler = Arc::new(move |text: String| {
            // OKX sends "pong" as plain text response to "ping"
            if text == "pong" {
                *last_pong.blocking_write() = Some(Instant::now());
                return;
            }

            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            // Check for data array
            let data = match json.get("data").and_then(|v| v.as_array()) {
                Some(d) => d,
                None => return,
            };

            let channel = json
                .get("arg")
                .and_then(|a| a.get("channel"))
                .and_then(|c| c.as_str())
                .unwrap_or("");

            let inst_id = json
                .get("arg")
                .and_then(|a| a.get("instId"))
                .and_then(|c| c.as_str())
                .unwrap_or("");

            let symbol = parsers::symbol_from_okx(inst_id);

            match channel {
                "tickers" => {
                    for item in data {
                        if let Ok(ticker) = parsers::parse_ticker(item, &symbol) {
                            let senders = ticker_senders.blocking_read();
                            if let Some(tx) = senders.get(&symbol) {
                                let _ = tx.send(ticker);
                            }
                        }
                    }
                }
                "books5" | "books" | "books50-l2-tbt" => {
                    // OKX sends snapshot vs update via "action" field
                    let action = json.get("action").and_then(|v| v.as_str()).unwrap_or("snapshot");

                    if let Some(item) = data.first() {
                        let bids = parse_levels(item.get("bids"));
                        let asks = parse_levels(item.get("asks"));
                        let nonce = item.get("seqId").and_then(|v| v.as_u64());
                        let timestamp = item.get("ts")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or_else(|| timestamp_ms());

                        // Update local orderbook
                        let lobs = local_orderbooks.blocking_read();
                        if let Some(lob) = lobs.get(&symbol) {
                            let mut book = lob.blocking_write();

                            match action {
                                "snapshot" => {
                                    book.reset(bids, asks, nonce, timestamp);
                                }
                                "update" => {
                                    book.update_bids(&bids);
                                    book.update_asks(&asks);
                                    if let Some(n) = nonce {
                                        book.set_nonce(n);
                                    }
                                    book.set_timestamp(timestamp);
                                }
                                _ => {}
                            }

                            // Validate checksum if present
                            if let Some(cs) = item.get("checksum").and_then(|v| v.as_i64()) {
                                let valid = book.validate_checksum(cs as u32, okx_checksum_format);
                                if valid {
                                    // Checksum valid - reset recovery state
                                    let mut recoveries = recovery_state.blocking_write();
                                    if let Some(recovery) = recoveries.get_mut(&symbol) {
                                        recovery.reset();
                                    }
                                } else if config.auto_recovery_enabled {
                                    // Checksum mismatch - trigger recovery
                                    tracing::warn!("OKX orderbook checksum mismatch for {}, triggering recovery", symbol);

                                    let should_recover = {
                                        let mut recoveries = recovery_state.blocking_write();
                                        let recovery = recoveries.entry(symbol.clone())
                                            .or_insert_with(|| OrderbookRecovery::new(config.max_recovery_attempts));

                                        recovery.record_failure()
                                    };

                                    if let Some(delay) = should_recover {
                                        let symbol_owned = symbol.clone();
                                        let public_conn_clone = public_conn.clone();
                                        let failure_count = {
                                            recovery_state.blocking_read()
                                                .get(&symbol)
                                                .map(|r| r.failure_count())
                                                .unwrap_or(0)
                                        };

                                        // Spawn recovery task in detached thread to avoid lifetime issues
                                        std::thread::spawn(move || {
                                            let rt = tokio::runtime::Handle::try_current();
                                            if let Ok(handle) = rt {
                                                handle.spawn(async move {
                                                    // Wait backoff delay
                                                    tokio::time::sleep(delay).await;

                                                    tracing::info!(
                                                        "Attempting orderbook recovery for {} (attempt {})",
                                                        symbol_owned,
                                                        failure_count
                                                    );

                                                    // Trigger re-subscription
                                                    let okx_id = parsers::symbol_to_okx(&symbol_owned);
                                                    let channel = "books5";

                                                    // Unsubscribe
                                                    let unsub_msg = format!(
                                                        r#"{{"op":"unsubscribe","args":[{{"channel":"{}","instId":"{}"}}]}}"#,
                                                        channel, okx_id
                                                    );
                                                    if let Err(e) = public_conn_clone.send_raw(unsub_msg).await {
                                                        tracing::error!("Failed to unsubscribe during recovery: {}", e);
                                                        return;
                                                    }

                                                    // Brief delay
                                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                                                    // Re-subscribe
                                                    let sub_msg = format!(
                                                        r#"{{"op":"subscribe","args":[{{"channel":"{}","instId":"{}"}}]}}"#,
                                                        channel, okx_id
                                                    );
                                                    let sub_id = SubscriptionId(format!("{}:{}", channel, okx_id));
                                                    if let Err(e) = public_conn_clone.subscribe(sub_id, sub_msg).await {
                                                        tracing::error!("Failed to re-subscribe during recovery: {}", e);
                                                    }
                                                });
                                            }
                                        });
                                    } else {
                                        tracing::error!(
                                            "Max recovery attempts ({}) reached for {}, stopping recovery",
                                            config.max_recovery_attempts,
                                            symbol
                                        );
                                    }
                                } else {
                                    // Recovery disabled, just log
                                    tracing::warn!("OKX orderbook checksum mismatch for {} (auto-recovery disabled)", symbol);
                                }
                            }

                            // Broadcast assembled orderbook
                            let snapshot = book.to_orderbook(None);
                            let senders = orderbook_senders.blocking_read();
                            if let Some(tx) = senders.get(&symbol) {
                                let _ = tx.send(snapshot);
                            }
                        }
                    }
                }
                "trades" => {
                    for item in data {
                        if let Ok(trade) = parsers::parse_trade(item, &symbol) {
                            let senders = trade_senders.blocking_read();
                            if let Some(tx) = senders.get(&symbol) {
                                let _ = tx.send(trade);
                            }
                        }
                    }
                }
                _ => {
                    tracing::trace!("OKX WS: unhandled channel: {}", channel);
                }
            }
        });

        self.public_conn.set_message_handler(handler).await;
    }
}

#[async_trait]
impl ExchangeWs for OkxWs {
    async fn watch_ticker(&self, symbol: &str) -> Result<WsStream<Ticker>> {
        let okx_id = Self::inst_id(symbol);
        let sub_id = SubscriptionId(format!("tickers:{}", okx_id));
        let sub_msg = Self::subscribe_msg("tickers", &okx_id);

        let rx = {
            let mut senders = self.ticker_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_public_handler().await;
        self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_order_book(&self, symbol: &str, _limit: Option<u32>) -> Result<WsStream<OrderBook>> {
        let okx_id = Self::inst_id(symbol);
        let sub_id = SubscriptionId(format!("books5:{}", okx_id));
        let sub_msg = Self::subscribe_msg("books5", &okx_id);

        // Initialize LocalOrderBook if not present
        {
            let mut lobs = self.local_orderbooks.write().await;
            lobs.entry(symbol.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(LocalOrderBook::new(symbol.to_string()))));
        }

        let rx = {
            let mut senders = self.orderbook_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_public_handler().await;
        self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_trades(&self, symbol: &str) -> Result<WsStream<Trade>> {
        let okx_id = Self::inst_id(symbol);
        let sub_id = SubscriptionId(format!("trades:{}", okx_id));
        let sub_msg = Self::subscribe_msg("trades", &okx_id);

        let rx = {
            let mut senders = self.trade_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_public_handler().await;
        self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_ohlcv(&self, symbol: &str, timeframe: Timeframe) -> Result<WsStream<OHLCV>> {
        let okx_id = Self::inst_id(symbol);
        let interval = parsers::timeframe_to_okx(&timeframe);
        let channel = format!("candle{}", interval);
        let sub_id = SubscriptionId(format!("{}:{}", channel, okx_id));
        let sub_msg = Self::subscribe_msg(&channel, &okx_id);

        let key = format!("{}:{}", symbol, interval);
        let rx = {
            let mut senders = self.ohlcv_senders.write().await;
            let tx = senders
                .entry(key)
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_public_handler().await;
        self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_orders(&self, _symbol: Option<&str>) -> Result<WsStream<Order>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("orders".to_string());
        let rx = self.order_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_balance(&self) -> Result<WsStream<Balances>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("account".to_string());
        let rx = self.balance_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_positions(&self, _symbols: Option<&[&str]>) -> Result<WsStream<Vec<Position>>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("positions".to_string());
        let rx = self.position_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_my_trades(&self, _symbol: Option<&str>) -> Result<WsStream<Trade>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("fills".to_string());
        let rx = self.my_trade_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    fn connection_state(&self) -> WsConnectionState {
        self.public_conn.connection_state().now_or_never()
            .unwrap_or(WsConnectionState::Disconnected)
    }

    async fn close(&self) -> Result<()> {
        self.public_conn.close().await?;
        let mut private = self.private_conn.write().await;
        if let Some(conn) = private.take() {
            conn.close().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_okx_checksum_format() {
        let mut lob = LocalOrderBook::new("BTC/USDT".to_string());
        lob.reset(
            vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))],
            vec![(dec!(50001), dec!(0.8)), (dec!(50002), dec!(1.2))],
            None,
            0,
        );
        let result = okx_checksum_format(&lob);
        assert_eq!(result, "50000:1.5:50001:0.8:49999:2.0:50002:1.2");
    }
}
