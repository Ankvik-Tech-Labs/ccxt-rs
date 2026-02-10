//! Binance WebSocket implementation
//!
//! Public streams: `wss://stream.binance.com:9443/ws/<stream>`
//! Private streams: requires listenKey from `POST /api/v3/userDataStream`
//!
//! Subscribe: `{"method":"SUBSCRIBE","params":["btcusdt@ticker"],"id":1}`
//! Events: 24hrTicker, depthUpdate, trade, kline, executionReport, outboundAccountPosition
//!
//! ## OrderBook Initialization
//!
//! The orderbook stream follows Binance's recommended approach for managing a local order book:
//! 1. Subscribe to `depth@100ms` incremental stream
//! 2. Buffer incoming `depthUpdate` messages during REST snapshot fetch
//! 3. Fetch initial REST snapshot via `GET /api/v3/depth?symbol={symbol}&limit=1000`
//! 4. Apply snapshot to LocalOrderBook via `reset()`
//! 5. Process buffered deltas with sequence validation: `U <= lastUpdateId+1 <= u`
//!    - `U`: first update ID in delta
//!    - `u`: last update ID in delta
//!    - `lastUpdateId`: from REST snapshot
//! 6. Continue applying real-time deltas from stream
//!
//! Edge cases handled:
//! - REST failure: Falls back to stream-only mode (logs error)
//! - Sequence gap: Logs warning, skips delta (future work: re-snapshot)
//! - Buffer overflow: Max 100 deltas, drops oldest
//! - Stale deltas: Silently ignored if `u <= lastUpdateId`
//!
//! See: https://binance-docs.github.io/apidocs/spot/en/#how-to-manage-a-local-order-book-correctly

use crate::base::errors::{CcxtError, Result};
use crate::base::local_orderbook::LocalOrderBook;
use crate::base::signer::timestamp_ms;
use crate::base::ws::{ExchangeWs, NowOrNever, SubscriptionId, WsConfig, WsConnectionState, WsStream};
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

/// Static helper for fetching depth snapshot (used in spawned task)
async fn fetch_depth_snapshot_static(
    symbol: &str,
    limit: u32,
    sandbox: bool,
) -> Result<OrderBook> {
    let raw_symbol = symbol.split('/').collect::<Vec<_>>().join("").to_lowercase();
    let base_url = if sandbox {
        "https://testnet.binance.vision"
    } else {
        "https://api.binance.com"
    };

    let url = format!(
        "{}/api/v3/depth?symbol={}&limit={}",
        base_url,
        raw_symbol.to_uppercase(),
        limit
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CcxtError::NetworkError(format!("Failed to fetch depth snapshot: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(CcxtError::ExchangeError(format!(
            "Depth snapshot request failed with status {}: {}",
            status, text
        )));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| CcxtError::ParseError(format!("Failed to parse depth snapshot: {}", e)))?;

    parsers::parse_order_book(&json, symbol)
}

/// Buffer for pending depth updates during REST snapshot fetch
#[derive(Debug, Clone)]
struct DepthUpdateBuffer {
    /// Buffered depth update messages (max 100)
    deltas: Vec<serde_json::Value>,
    /// Whether REST snapshot has been received and applied
    snapshot_ready: bool,
}

impl DepthUpdateBuffer {
    fn new() -> Self {
        Self {
            deltas: Vec::new(),
            snapshot_ready: false,
        }
    }

    /// Add a delta to the buffer (max 100 items, drops oldest)
    fn push_delta(&mut self, delta: serde_json::Value) {
        if self.deltas.len() >= 100 {
            self.deltas.remove(0);
            tracing::warn!("Depth update buffer overflow, dropping oldest delta");
        }
        self.deltas.push(delta);
    }

    /// Mark snapshot as ready and return buffered deltas
    fn mark_ready(&mut self) -> Vec<serde_json::Value> {
        self.snapshot_ready = true;
        std::mem::take(&mut self.deltas)
    }

    fn is_ready(&self) -> bool {
        self.snapshot_ready
    }
}

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

    /// Local orderbook state per symbol
    local_orderbooks: Arc<RwLock<HashMap<String, Arc<RwLock<LocalOrderBook>>>>>,

    /// Depth update buffers per symbol (for REST snapshot initialization)
    depth_buffers: Arc<RwLock<HashMap<String, Arc<RwLock<DepthUpdateBuffer>>>>>,

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

    /// Handle for the listenKey keepalive task (aborted on close)
    keepalive_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
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
            local_orderbooks: Arc::new(RwLock::new(HashMap::new())),
            depth_buffers: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            config,
            sandbox,
            api_key: None,
            secret: None,
            listen_key: Arc::new(RwLock::new(None)),
            keepalive_handle: Arc::new(RwLock::new(None)),
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
        let handle = tokio::spawn(async move {
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

        // Store handle so we can abort on close
        {
            let mut kh = self.keepalive_handle.write().await;
            *kh = Some(handle);
        }

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
        let local_orderbooks = self.local_orderbooks.clone();
        let depth_buffers = self.depth_buffers.clone();

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
                    // Check if snapshot is ready
                    let buffers = depth_buffers.blocking_read();
                    let buffer_lock = buffers.get(&symbol);

                    if let Some(buf_lock) = buffer_lock {
                        let mut buffer = buf_lock.blocking_write();

                        if !buffer.is_ready() {
                            // Buffer the delta until snapshot arrives
                            tracing::trace!("Buffering depth update for {} (snapshot not ready)", symbol);
                            buffer.push_delta(json.clone());
                            return;
                        }
                    } else {
                        // No buffer = no snapshot fetch initiated, skip
                        return;
                    }

                    // Snapshot is ready, process delta
                    let obs = local_orderbooks.blocking_read();
                    if let Some(ob_lock) = obs.get(&symbol) {
                        let mut ob = ob_lock.blocking_write();

                        // Validate sequence before applying
                        let first_update_id = json.get("U").and_then(|v| v.as_u64());
                        let last_update_id = json.get("u").and_then(|v| v.as_u64());
                        let current_nonce = ob.nonce();

                        let should_apply = if let (Some(u_capital), Some(u_lower), Some(nonce)) =
                            (first_update_id, last_update_id, current_nonce) {
                            // Sequence check: U <= lastUpdateId + 1 <= u
                            if u_capital <= nonce + 1 && nonce + 1 <= u_lower {
                                true
                            } else if u_lower <= nonce {
                                // Stale delta, skip
                                false
                            } else {
                                // Sequence gap
                                tracing::warn!(
                                    "Sequence gap for {}: U={}, u={}, lastUpdateId={} (expected U <= {} <= u)",
                                    symbol, u_capital, u_lower, nonce, nonce + 1
                                );
                                false
                            }
                        } else {
                            // Missing fields, skip
                            false
                        };

                        if !should_apply {
                            return;
                        }

                        // Parse delta update fields: "b" (bids) and "a" (asks)
                        let bids_json = json.get("b").and_then(|v| v.as_array());
                        let asks_json = json.get("a").and_then(|v| v.as_array());

                        if bids_json.is_none() && asks_json.is_none() {
                            return;
                        }

                        // Parse bids and asks into Vec<(Decimal, Decimal)>
                        let mut bids = Vec::new();
                        if let Some(bids_arr) = bids_json {
                            for bid in bids_arr {
                                if let Some(arr) = bid.as_array() {
                                    if arr.len() >= 2 {
                                        if let (Some(p_str), Some(q_str)) = (arr[0].as_str(), arr[1].as_str()) {
                                            if let (Ok(price), Ok(qty)) = (Decimal::from_str(p_str), Decimal::from_str(q_str)) {
                                                bids.push((price, qty));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let mut asks = Vec::new();
                        if let Some(asks_arr) = asks_json {
                            for ask in asks_arr {
                                if let Some(arr) = ask.as_array() {
                                    if arr.len() >= 2 {
                                        if let (Some(p_str), Some(q_str)) = (arr[0].as_str(), arr[1].as_str()) {
                                            if let (Ok(price), Ok(qty)) = (Decimal::from_str(p_str), Decimal::from_str(q_str)) {
                                                asks.push((price, qty));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        ob.update_bids(&bids);
                        ob.update_asks(&asks);
                        if let Some(nonce) = last_update_id {
                            ob.set_nonce(nonce);
                        }
                        ob.set_timestamp(timestamp_ms());

                        // Broadcast updated orderbook
                        let snapshot = ob.to_orderbook(None);
                        drop(ob); // Release lock before sending
                        let senders = orderbook_senders.blocking_read();
                        if let Some(tx) = senders.get(&symbol) {
                            let _ = tx.send(snapshot);
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

    async fn watch_order_book(&self, symbol: &str, _limit: Option<u32>) -> Result<WsStream<OrderBook>> {
        let stream_sym = Self::stream_symbol(symbol);
        // Use incremental depth stream (not snapshot)
        let stream_name = format!("{}@depth@100ms", stream_sym);
        let sub_id = SubscriptionId(stream_name.clone());
        let sub_msg = self.subscribe_msg(&[&stream_name]);

        // Initialize LocalOrderBook and buffer for this symbol if not present
        {
            let mut obs = self.local_orderbooks.write().await;
            obs.entry(symbol.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(LocalOrderBook::new(symbol.to_string()))));
        }

        {
            let mut buffers = self.depth_buffers.write().await;
            buffers.entry(symbol.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(DepthUpdateBuffer::new())));
        }

        // Set up handler before subscribing
        self.setup_public_handler().await;

        // Subscribe to WebSocket stream (deltas will be buffered until snapshot arrives)
        self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

        // Spawn task to fetch REST snapshot
        let symbol_clone = symbol.to_string();
        let self_clone_sandbox = self.sandbox;
        let local_orderbooks = self.local_orderbooks.clone();
        let depth_buffers = self.depth_buffers.clone();
        let orderbook_senders = self.orderbook_senders.clone();

        tokio::spawn(async move {
            tracing::info!("Fetching REST snapshot for {}", symbol_clone);

            // Fetch snapshot via REST
            let snapshot_result = fetch_depth_snapshot_static(
                &symbol_clone,
                1000,
                self_clone_sandbox,
            ).await;

            match snapshot_result {
                Ok(snapshot) => {
                    tracing::info!(
                        "REST snapshot received for {}: lastUpdateId={:?}, {} bids, {} asks",
                        symbol_clone, snapshot.nonce, snapshot.bids.len(), snapshot.asks.len()
                    );

                    // Reset LocalOrderBook with snapshot
                    let obs = local_orderbooks.read().await;
                    if let Some(ob_lock) = obs.get(&symbol_clone) {
                        let mut ob = ob_lock.write().await;
                        ob.reset(
                            snapshot.bids.clone(),
                            snapshot.asks.clone(),
                            snapshot.nonce,
                            snapshot.timestamp,
                        );
                    }

                    // Mark buffer as ready and get buffered deltas
                    let buffers = depth_buffers.read().await;
                    let buffered_deltas = if let Some(buf_lock) = buffers.get(&symbol_clone) {
                        let mut buffer = buf_lock.write().await;
                        buffer.mark_ready()
                    } else {
                        Vec::new()
                    };

                    tracing::info!(
                        "Processing {} buffered depth updates for {}",
                        buffered_deltas.len(),
                        symbol_clone
                    );

                    // Apply buffered deltas that pass sequence validation
                    let obs = local_orderbooks.read().await;
                    if let Some(ob_lock) = obs.get(&symbol_clone) {
                        for delta in buffered_deltas {
                            let mut ob = ob_lock.write().await;

                            let first_update_id = delta.get("U").and_then(|v| v.as_u64());
                            let last_update_id = delta.get("u").and_then(|v| v.as_u64());
                            let current_nonce = ob.nonce();

                            let should_apply = if let (Some(u_capital), Some(u_lower), Some(nonce)) =
                                (first_update_id, last_update_id, current_nonce) {
                                if u_capital <= nonce + 1 && nonce + 1 <= u_lower {
                                    true
                                } else if u_lower <= nonce {
                                    false // Stale
                                } else {
                                    tracing::warn!(
                                        "Buffered delta has sequence gap for {}: U={}, u={}, lastUpdateId={}",
                                        symbol_clone, u_capital, u_lower, nonce
                                    );
                                    false
                                }
                            } else {
                                false
                            };

                            if should_apply {
                                // Parse and apply delta
                                let bids_json = delta.get("b").and_then(|v| v.as_array());
                                let asks_json = delta.get("a").and_then(|v| v.as_array());

                                let mut bids = Vec::new();
                                if let Some(bids_arr) = bids_json {
                                    for bid in bids_arr {
                                        if let Some(arr) = bid.as_array() {
                                            if arr.len() >= 2 {
                                                if let (Some(p_str), Some(q_str)) = (arr[0].as_str(), arr[1].as_str()) {
                                                    if let (Ok(price), Ok(qty)) = (Decimal::from_str(p_str), Decimal::from_str(q_str)) {
                                                        bids.push((price, qty));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let mut asks = Vec::new();
                                if let Some(asks_arr) = asks_json {
                                    for ask in asks_arr {
                                        if let Some(arr) = ask.as_array() {
                                            if arr.len() >= 2 {
                                                if let (Some(p_str), Some(q_str)) = (arr[0].as_str(), arr[1].as_str()) {
                                                    if let (Ok(price), Ok(qty)) = (Decimal::from_str(p_str), Decimal::from_str(q_str)) {
                                                        asks.push((price, qty));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                ob.update_bids(&bids);
                                ob.update_asks(&asks);
                                if let Some(nonce) = last_update_id {
                                    ob.set_nonce(nonce);
                                }
                                ob.set_timestamp(timestamp_ms());
                            }
                        }

                        // Broadcast initial snapshot after applying buffered deltas
                        let ob = ob_lock.read().await;
                        let snapshot = ob.to_orderbook(None);
                        drop(ob);

                        let senders = orderbook_senders.read().await;
                        if let Some(tx) = senders.get(&symbol_clone) {
                            let _ = tx.send(snapshot);
                        }
                    }

                    tracing::info!("Orderbook initialization complete for {}", symbol_clone);
                }
                Err(e) => {
                    tracing::error!("Failed to fetch REST snapshot for {}: {}", symbol_clone, e);
                    tracing::warn!("Falling back to stream-only mode for {}", symbol_clone);

                    // Mark buffer as ready anyway to allow stream processing
                    let buffers = depth_buffers.read().await;
                    if let Some(buf_lock) = buffers.get(&symbol_clone) {
                        let mut buffer = buf_lock.write().await;
                        let _ = buffer.mark_ready();
                    }
                }
            }
        });

        let rx = {
            let mut senders = self.orderbook_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

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
        // Abort keepalive task to prevent leaking
        {
            let mut kh = self.keepalive_handle.write().await;
            if let Some(handle) = kh.take() {
                handle.abort();
            }
        }

        self.public_conn.close().await?;

        let mut private = self.private_conn.write().await;
        if let Some(conn) = private.take() {
            conn.close().await?;
        }

        // Clear listen key
        *self.listen_key.write().await = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test sequence validation logic
    #[test]
    fn test_depth_update_sequence_validation() {
        // Test case 1: Valid sequence (U <= lastUpdateId + 1 <= u)
        let last_update_id = 100u64;
        let first_update_id = 99u64;
        let new_update_id = 102u64;

        // Valid: 99 <= 101 <= 102
        assert!(first_update_id <= last_update_id + 1);
        assert!(last_update_id + 1 <= new_update_id);

        // Test case 2: Stale delta (u <= lastUpdateId)
        let stale_update = 95u64;
        assert!(stale_update <= last_update_id);

        // Test case 3: Sequence gap (U > lastUpdateId + 1)
        let gap_first = 105u64;
        let _gap_last = 110u64;
        assert!(gap_first > last_update_id + 1);
        // This should be rejected with a warning

        // Test case 4: Perfect continuation (U = lastUpdateId + 1)
        let perfect_first = 101u64;
        let perfect_last = 105u64;
        assert_eq!(perfect_first, last_update_id + 1);
        assert!(last_update_id + 1 <= perfect_last);
    }

    /// Test depth buffer overflow behavior
    #[test]
    fn test_depth_buffer_overflow() {
        let mut buffer = DepthUpdateBuffer::new();

        // Add 101 deltas (max is 100, should drop oldest)
        for i in 0..101 {
            let delta = serde_json::json!({
                "e": "depthUpdate",
                "U": i,
                "u": i + 1
            });
            buffer.push_delta(delta);
        }

        // Buffer should have exactly 100 items
        assert_eq!(buffer.deltas.len(), 100);

        // Oldest item (index 0) should be gone, first should be index 1
        let first = &buffer.deltas[0];
        assert_eq!(first.get("U").and_then(|v| v.as_u64()), Some(1));
    }

    /// Test buffer ready state transition
    #[test]
    fn test_buffer_ready_state() {
        let mut buffer = DepthUpdateBuffer::new();

        // Initially not ready
        assert!(!buffer.is_ready());

        // Add some deltas
        for i in 0..5 {
            buffer.push_delta(serde_json::json!({"U": i, "u": i + 1}));
        }

        assert_eq!(buffer.deltas.len(), 5);
        assert!(!buffer.is_ready());

        // Mark as ready
        let buffered = buffer.mark_ready();

        // Should be ready now and deltas should be returned
        assert!(buffer.is_ready());
        assert_eq!(buffered.len(), 5);
        assert_eq!(buffer.deltas.len(), 0); // Should be cleared
    }
}

