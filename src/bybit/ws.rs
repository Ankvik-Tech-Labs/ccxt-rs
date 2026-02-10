//! Bybit WebSocket implementation
//!
//! Public: `wss://stream.bybit.com/v5/public/spot` (or `/linear`)
//! Private: `wss://stream.bybit.com/v5/private` with HMAC auth at connect
//!
//! Auth: `{"op":"auth","args":["api_key","expires","signature"]}`
//! Heartbeat: `{"op":"ping"}` / `{"op":"pong"}`
//! Subscribe: `{"op":"subscribe","args":["orderbook.50.BTCUSDT"]}`

use crate::base::errors::{CcxtError, Result};
use crate::base::local_orderbook::LocalOrderBook;
use crate::base::signer::{hmac_sha256, timestamp_ms};
use crate::base::ws::{ExchangeWs, NowOrNever, SubscriptionId, WsConfig, WsConnectionState, WsStream};
use crate::base::ws_connection::{WsConnectionManager, MessageHandler};
use crate::bybit::parsers;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::Instant;

const BYBIT_WS_PUBLIC: &str = "wss://stream.bybit.com/v5/public/spot";
const BYBIT_WS_PUBLIC_LINEAR: &str = "wss://stream.bybit.com/v5/public/linear";
const BYBIT_WS_PRIVATE: &str = "wss://stream.bybit.com/v5/private";
const BYBIT_WS_TESTNET_PUBLIC: &str = "wss://stream-testnet.bybit.com/v5/public/spot";
const BYBIT_WS_TESTNET_PRIVATE: &str = "wss://stream-testnet.bybit.com/v5/private";

/// Bybit checksum format: first 25 bids and asks interleaved
/// "bid1_price:bid1_amount:ask1_price:ask1_amount:..."
fn bybit_checksum_format(lob: &LocalOrderBook) -> String {
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

/// Bybit WebSocket client
pub struct BybitWs {
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

    config: WsConfig,
    sandbox: bool,
    api_key: Option<String>,
    secret: Option<String>,
}

impl BybitWs {
    /// Create a new Bybit WebSocket client
    pub fn new(sandbox: bool, config: WsConfig) -> Self {
        let ws_url = if sandbox {
            BYBIT_WS_TESTNET_PUBLIC
        } else {
            BYBIT_WS_PUBLIC
        };

        let public_conn = WsConnectionManager::new(ws_url, config.clone())
            .with_ping_message(r#"{"op":"ping"}"#);

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
            config,
            sandbox,
            api_key: None,
            secret: None,
        }
    }

    /// Set API credentials for private streams
    pub fn with_credentials(mut self, api_key: String, secret: String) -> Self {
        self.api_key = Some(api_key);
        self.secret = Some(secret);
        self
    }

    /// Convert unified symbol to Bybit format
    fn stream_symbol(symbol: &str) -> String {
        // "BTC/USDT" → "BTCUSDT"
        symbol.replace('/', "")
    }

    /// Build Bybit subscribe message
    fn subscribe_msg(args: &[&str]) -> String {
        let args_json: Vec<String> = args.iter().map(|a| format!("\"{}\"", a)).collect();
        format!(r#"{{"op":"subscribe","args":[{}]}}"#, args_json.join(","))
    }

    /// Build auth message for private connection
    fn build_auth_message(api_key: &str, secret: &str) -> Result<String> {
        let expires = (timestamp_ms() + 10000).to_string(); // 10s from now
        let sign_str = format!("GET/realtime{}", expires);
        let signature = hmac_sha256(secret, &sign_str)?;
        Ok(format!(
            r#"{{"op":"auth","args":["{}","{}","{}"]}}"#,
            api_key, expires, signature
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

        let private_url = if self.sandbox {
            BYBIT_WS_TESTNET_PRIVATE
        } else {
            BYBIT_WS_PRIVATE
        };

        let auth_msg = Self::build_auth_message(api_key, secret)?;

        let private_conn = WsConnectionManager::new(private_url, self.config.clone())
            .with_ping_message(r#"{"op":"ping"}"#);

        // Set auth message so it's sent on connect and reconnect
        private_conn.set_auth_message(auth_msg).await;

        // Set up private handler
        self.setup_private_handler(&private_conn).await;

        // Connect (will auto-send auth)
        private_conn.connect().await?;

        // Subscribe to private topics
        let sub_msg = Self::subscribe_msg(&["order", "wallet", "position", "execution"]);
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
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            // Track text-based pong responses for keepalive timeout detection
            if json.get("op").and_then(|v| v.as_str()) == Some("pong") {
                *last_pong.blocking_write() = Some(Instant::now());
                return;
            }

            let topic = json.get("topic").and_then(|v| v.as_str()).unwrap_or("");

            match topic {
                "order" => {
                    if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                        for item in data {
                            let bybit_symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                            let symbol = parsers::symbol_from_bybit(bybit_symbol);
                            if let Ok(order) = parsers::parse_order(item, &symbol) {
                                let _ = order_sender.send(order);
                            }
                        }
                    }
                }
                "wallet" => {
                    if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                        let now = timestamp_ms();
                        let mut balances_map = HashMap::new();
                        let mut free_map = HashMap::new();
                        let mut used_map = HashMap::new();
                        let mut total_map = HashMap::new();

                        for item in data {
                            if let Some(coins) = item.get("coin").and_then(|v| v.as_array()) {
                                for coin in coins {
                                    let currency = coin.get("coin").and_then(|v| v.as_str()).unwrap_or("");
                                    let free = coin.get("availableToWithdraw").and_then(|v| v.as_str())
                                        .and_then(|s| Decimal::from_str(s).ok())
                                        .unwrap_or(Decimal::ZERO);
                                    let total_val = coin.get("walletBalance").and_then(|v| v.as_str())
                                        .and_then(|s| Decimal::from_str(s).ok())
                                        .unwrap_or(Decimal::ZERO);
                                    let used = total_val - free;

                                    balances_map.insert(
                                        currency.to_string(),
                                        Balance::new(currency.to_string(), free, used),
                                    );
                                    free_map.insert(currency.to_string(), free);
                                    used_map.insert(currency.to_string(), used);
                                    total_map.insert(currency.to_string(), total_val);
                                }
                            }
                        }

                        let _ = balance_sender.send(Balances {
                            timestamp: now,
                            datetime: crate::base::signer::timestamp_to_iso8601(now),
                            balances: balances_map,
                            free: free_map,
                            used: used_map,
                            total: total_map,
                            info: None,
                        });
                    }
                }
                "position" => {
                    if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
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
                }
                "execution" => {
                    if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                        for item in data {
                            let bybit_symbol = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                            let symbol = parsers::symbol_from_bybit(bybit_symbol);
                            if let Ok(trade) = parsers::parse_trade(item, &symbol) {
                                let _ = my_trade_sender.send(trade);
                            }
                        }
                    }
                }
                _ => {}
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
        let last_pong = self.public_conn.last_pong_handle();

        let handler: MessageHandler = Arc::new(move |text: String| {
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            // Track text-based pong responses for keepalive timeout detection
            if json.get("op").and_then(|v| v.as_str()) == Some("pong") {
                *last_pong.blocking_write() = Some(Instant::now());
                return;
            }

            let topic = json.get("topic").and_then(|v| v.as_str()).unwrap_or("");

            if topic.starts_with("tickers.") {
                if let Some(data) = json.get("data") {
                    let bybit_symbol = data.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                    let symbol = parsers::symbol_from_bybit(bybit_symbol);
                    if let Ok(ticker) = parsers::parse_ticker(data, &symbol) {
                        let senders = ticker_senders.blocking_read();
                        if let Some(tx) = senders.get(&symbol) {
                            let _ = tx.send(ticker);
                        }
                    }
                }
            } else if topic.starts_with("orderbook.") {
                let ob_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("snapshot");
                let data = match json.get("data") {
                    Some(d) => d,
                    None => return,
                };

                let bybit_symbol = data.get("s").and_then(|v| v.as_str()).unwrap_or("");
                let symbol = parsers::symbol_from_bybit(bybit_symbol);

                // Parse bids and asks from data
                let bids_array = data.get("b").and_then(|v| v.as_array());
                let asks_array = data.get("a").and_then(|v| v.as_array());

                let bids: Vec<(Decimal, Decimal)> = bids_array
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                let arr = item.as_array()?;
                                let price = arr.first()?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                                let amount = arr.get(1)?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                                Some((price, amount))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let asks: Vec<(Decimal, Decimal)> = asks_array
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                let arr = item.as_array()?;
                                let price = arr.first()?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                                let amount = arr.get(1)?.as_str().and_then(|s| Decimal::from_str(s).ok())?;
                                Some((price, amount))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let nonce = data.get("u").and_then(|v| v.as_u64());
                let timestamp = data
                    .get("ts")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .or_else(|| data.get("ts").and_then(|v| v.as_i64()))
                    .unwrap_or(0);

                let lobs = local_orderbooks.blocking_read();
                if let Some(lob) = lobs.get(&symbol) {
                    let mut book = lob.blocking_write();
                    match ob_type {
                        "snapshot" => {
                            book.reset(bids, asks, nonce, timestamp);
                        }
                        "delta" => {
                            book.update_bids(&bids);
                            book.update_asks(&asks);
                            if let Some(n) = nonce {
                                book.set_nonce(n);
                            }
                            book.set_timestamp(timestamp);
                        }
                        _ => {}
                    }

                    // Validate checksum if present (Bybit sends `cs` field)
                    if let Some(cs) = data.get("cs").and_then(|v| v.as_u64()) {
                        let valid = book.validate_checksum(cs as u32, bybit_checksum_format);
                        if !valid {
                            // Log checksum mismatch but continue
                            eprintln!("Bybit orderbook checksum mismatch for {}", symbol);
                        }
                    }

                    let snapshot = book.to_orderbook(None);
                    let senders = orderbook_senders.blocking_read();
                    if let Some(tx) = senders.get(&symbol) {
                        let _ = tx.send(snapshot);
                    }
                }
            } else if topic.starts_with("publicTrade.") {
                if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                    for trade_json in data {
                        let bybit_symbol = trade_json.get("s").and_then(|v| v.as_str()).unwrap_or("");
                        let symbol = parsers::symbol_from_bybit(bybit_symbol);
                        if let Ok(trade) = parsers::parse_trade(trade_json, &symbol) {
                            let senders = trade_senders.blocking_read();
                            if let Some(tx) = senders.get(&symbol) {
                                let _ = tx.send(trade);
                            }
                        }
                    }
                }
            }
        });

        self.public_conn.set_message_handler(handler).await;
    }
}

#[async_trait]
impl ExchangeWs for BybitWs {
    async fn watch_ticker(&self, symbol: &str) -> Result<WsStream<Ticker>> {
        let bybit_sym = Self::stream_symbol(symbol);
        let topic = format!("tickers.{}", bybit_sym);
        let sub_id = SubscriptionId(topic.clone());
        let sub_msg = Self::subscribe_msg(&[&topic]);

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

    async fn watch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<WsStream<OrderBook>> {
        let bybit_sym = Self::stream_symbol(symbol);
        let depth = limit.unwrap_or(50);
        let topic = format!("orderbook.{}.{}", depth, bybit_sym);
        let sub_id = SubscriptionId(topic.clone());
        let sub_msg = Self::subscribe_msg(&[&topic]);

        // Initialize local orderbook if not present
        {
            let mut lobs = self.local_orderbooks.write().await;
            if !lobs.contains_key(symbol) {
                let lob = LocalOrderBook::new(symbol.to_string());
                lobs.insert(symbol.to_string(), Arc::new(RwLock::new(lob)));
            }
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
        let bybit_sym = Self::stream_symbol(symbol);
        let topic = format!("publicTrade.{}", bybit_sym);
        let sub_id = SubscriptionId(topic.clone());
        let sub_msg = Self::subscribe_msg(&[&topic]);

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
        let bybit_sym = Self::stream_symbol(symbol);
        let interval = parsers::timeframe_to_bybit(&timeframe);
        let topic = format!("kline.{}.{}", interval, bybit_sym);
        let sub_id = SubscriptionId(topic.clone());
        let sub_msg = Self::subscribe_msg(&[&topic]);

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
        let sub_id = SubscriptionId("order".to_string());
        let rx = self.order_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_balance(&self) -> Result<WsStream<Balances>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("wallet".to_string());
        let rx = self.balance_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_positions(&self, _symbols: Option<&[&str]>) -> Result<WsStream<Vec<Position>>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("position".to_string());
        let rx = self.position_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_my_trades(&self, _symbol: Option<&str>) -> Result<WsStream<Trade>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("execution".to_string());
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
    fn test_bybit_checksum_format() {
        let mut lob = LocalOrderBook::new("BTC/USDT".to_string());
        lob.reset(
            vec![(dec!(50000.5), dec!(1.5)), (dec!(49999), dec!(2.0))],
            vec![(dec!(50001), dec!(0.8)), (dec!(50002.5), dec!(1.2))],
            None,
            0,
        );
        let result = bybit_checksum_format(&lob);
        assert_eq!(result, "50000.5:1.5:50001:0.8:49999:2.0:50002.5:1.2");
    }
}
