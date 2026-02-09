//! Binance WebSocket implementation
//!
//! Public streams: `wss://stream.binance.com:9443/ws/<stream>`
//! Private streams: requires listenKey from `POST /api/v3/userDataStream`
//!
//! Subscribe: `{"method":"SUBSCRIBE","params":["btcusdt@ticker"],"id":1}`
//! Events: 24hrTicker, depthUpdate, trade, kline, executionReport, outboundAccountPosition

use crate::base::errors::{CcxtError, Result};
use crate::base::signer::timestamp_ms;
use crate::base::ws::{ExchangeWs, SubscriptionId, WsConfig, WsConnectionState, WsStream};
use crate::base::ws_connection::{WsConnectionManager, MessageHandler};
use crate::binance::parsers;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";
const BINANCE_WS_TESTNET_URL: &str = "wss://testnet.binance.vision/ws";

/// Binance WebSocket client for real-time data streams
pub struct BinanceWs {
    /// Public stream connection
    public_conn: Arc<WsConnectionManager>,

    /// Private stream connection (user data)
    private_conn: Arc<RwLock<Option<WsConnectionManager>>>,

    /// Ticker broadcast senders by symbol
    ticker_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Ticker>>>>,

    /// OrderBook broadcast senders by symbol
    orderbook_senders: Arc<RwLock<HashMap<String, broadcast::Sender<OrderBook>>>>,

    /// Trade broadcast senders by symbol
    trade_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Trade>>>>,

    /// OHLCV broadcast senders by symbol+timeframe
    ohlcv_senders: Arc<RwLock<HashMap<String, broadcast::Sender<OHLCV>>>>,

    /// Order update broadcast sender
    order_sender: broadcast::Sender<Order>,

    /// Balance update broadcast sender
    balance_sender: broadcast::Sender<Balances>,

    /// Position update broadcast sender
    position_sender: broadcast::Sender<Vec<Position>>,

    /// My trades broadcast sender
    my_trade_sender: broadcast::Sender<Trade>,

    /// Subscribe message ID counter
    next_id: AtomicU64,

    /// Config
    config: WsConfig,

    /// Sandbox mode
    sandbox: bool,

    /// API key and secret for private streams
    api_key: Option<String>,
    #[allow(dead_code)]
    secret: Option<String>,

    /// ListenKey for user data stream
    listen_key: Arc<RwLock<Option<String>>>,
}

impl BinanceWs {
    /// Create a new Binance WebSocket client
    pub fn new(sandbox: bool, config: WsConfig) -> Self {
        let ws_url = if sandbox {
            BINANCE_WS_TESTNET_URL
        } else {
            BINANCE_WS_URL
        };

        let public_conn = WsConnectionManager::new(ws_url, config.clone());
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
            next_id: AtomicU64::new(1),
            config,
            sandbox,
            api_key: None,
            secret: None,
            listen_key: Arc::new(RwLock::new(None)),
        }
    }

    /// Set API credentials for private streams
    pub fn with_credentials(mut self, api_key: String, secret: String) -> Self {
        self.api_key = Some(api_key);
        self.secret = Some(secret);
        self
    }

    /// Get the next subscribe message ID
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Convert unified symbol to Binance stream format (lowercase, no slash)
    fn stream_symbol(symbol: &str) -> String {
        parsers::symbol_to_binance(symbol).to_lowercase()
    }

    /// Build a subscribe message
    fn subscribe_msg(&self, params: &[&str]) -> String {
        let id = self.next_id();
        let params_json: Vec<String> = params.iter().map(|p| format!("\"{}\"", p)).collect();
        format!(
            r#"{{"method":"SUBSCRIBE","params":[{}],"id":{}}}"#,
            params_json.join(","),
            id
        )
    }

    /// Ensure the private WebSocket connection is established.
    /// Creates a listenKey via REST API and connects to the user data stream.
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

        // Get listenKey via REST
        let base_url = if self.sandbox {
            "https://testnet.binance.vision"
        } else {
            "https://api.binance.com"
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/v3/userDataStream", base_url))
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| CcxtError::NetworkError(format!("Failed to get listenKey: {}", e)))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CcxtError::ParseError(format!("Failed to parse listenKey response: {}", e)))?;

        let listen_key = json
            .get("listenKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing listenKey in response".to_string()))?
            .to_string();

        // Store listenKey
        {
            let mut lk = self.listen_key.write().await;
            *lk = Some(listen_key.clone());
        }

        // Create private WS connection
        let ws_base = if self.sandbox {
            BINANCE_WS_TESTNET_URL
        } else {
            BINANCE_WS_URL
        };
        let private_url = format!("{}/{}", ws_base, listen_key);
        let private_conn = WsConnectionManager::new(&private_url, self.config.clone());

        // Set up private handler
        self.setup_private_handler(&private_conn).await;

        // Connect
        private_conn.connect().await?;

        // Store connection
        {
            let mut guard = self.private_conn.write().await;
            *guard = Some(private_conn);
        }

        // Spawn keepalive task (PUT listenKey every 30 minutes)
        let api_key_clone = api_key.clone();
        let base_url_clone = base_url.to_string();
        let listen_key_clone = listen_key;
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
            loop {
                interval.tick().await;
                let _ = client
                    .put(format!(
                        "{}/api/v3/userDataStream?listenKey={}",
                        base_url_clone, listen_key_clone
                    ))
                    .header("X-MBX-APIKEY", &api_key_clone)
                    .send()
                    .await;
            }
        });

        Ok(())
    }

    /// Set up the message handler for the private connection.
    async fn setup_private_handler(&self, conn: &WsConnectionManager) {
        let order_sender = self.order_sender.clone();
        let balance_sender = self.balance_sender.clone();
        let my_trade_sender = self.my_trade_sender.clone();

        let handler: MessageHandler = Arc::new(move |text: String| {
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            let event = json.get("e").and_then(|v| v.as_str()).unwrap_or("");

            match event {
                "executionReport" => {
                    let raw_symbol = json.get("s").and_then(|v| v.as_str()).unwrap_or("");
                    let symbol = parsers::symbol_from_binance(raw_symbol);

                    if let Ok(order) = parsers::parse_order(&json, &symbol, false) {
                        let _ = order_sender.send(order.clone());
                        // If it's a trade execution, also send to my_trade_sender
                        if json.get("x").and_then(|v| v.as_str()) == Some("TRADE") {
                            let now = timestamp_ms();
                            let price = json.get("L").and_then(|v| v.as_str())
                                .and_then(|s| Decimal::from_str(s).ok())
                                .unwrap_or(Decimal::ZERO);
                            let amount = json.get("l").and_then(|v| v.as_str())
                                .and_then(|s| Decimal::from_str(s).ok())
                                .unwrap_or(Decimal::ZERO);
                            let trade = Trade {
                                id: json.get("t").and_then(|v| v.as_i64())
                                    .map(|t| t.to_string()).unwrap_or_default(),
                                symbol: symbol.clone(),
                                order: Some(order.id.clone()),
                                timestamp: now,
                                datetime: crate::base::signer::timestamp_to_iso8601(now),
                                side: order.side,
                                price,
                                amount,
                                cost: price * amount,
                                fee: None,
                                taker_or_maker: None,
                                info: None,
                            };
                            let _ = my_trade_sender.send(trade);
                        }
                    }
                }
                "outboundAccountPosition" => {
                    let now = timestamp_ms();
                    let mut balances_map = HashMap::new();
                    let mut free_map = HashMap::new();
                    let mut used_map = HashMap::new();
                    let mut total_map = HashMap::new();

                    if let Some(assets) = json.get("B").and_then(|v| v.as_array()) {
                        for asset in assets {
                            let currency = asset.get("a").and_then(|v| v.as_str()).unwrap_or("");
                            let free = asset.get("f").and_then(|v| v.as_str())
                                .and_then(|s| Decimal::from_str(s).ok())
                                .unwrap_or(Decimal::ZERO);
                            let locked = asset.get("l").and_then(|v| v.as_str())
                                .and_then(|s| Decimal::from_str(s).ok())
                                .unwrap_or(Decimal::ZERO);
                            let total = free + locked;

                            balances_map.insert(
                                currency.to_string(),
                                Balance::new(currency.to_string(), free, locked),
                            );
                            free_map.insert(currency.to_string(), free);
                            used_map.insert(currency.to_string(), locked);
                            total_map.insert(currency.to_string(), total);
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
                _ => {}
            }
        });

        conn.set_message_handler(handler).await;
    }

    /// Set up the message handler for the public connection.
    /// Dispatches incoming messages to the appropriate broadcast channels.
    async fn setup_public_handler(&self) {
        let ticker_senders = self.ticker_senders.clone();
        let orderbook_senders = self.orderbook_senders.clone();
        let trade_senders = self.trade_senders.clone();
        let ohlcv_senders = self.ohlcv_senders.clone();

        let handler: MessageHandler = Arc::new(move |text: String| {
            // Parse the JSON message
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            // Determine event type
            let event = json.get("e").and_then(|v| v.as_str()).unwrap_or("");
            let stream_symbol = json.get("s").and_then(|v| v.as_str()).unwrap_or("");
            let symbol = parsers::symbol_from_binance(stream_symbol);

            match event {
                "24hrTicker" => {
                    if let Ok(ticker) = parsers::parse_ticker(&json, &symbol) {
                        let senders = ticker_senders.blocking_read();
                        if let Some(tx) = senders.get(&symbol) {
                            let _ = tx.send(ticker);
                        }
                    }
                }
                "depthUpdate" => {
                    if let Ok(ob) = parsers::parse_order_book(&json, &symbol) {
                        let senders = orderbook_senders.blocking_read();
                        if let Some(tx) = senders.get(&symbol) {
                            let _ = tx.send(ob);
                        }
                    }
                }
                "trade" => {
                    if let Ok(trade) = parsers::parse_trade(&json, &symbol) {
                        let senders = trade_senders.blocking_read();
                        if let Some(tx) = senders.get(&symbol) {
                            let _ = tx.send(trade);
                        }
                    }
                }
                "kline" => {
                    if let Some(k) = json.get("k") {
                        if let Ok(ohlcv) = parsers::parse_ohlcv(k) {
                            let kline_symbol = k.get("s")
                                .and_then(|v| v.as_str())
                                .map(parsers::symbol_from_binance)
                                .unwrap_or_default();
                            let interval = k.get("i").and_then(|v| v.as_str()).unwrap_or("");
                            let key = format!("{}:{}", kline_symbol, interval);
                            let senders = ohlcv_senders.blocking_read();
                            if let Some(tx) = senders.get(&key) {
                                let _ = tx.send(ohlcv);
                            }
                        }
                    }
                }
                _ => {
                    // Subscription confirmation, errors, etc.
                    tracing::trace!("Binance WS: unhandled event type: {}", event);
                }
            }
        });

        self.public_conn.set_message_handler(handler).await;
    }
}

#[async_trait]
impl ExchangeWs for BinanceWs {
    async fn watch_ticker(&self, symbol: &str) -> Result<WsStream<Ticker>> {
        let stream_sym = Self::stream_symbol(symbol);
        let stream_name = format!("{}@ticker", stream_sym);
        let sub_id = SubscriptionId(stream_name.clone());
        let sub_msg = self.subscribe_msg(&[&stream_name]);

        // Create or get broadcast sender
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
        let stream_sym = Self::stream_symbol(symbol);
        let depth = limit.unwrap_or(20);
        let stream_name = format!("{}@depth{}@100ms", stream_sym, depth);
        let sub_id = SubscriptionId(stream_name.clone());
        let sub_msg = self.subscribe_msg(&[&stream_name]);

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
        let stream_sym = Self::stream_symbol(symbol);
        let stream_name = format!("{}@trade", stream_sym);
        let sub_id = SubscriptionId(stream_name.clone());
        let sub_msg = self.subscribe_msg(&[&stream_name]);

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
        let stream_sym = Self::stream_symbol(symbol);
        let interval = parsers::timeframe_to_binance(timeframe);
        let stream_name = format!("{}@kline_{}", stream_sym, interval);
        let sub_id = SubscriptionId(stream_name.clone());
        let sub_msg = self.subscribe_msg(&[&stream_name]);

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
        let sub_id = SubscriptionId("userDataStream:orders".to_string());
        let rx = self.order_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_balance(&self) -> Result<WsStream<Balances>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("userDataStream:balance".to_string());
        let rx = self.balance_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_positions(&self, _symbols: Option<&[&str]>) -> Result<WsStream<Vec<Position>>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("userDataStream:positions".to_string());
        let rx = self.position_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_my_trades(&self, _symbol: Option<&str>) -> Result<WsStream<Trade>> {
        self.ensure_private_connection().await?;
        let sub_id = SubscriptionId("userDataStream:trades".to_string());
        let rx = self.my_trade_sender.subscribe();
        Ok(WsStream::new(rx, sub_id))
    }

    fn connection_state(&self) -> WsConnectionState {
        // Use try_read to avoid blocking in a sync context
        match self.public_conn.connection_state().now_or_never() {
            Some(state) => state,
            None => WsConnectionState::Disconnected,
        }
    }

    async fn close(&self) -> Result<()> {
        self.public_conn.close().await?;

        let private = self.private_conn.read().await;
        if let Some(ref conn) = *private {
            conn.close().await?;
        }

        Ok(())
    }
}

/// Helper trait for sync access to async state
trait NowOrNever {
    type Output;
    fn now_or_never(self) -> Option<Self::Output>;
}

impl<F: std::future::Future> NowOrNever for F {
    type Output = F::Output;
    fn now_or_never(self) -> Option<Self::Output> {
        // This is a sync poll — works for RwLock reads that are likely uncontested
        let mut pinned = std::pin::pin!(self);
        let waker = futures_util::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        match pinned.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => Some(v),
            std::task::Poll::Pending => None,
        }
    }
}
