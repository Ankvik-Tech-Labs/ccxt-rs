//! WebSocket connection manager
//!
//! Exchange-agnostic WebSocket connection management with:
//! - Automatic reconnection with exponential backoff
//! - Ping/pong keepalive
//! - Subscription replay on reconnect
//! - Read/write split for concurrent operation

use crate::base::errors::{CcxtError, Result};
use crate::base::ws::{SubscriptionId, WsConfig, WsConnectionState};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{self, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// Callback type for processing incoming WebSocket messages
pub type MessageHandler = Arc<dyn Fn(String) + Send + Sync>;

/// Subscription entry — stores the subscribe message for replay
#[derive(Clone)]
struct Subscription {
    id: SubscriptionId,
    subscribe_message: String,
}

/// Exchange-agnostic WebSocket connection manager.
///
/// Handles:
/// - Connecting to WebSocket endpoints
/// - Sending subscribe/unsubscribe messages
/// - Ping/pong keepalive
/// - Automatic reconnection with exponential backoff
/// - Subscription replay after reconnect
pub struct WsConnectionManager {
    /// WebSocket URL
    url: String,

    /// Connection configuration
    config: WsConfig,

    /// Current connection state
    state: Arc<RwLock<WsConnectionState>>,

    /// Write half of the WebSocket (for sending)
    writer: Arc<Mutex<Option<WsSink>>>,

    /// Active subscriptions (for replay on reconnect)
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,

    /// Message handler callback
    on_message: Arc<RwLock<Option<MessageHandler>>>,

    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<tokio::sync::watch::Sender<bool>>>>,

    /// Custom ping message (None = use WebSocket Ping frame)
    ping_message: Option<String>,

    /// Authentication message to send on connect (private streams)
    auth_message: Arc<RwLock<Option<String>>>,

    /// Timestamp of last pong received (for timeout detection)
    last_pong: Arc<RwLock<Option<Instant>>>,
}

impl WsConnectionManager {
    /// Create a new connection manager
    pub fn new(url: impl Into<String>, config: WsConfig) -> Self {
        Self {
            url: url.into(),
            config,
            state: Arc::new(RwLock::new(WsConnectionState::Disconnected)),
            writer: Arc::new(Mutex::new(None)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            on_message: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            ping_message: None,
            auth_message: Arc::new(RwLock::new(None)),
            last_pong: Arc::new(RwLock::new(None)),
        }
    }

    /// Set a custom ping message (JSON string).
    /// If not set, standard WebSocket Ping frames are used.
    pub fn with_ping_message(mut self, msg: impl Into<String>) -> Self {
        self.ping_message = Some(msg.into());
        self
    }

    /// Set the authentication message for private streams.
    pub async fn set_auth_message(&self, msg: String) {
        let mut auth = self.auth_message.write().await;
        *auth = Some(msg);
    }

    /// Set the message handler callback
    pub async fn set_message_handler(&self, handler: MessageHandler) {
        let mut h = self.on_message.write().await;
        *h = Some(handler);
    }

    /// Notify the manager that a pong was received.
    ///
    /// Call this from exchange-specific text message handlers when they
    /// detect a text-based pong response (e.g., Bybit `{"op":"pong"}`,
    /// OKX `"pong"`).
    pub async fn notify_pong(&self) {
        *self.last_pong.write().await = Some(Instant::now());
    }

    /// Get a clone of the last-pong timestamp handle.
    ///
    /// Use this from synchronous message handler closures that need to
    /// signal pong receipt via `handle.blocking_write()`.
    pub fn last_pong_handle(&self) -> Arc<RwLock<Option<Instant>>> {
        self.last_pong.clone()
    }

    /// Get the current connection state
    pub async fn connection_state(&self) -> WsConnectionState {
        *self.state.read().await
    }

    /// Connect to the WebSocket endpoint and start the read loop
    pub async fn connect(&self) -> Result<()> {
        {
            let state = self.state.read().await;
            if *state == WsConnectionState::Connected {
                return Ok(());
            }
        }

        *self.state.write().await = WsConnectionState::Connecting;

        let (ws_stream, _response) = connect_async(&self.url)
            .await
            .map_err(|e| CcxtError::WsConnectionError(format!("Failed to connect to {}: {}", self.url, e)))?;

        let (write, read) = ws_stream.split();
        *self.writer.lock().await = Some(write);
        *self.state.write().await = WsConnectionState::Connected;

        tracing::info!("WebSocket connected to {}", self.url);

        // Send auth message if present
        {
            let auth = self.auth_message.read().await;
            if let Some(auth_msg) = auth.as_ref() {
                self.send_raw(auth_msg.clone()).await?;
            }
        }

        // Replay existing subscriptions
        {
            let subs = self.subscriptions.read().await;
            for sub in subs.values() {
                self.send_raw(sub.subscribe_message.clone()).await?;
            }
        }

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Spawn read loop
        let on_message = self.on_message.clone();
        let state = self.state.clone();
        let url = self.url.clone();
        let config = self.config.clone();
        let writer = self.writer.clone();
        let subscriptions = self.subscriptions.clone();
        let auth_message = self.auth_message.clone();
        let ping_message = self.ping_message.clone();
        let last_pong = self.last_pong.clone();
        let shutdown_tx_arc = self.shutdown_tx.clone();

        tokio::spawn(Self::read_loop(
            read,
            shutdown_rx,
            on_message,
            state,
            writer,
            subscriptions,
            auth_message,
            url,
            config,
            ping_message,
            last_pong,
            shutdown_tx_arc,
        ));

        Ok(())
    }

    /// Internal read loop with ping/pong and reconnection
    #[allow(clippy::too_many_arguments)]
    async fn read_loop(
        mut read: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        on_message: Arc<RwLock<Option<MessageHandler>>>,
        state: Arc<RwLock<WsConnectionState>>,
        writer: Arc<Mutex<Option<WsSink>>>,
        subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
        auth_message: Arc<RwLock<Option<String>>>,
        url: String,
        config: WsConfig,
        ping_message: Option<String>,
        last_pong: Arc<RwLock<Option<Instant>>>,
        shutdown_tx: Arc<Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
    ) {
        let mut ping_interval = time::interval(config.ping_interval);
        ping_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("WebSocket read loop shutting down");
                        break;
                    }
                }
                _ = ping_interval.tick() => {
                    // Check pong timeout before sending next ping
                    {
                        let lp = last_pong.read().await;
                        if let Some(ts) = *lp {
                            if ts.elapsed() > config.pong_timeout {
                                tracing::warn!("Pong timeout exceeded ({:?}), triggering reconnect", config.pong_timeout);
                                break;
                            }
                        }
                    }

                    // Send ping
                    let mut w = writer.lock().await;
                    if let Some(ref mut sink) = *w {
                        let msg = match &ping_message {
                            Some(custom) => Message::Text(custom.clone()),
                            None => Message::Ping(vec![]),
                        };
                        if let Err(e) = sink.send(msg).await {
                            tracing::warn!("Ping send failed: {}", e);
                            break;
                        }

                        // Initialize last_pong on first successful ping if still None
                        let mut lp = last_pong.write().await;
                        if lp.is_none() {
                            *lp = Some(Instant::now());
                        }
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let handler = on_message.read().await;
                            if let Some(ref h) = *handler {
                                h(text.to_string());
                            }
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let mut w = writer.lock().await;
                            if let Some(ref mut sink) = *w {
                                let _ = sink.send(Message::Pong(data)).await;
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            *last_pong.write().await = Some(Instant::now());
                        }
                        Some(Ok(Message::Close(_))) => {
                            tracing::info!("WebSocket received close frame");
                            break;
                        }
                        Some(Ok(Message::Binary(data))) => {
                            // Some exchanges send binary; try to convert to string
                            if let Ok(text) = String::from_utf8(data.to_vec()) {
                                let handler = on_message.read().await;
                                if let Some(ref h) = *handler {
                                    h(text);
                                }
                            }
                        }
                        Some(Ok(Message::Frame(_))) => {
                            // Raw frame, ignore
                        }
                        Some(Err(e)) => {
                            tracing::warn!("WebSocket read error: {}", e);
                            break;
                        }
                        None => {
                            tracing::info!("WebSocket stream ended");
                            break;
                        }
                    }
                }
            }
        }

        // Connection lost — attempt reconnection
        *state.write().await = WsConnectionState::Reconnecting;
        *writer.lock().await = None;
        // Reset pong tracker for reconnection
        *last_pong.write().await = None;

        let mut delay = config.reconnect_delay;
        let mut attempts = 0u32;

        loop {
            if config.max_reconnect_attempts > 0 && attempts >= config.max_reconnect_attempts {
                tracing::error!("Max reconnect attempts reached ({})", attempts);
                *state.write().await = WsConnectionState::Disconnected;
                return;
            }

            tracing::info!("Reconnecting to {} in {:?} (attempt {})", url, delay, attempts + 1);
            time::sleep(delay).await;

            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    let (write, new_read) = ws_stream.split();
                    *writer.lock().await = Some(write);
                    *state.write().await = WsConnectionState::Connected;

                    tracing::info!("Reconnected to {}", url);

                    // Re-authenticate
                    {
                        let auth = auth_message.read().await;
                        if let Some(auth_msg) = auth.as_ref() {
                            let mut w = writer.lock().await;
                            if let Some(ref mut sink) = *w {
                                let _ = sink.send(Message::Text(auth_msg.clone())).await;
                            }
                        }
                    }

                    // Replay subscriptions
                    {
                        let subs = subscriptions.read().await;
                        for sub in subs.values() {
                            let mut w = writer.lock().await;
                            if let Some(ref mut sink) = *w {
                                let _ = sink.send(Message::Text(sub.subscribe_message.clone())).await;
                            }
                        }
                    }

                    // Create new shutdown channel and store sender so close() works
                    let (new_shutdown_tx, new_shutdown_rx) = tokio::sync::watch::channel(false);
                    *shutdown_tx.lock().await = Some(new_shutdown_tx);

                    // Recurse into read loop with new stream
                    // Use Box::pin to avoid infinite type recursion
                    Box::pin(Self::read_loop(
                        new_read,
                        new_shutdown_rx,
                        on_message,
                        state,
                        writer,
                        subscriptions,
                        auth_message,
                        url,
                        config,
                        ping_message,
                        last_pong,
                        shutdown_tx,
                    ))
                    .await;
                    return;
                }
                Err(e) => {
                    tracing::warn!("Reconnect failed: {}", e);
                    attempts += 1;

                    // Exponential backoff
                    delay = std::cmp::min(delay * 2, config.max_reconnect_delay);
                }
            }
        }
    }

    /// Send a raw text message on the WebSocket
    pub async fn send_raw(&self, message: String) -> Result<()> {
        let mut writer = self.writer.lock().await;
        let sink = writer
            .as_mut()
            .ok_or_else(|| CcxtError::WsConnectionError("Not connected".to_string()))?;

        sink.send(Message::Text(message))
            .await
            .map_err(|e| CcxtError::WsConnectionError(format!("Send failed: {}", e)))
    }

    /// Subscribe to a channel with the given subscribe message
    pub async fn subscribe(&self, id: SubscriptionId, subscribe_message: String) -> Result<()> {
        // Ensure connected
        self.connect().await?;

        // Store subscription for replay
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(
                id.0.clone(),
                Subscription {
                    id: id.clone(),
                    subscribe_message: subscribe_message.clone(),
                },
            );
        }

        // Send subscribe message
        self.send_raw(subscribe_message).await
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(
        &self,
        id: &SubscriptionId,
        unsubscribe_message: Option<String>,
    ) -> Result<()> {
        // Remove from stored subscriptions
        {
            let mut subs = self.subscriptions.write().await;
            subs.remove(&id.0);
        }

        // Send unsubscribe message if provided
        if let Some(msg) = unsubscribe_message {
            self.send_raw(msg).await?;
        }

        Ok(())
    }

    /// Close the WebSocket connection
    pub async fn close(&self) -> Result<()> {
        // Signal shutdown
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(true);
        }

        // Close the writer
        let mut writer = self.writer.lock().await;
        if let Some(ref mut sink) = *writer {
            let _ = sink.close().await;
        }
        *writer = None;

        *self.state.write().await = WsConnectionState::Disconnected;

        // Clear subscriptions
        self.subscriptions.write().await.clear();

        Ok(())
    }
}
