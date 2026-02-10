//! WebSocket trait and core types for real-time exchange data streams
//!
//! This module defines the `ExchangeWs` trait (parallel to `Exchange` trait)
//! and supporting types for WebSocket-based real-time data.

use crate::base::errors::Result;
use crate::types::*;
use async_trait::async_trait;
use std::fmt;
use std::future::Future;
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::broadcast;

/// Helper trait for synchronous polling of an async future.
///
/// Useful for calling async methods (e.g., `connection_state()`) from a
/// synchronous trait method by polling exactly once. Returns `None` if
/// the future is not immediately ready.
pub trait NowOrNever {
    type Output;
    fn now_or_never(self) -> Option<Self::Output>;
}

impl<F: Future> NowOrNever for F {
    type Output = F::Output;
    fn now_or_never(self) -> Option<Self::Output> {
        let mut pinned = pin!(self);
        let waker = futures_util::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(v) => Some(v),
            Poll::Pending => None,
        }
    }
}

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsConnectionState {
    /// Not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Connected and ready
    Connected,
    /// Lost connection, attempting to reconnect
    Reconnecting,
}

impl fmt::Display for WsConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WsConnectionState::Disconnected => write!(f, "disconnected"),
            WsConnectionState::Connecting => write!(f, "connecting"),
            WsConnectionState::Connected => write!(f, "connected"),
            WsConnectionState::Reconnecting => write!(f, "reconnecting"),
        }
    }
}

/// Configuration for WebSocket connections
#[derive(Debug, Clone)]
pub struct WsConfig {
    /// Interval between ping frames (default: 20s)
    pub ping_interval: Duration,
    /// Timeout waiting for pong response (default: 10s)
    pub pong_timeout: Duration,
    /// Initial reconnect delay (default: 1s)
    pub reconnect_delay: Duration,
    /// Maximum reconnect delay with exponential backoff (default: 30s)
    pub max_reconnect_delay: Duration,
    /// Maximum number of reconnect attempts (0 = unlimited)
    pub max_reconnect_attempts: u32,
    /// Channel capacity for broadcast channels
    pub channel_capacity: usize,
    /// Enable automatic recovery on orderbook checksum failure (default: true)
    pub auto_recovery_enabled: bool,
    /// Maximum recovery attempts before giving up (0 = unlimited, default: 5)
    pub max_recovery_attempts: u32,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(20),
            pong_timeout: Duration::from_secs(10),
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            max_reconnect_attempts: 0, // unlimited
            channel_capacity: 256,
            auto_recovery_enabled: true,
            max_recovery_attempts: 5,
        }
    }
}

/// Unique identifier for a subscription
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub String);

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A stream of typed messages from a WebSocket subscription.
///
/// Wraps a `broadcast::Receiver<T>` and provides an async `.next()` method.
/// Multiple consumers can subscribe to the same stream (broadcast pattern).
pub struct WsStream<T: Clone> {
    receiver: broadcast::Receiver<T>,
    subscription_id: SubscriptionId,
}

impl<T: Clone> WsStream<T> {
    /// Create a new WsStream from a broadcast receiver
    pub fn new(receiver: broadcast::Receiver<T>, subscription_id: SubscriptionId) -> Self {
        Self {
            receiver,
            subscription_id,
        }
    }

    /// Receive the next message from the stream.
    ///
    /// Returns `None` if the channel is closed (all senders dropped).
    /// Skips over lagged messages (if consumer is too slow).
    pub async fn next(&mut self) -> Option<T> {
        loop {
            match self.receiver.recv().await {
                Ok(item) => return Some(item),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        "WsStream {} lagged by {} messages",
                        self.subscription_id,
                        n
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    /// Get the subscription ID for this stream
    pub fn subscription_id(&self) -> &SubscriptionId {
        &self.subscription_id
    }
}

/// WebSocket exchange trait for real-time data streams.
///
/// Parallel to the `Exchange` trait but provides streaming data
/// instead of request/response. Connections are lazy — WebSocket
/// connects only when the first `watch_*` method is called.
#[async_trait]
pub trait ExchangeWs: Send + Sync {
    // === Public Streams ===

    /// Watch real-time ticker updates for a symbol
    async fn watch_ticker(&self, symbol: &str) -> Result<WsStream<Ticker>>;

    /// Watch order book updates for a symbol
    async fn watch_order_book(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<WsStream<OrderBook>>;

    /// Watch trade stream for a symbol
    async fn watch_trades(&self, symbol: &str) -> Result<WsStream<Trade>>;

    /// Watch OHLCV candle updates for a symbol
    async fn watch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<WsStream<OHLCV>>;

    // === Private Streams (require authentication) ===

    /// Watch user's order updates (fills, cancellations, new orders)
    async fn watch_orders(&self, symbol: Option<&str>) -> Result<WsStream<Order>>;

    /// Watch balance changes
    async fn watch_balance(&self) -> Result<WsStream<Balances>>;

    /// Watch position updates (derivatives)
    async fn watch_positions(
        &self,
        symbols: Option<&[&str]>,
    ) -> Result<WsStream<Vec<Position>>>;

    /// Watch user's own trade executions
    async fn watch_my_trades(
        &self,
        symbol: Option<&str>,
    ) -> Result<WsStream<Trade>>;

    // === Connection Management ===

    /// Get the current WebSocket connection state
    fn connection_state(&self) -> WsConnectionState;

    /// Close all WebSocket connections
    async fn close(&self) -> Result<()>;
}
