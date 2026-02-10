//! Hyperliquid WebSocket implementation
//!
//! URL: `wss://api.hyperliquid.xyz/ws` (mainnet) or testnet
//! No auth for public; private uses user address subscription
//!
//! Subscribe: `{"method":"subscribe","subscription":{"type":"l2Book","coin":"BTC"}}`
//! Private: `{"method":"subscribe","subscription":{"type":"userEvents","user":"0x..."}}`

use crate::base::errors::{CcxtError, Result};
use crate::base::ws::{ExchangeWs, NowOrNever, SubscriptionId, WsConfig, WsConnectionState, WsStream};
use crate::base::ws_connection::{WsConnectionManager, MessageHandler};
use crate::hyperliquid::parsers;
use crate::hyperliquid::types::{HlL2Book, HlRecentTrade, HlUserFill};
use crate::types::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

const HL_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const HL_WS_TESTNET_URL: &str = "wss://api.hyperliquid-testnet.xyz/ws";

/// Hyperliquid WebSocket client
pub struct HyperliquidWs {
    conn: Arc<WsConnectionManager>,

    ticker_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Ticker>>>>,
    orderbook_senders: Arc<RwLock<HashMap<String, broadcast::Sender<OrderBook>>>>,
    trade_senders: Arc<RwLock<HashMap<String, broadcast::Sender<Trade>>>>,

    order_sender: broadcast::Sender<Order>,
    balance_sender: broadcast::Sender<Balances>,
    position_sender: broadcast::Sender<Vec<Position>>,
    my_trade_sender: broadcast::Sender<Trade>,

    config: WsConfig,
    #[allow(dead_code)]
    sandbox: bool,
    user_address: Option<String>,
}

impl HyperliquidWs {
    /// Create a new Hyperliquid WebSocket client
    pub fn new(sandbox: bool, config: WsConfig) -> Self {
        let ws_url = if sandbox { HL_WS_TESTNET_URL } else { HL_WS_URL };
        let conn = WsConnectionManager::new(ws_url, config.clone());

        let (order_tx, _) = broadcast::channel(config.channel_capacity);
        let (balance_tx, _) = broadcast::channel(config.channel_capacity);
        let (position_tx, _) = broadcast::channel(config.channel_capacity);
        let (my_trade_tx, _) = broadcast::channel(config.channel_capacity);

        Self {
            conn: Arc::new(conn),
            ticker_senders: Arc::new(RwLock::new(HashMap::new())),
            orderbook_senders: Arc::new(RwLock::new(HashMap::new())),
            trade_senders: Arc::new(RwLock::new(HashMap::new())),
            order_sender: order_tx,
            balance_sender: balance_tx,
            position_sender: position_tx,
            my_trade_sender: my_trade_tx,
            config,
            sandbox,
            user_address: None,
        }
    }

    /// Set the user address for private subscriptions
    pub fn with_user_address(mut self, address: String) -> Self {
        self.user_address = Some(address);
        self
    }

    /// Convert unified symbol to Hyperliquid coin name
    /// "BTC/USD:USDC" → "BTC"
    fn coin_name(symbol: &str) -> String {
        symbol
            .split('/')
            .next()
            .unwrap_or(symbol)
            .to_string()
    }

    /// Build subscribe message
    fn subscribe_msg(subscription: &serde_json::Value) -> String {
        serde_json::json!({
            "method": "subscribe",
            "subscription": subscription
        })
        .to_string()
    }

    /// Setup message handler
    async fn setup_handler(&self) {
        let ticker_senders = self.ticker_senders.clone();
        let orderbook_senders = self.orderbook_senders.clone();
        let trade_senders = self.trade_senders.clone();
        let order_sender = self.order_sender.clone();
        let my_trade_sender = self.my_trade_sender.clone();
        let position_sender = self.position_sender.clone();

        let handler: MessageHandler = Arc::new(move |text: String| {
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };

            let channel = json.get("channel").and_then(|v| v.as_str()).unwrap_or("");

            match channel {
                "allMids" => {
                    // Mid prices for all assets — extract tickers
                    if let Some(data) = json.get("data").and_then(|d| d.get("mids")) {
                        if let Some(obj) = data.as_object() {
                            let now = crate::base::signer::timestamp_ms();
                            let dt = crate::base::signer::iso8601_now();
                            for (coin, mid) in obj {
                                let symbol = format!("{}/USD:USDC", coin);
                                if let Some(mid_str) = mid.as_str() {
                                    if let Ok(mid_val) = mid_str.parse::<rust_decimal::Decimal>() {
                                        let ticker = Ticker {
                                            symbol: symbol.clone(),
                                            timestamp: now,
                                            datetime: dt.clone(),
                                            high: None,
                                            low: None,
                                            bid: Some(mid_val),
                                            bid_volume: None,
                                            ask: Some(mid_val),
                                            ask_volume: None,
                                            vwap: None,
                                            open: None,
                                            close: Some(mid_val),
                                            last: Some(mid_val),
                                            previous_close: None,
                                            change: None,
                                            percentage: None,
                                            average: None,
                                            base_volume: None,
                                            quote_volume: None,
                                            index_price: None,
                                            mark_price: None,
                                            info: None,
                                        };
                                        let senders = ticker_senders.blocking_read();
                                        if let Some(tx) = senders.get(&symbol) {
                                            let _ = tx.send(ticker);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "l2Book" => {
                    if let Some(data) = json.get("data") {
                        let coin = data.get("coin").and_then(|v| v.as_str()).unwrap_or("");
                        let symbol = format!("{}/USD:USDC", coin);
                        // Deserialize into HlL2Book for the parser
                        if let Ok(book) = serde_json::from_value::<HlL2Book>(data.clone()) {
                            if let Ok(ob) = parsers::parse_order_book(&book, &symbol) {
                                let senders = orderbook_senders.blocking_read();
                                if let Some(tx) = senders.get(&symbol) {
                                    let _ = tx.send(ob);
                                }
                            }
                        }
                    }
                }
                "trades" => {
                    if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                        for trade_json in data {
                            let coin = trade_json.get("coin").and_then(|v| v.as_str()).unwrap_or("");
                            let symbol = format!("{}/USD:USDC", coin);
                            // Deserialize into HlRecentTrade for the parser
                            if let Ok(hl_trade) = serde_json::from_value::<HlRecentTrade>(trade_json.clone()) {
                                if let Ok(trades) = parsers::parse_trades(&[hl_trade], &symbol) {
                                    let senders = trade_senders.blocking_read();
                                    if let Some(tx) = senders.get(&symbol) {
                                        for trade in trades {
                                            let _ = tx.send(trade);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "userFills" => {
                    // User fills — dispatch to both order_sender and my_trade_sender
                    if let Some(data) = json.get("data") {
                        if let Some(fills) = data.as_array().or_else(|| data.get("fills").and_then(|v| v.as_array())) {
                            let hl_fills: Vec<HlUserFill> = fills
                                .iter()
                                .filter_map(|f| serde_json::from_value(f.clone()).ok())
                                .collect();
                            if let Ok(trades) = parsers::parse_user_fills(&hl_fills) {
                                for trade in &trades {
                                    // Send as my_trade
                                    let _ = my_trade_sender.send(trade.clone());
                                    // Also send as order (for watch_orders consumers)
                                    let _ = order_sender.send(Order {
                                        id: trade.id.clone(),
                                        client_order_id: None,
                                        symbol: trade.symbol.clone(),
                                        order_type: OrderType::Market,
                                        side: trade.side,
                                        status: OrderStatus::Closed,
                                        timestamp: trade.timestamp,
                                        datetime: trade.datetime.clone(),
                                        last_trade_timestamp: None,
                                        price: Some(trade.price),
                                        average: Some(trade.price),
                                        amount: trade.amount,
                                        filled: Some(trade.amount),
                                        remaining: Some(rust_decimal::Decimal::ZERO),
                                        cost: Some(trade.cost),
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
                                    });
                                }
                            }
                        }
                    }
                }
                "userEvents" => {
                    // User events — contains fills and position updates
                    if let Some(data) = json.get("data") {
                        // Parse fills from userEvents
                        if let Some(fills) = data.get("fills").and_then(|v| v.as_array()) {
                            let hl_fills: Vec<HlUserFill> = fills
                                .iter()
                                .filter_map(|f| serde_json::from_value(f.clone()).ok())
                                .collect();
                            if let Ok(trades) = parsers::parse_user_fills(&hl_fills) {
                                for trade in &trades {
                                    let _ = my_trade_sender.send(trade.clone());
                                    let _ = order_sender.send(Order {
                                        id: trade.id.clone(),
                                        client_order_id: None,
                                        symbol: trade.symbol.clone(),
                                        order_type: OrderType::Market,
                                        side: trade.side,
                                        status: OrderStatus::Closed,
                                        timestamp: trade.timestamp,
                                        datetime: trade.datetime.clone(),
                                        last_trade_timestamp: None,
                                        price: Some(trade.price),
                                        average: Some(trade.price),
                                        amount: trade.amount,
                                        filled: Some(trade.amount),
                                        remaining: Some(rust_decimal::Decimal::ZERO),
                                        cost: Some(trade.cost),
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
                                    });
                                }
                            }
                        }

                        // Parse position updates from userEvents
                        if let Some(ledger_updates) = data.get("ledgerUpdates").and_then(|v| v.as_array()) {
                            let now = crate::base::signer::timestamp_ms();
                            let dt = crate::base::signer::timestamp_to_iso8601(now);
                            let mut positions = Vec::new();
                            for update in ledger_updates {
                                if let Some(pos_data) = update.get("position") {
                                    let coin = pos_data.get("coin").and_then(|v| v.as_str()).unwrap_or("");
                                    let symbol = format!("{}/USD:USDC", coin);
                                    let szi = pos_data.get("szi").and_then(|v| v.as_str())
                                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                                        .unwrap_or(rust_decimal::Decimal::ZERO);
                                    let entry_px = pos_data.get("entryPx").and_then(|v| v.as_str())
                                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok());
                                    let unrealized_pnl = pos_data.get("unrealizedPnl").and_then(|v| v.as_str())
                                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok());
                                    let leverage_val = pos_data.get("leverage").and_then(|v| v.get("value"))
                                        .and_then(|v| v.as_str())
                                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

                                    let side = if szi > rust_decimal::Decimal::ZERO {
                                        PositionSide::Long
                                    } else if szi < rust_decimal::Decimal::ZERO {
                                        PositionSide::Short
                                    } else {
                                        PositionSide::Both
                                    };

                                    positions.push(Position {
                                        symbol,
                                        id: None,
                                        timestamp: now,
                                        datetime: dt.clone(),
                                        side,
                                        margin_mode: MarginMode::Cross,
                                        contracts: szi.abs(),
                                        contract_size: None,
                                        notional: None,
                                        leverage: leverage_val,
                                        entry_price: entry_px,
                                        mark_price: None,
                                        unrealized_pnl,
                                        realized_pnl: None,
                                        collateral: None,
                                        initial_margin: None,
                                        maintenance_margin: None,
                                        liquidation_price: None,
                                        margin_ratio: None,
                                        percentage: None,
                                        hedged: Some(false),
                                        maintenance_margin_percentage: None,
                                        initial_margin_percentage: None,
                                        last_update_timestamp: None,
                                        last_price: None,
                                        stop_loss_price: None,
                                        take_profit_price: None,
                                        info: Some(serde_json::json!(pos_data)),
                                    });
                                }
                            }
                            if !positions.is_empty() {
                                let _ = position_sender.send(positions);
                            }
                        }
                    }
                }
                "userFundings" => {
                    // Funding payments — ignore for now (no dedicated sender)
                    tracing::trace!("Hyperliquid WS: received userFundings event");
                }
                _ => {
                    tracing::trace!("Hyperliquid WS: unhandled channel: {}", channel);
                }
            }
        });

        self.conn.set_message_handler(handler).await;
    }
}

#[async_trait]
impl ExchangeWs for HyperliquidWs {
    async fn watch_ticker(&self, symbol: &str) -> Result<WsStream<Ticker>> {
        let sub_id = SubscriptionId(format!("allMids:{}", symbol));
        let sub = serde_json::json!({ "type": "allMids" });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = {
            let mut senders = self.ticker_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_order_book(&self, symbol: &str, _limit: Option<u32>) -> Result<WsStream<OrderBook>> {
        let coin = Self::coin_name(symbol);
        let sub_id = SubscriptionId(format!("l2Book:{}", coin));
        let sub = serde_json::json!({ "type": "l2Book", "coin": coin });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = {
            let mut senders = self.orderbook_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_trades(&self, symbol: &str) -> Result<WsStream<Trade>> {
        let coin = Self::coin_name(symbol);
        let sub_id = SubscriptionId(format!("trades:{}", coin));
        let sub = serde_json::json!({ "type": "trades", "coin": coin });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = {
            let mut senders = self.trade_senders.write().await;
            let tx = senders
                .entry(symbol.to_string())
                .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
            tx.subscribe()
        };

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_ohlcv(&self, _symbol: &str, _timeframe: Timeframe) -> Result<WsStream<OHLCV>> {
        Err(CcxtError::NotSupported(
            "Hyperliquid does not support OHLCV WebSocket streams".to_string(),
        ))
    }

    async fn watch_orders(&self, _symbol: Option<&str>) -> Result<WsStream<Order>> {
        let address = self.user_address.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("User address required for private streams".to_string())
        })?;

        let sub_id = SubscriptionId(format!("userEvents:{}", address));
        let sub = serde_json::json!({ "type": "userEvents", "user": address });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = self.order_sender.subscribe();

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_balance(&self) -> Result<WsStream<Balances>> {
        Err(CcxtError::NotSupported(
            "Hyperliquid balance WebSocket not available; use REST API".to_string(),
        ))
    }

    async fn watch_positions(&self, _symbols: Option<&[&str]>) -> Result<WsStream<Vec<Position>>> {
        let address = self.user_address.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("User address required for private streams".to_string())
        })?;

        let sub_id = SubscriptionId(format!("userEvents:{}", address));
        let sub = serde_json::json!({ "type": "userEvents", "user": address });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = self.position_sender.subscribe();

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    async fn watch_my_trades(&self, _symbol: Option<&str>) -> Result<WsStream<Trade>> {
        let address = self.user_address.as_ref().ok_or_else(|| {
            CcxtError::AuthenticationError("User address required for private streams".to_string())
        })?;

        let sub_id = SubscriptionId(format!("userFills:{}", address));
        let sub = serde_json::json!({ "type": "userFills", "user": address });
        let sub_msg = Self::subscribe_msg(&sub);

        let rx = self.my_trade_sender.subscribe();

        self.setup_handler().await;
        self.conn.subscribe(sub_id.clone(), sub_msg).await?;

        Ok(WsStream::new(rx, sub_id))
    }

    fn connection_state(&self) -> WsConnectionState {
        self.conn.connection_state().now_or_never()
            .unwrap_or(WsConnectionState::Disconnected)
    }

    async fn close(&self) -> Result<()> {
        self.conn.close().await
    }
}
