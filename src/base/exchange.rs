//! Core Exchange trait and types
//!
//! All exchanges (CEX and DEX) implement the `Exchange` trait, providing a unified API.

use crate::base::errors::Result;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Type alias for exchange-specific parameters
pub type Params = HashMap<String, serde_json::Value>;

/// Exchange type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeType {
    /// Centralized exchange (Binance, Bybit, OKX)
    Cex,
    /// Decentralized exchange (Uniswap, PancakeSwap, Hyperliquid)
    Dex,
}

/// Exchange capabilities and feature support
#[derive(Debug, Clone)]
pub struct ExchangeFeatures {
    // === Market Data ===
    pub fetch_ticker: bool,
    pub fetch_tickers: bool,
    pub fetch_order_book: bool,
    pub fetch_ohlcv: bool,
    pub fetch_trades: bool,
    pub fetch_markets: bool,
    pub fetch_currencies: bool,
    pub fetch_status: bool,

    // === Trading ===
    pub create_order: bool,
    pub create_market_order: bool,
    pub create_limit_order: bool,
    pub cancel_order: bool,
    pub cancel_all_orders: bool,
    pub edit_order: bool,

    // === Order Queries ===
    pub fetch_order: bool,
    pub fetch_orders: bool,
    pub fetch_open_orders: bool,
    pub fetch_closed_orders: bool,
    pub fetch_my_trades: bool,

    // === Account ===
    pub fetch_balance: bool,
    pub fetch_deposit_address: bool,
    pub fetch_deposits: bool,
    pub fetch_withdrawals: bool,
    pub withdraw: bool,
    pub transfer: bool,

    // === Derivatives/Futures ===
    pub fetch_positions: bool,
    pub fetch_position: bool,
    pub fetch_funding_rate: bool,
    pub fetch_funding_rates: bool,
    pub fetch_funding_rate_history: bool,
    pub set_leverage: bool,
    pub set_margin_mode: bool,
    pub add_margin: bool,
    pub reduce_margin: bool,

    // === Advanced Features ===
    pub margin_trading: bool,
    pub futures_trading: bool,
    pub options_trading: bool,
    pub swap_trading: bool,
}

impl Default for ExchangeFeatures {
    fn default() -> Self {
        Self {
            fetch_ticker: false,
            fetch_tickers: false,
            fetch_order_book: false,
            fetch_ohlcv: false,
            fetch_trades: false,
            fetch_markets: false,
            fetch_currencies: false,
            fetch_status: false,
            create_order: false,
            create_market_order: false,
            create_limit_order: false,
            cancel_order: false,
            cancel_all_orders: false,
            edit_order: false,
            fetch_order: false,
            fetch_orders: false,
            fetch_open_orders: false,
            fetch_closed_orders: false,
            fetch_my_trades: false,
            fetch_balance: false,
            fetch_deposit_address: false,
            fetch_deposits: false,
            fetch_withdrawals: false,
            withdraw: false,
            transfer: false,
            fetch_positions: false,
            fetch_position: false,
            fetch_funding_rate: false,
            fetch_funding_rates: false,
            fetch_funding_rate_history: false,
            set_leverage: false,
            set_margin_mode: false,
            add_margin: false,
            reduce_margin: false,
            margin_trading: false,
            futures_trading: false,
            options_trading: false,
            swap_trading: false,
        }
    }
}

/// Core exchange trait implemented by all exchanges
///
/// This trait provides a unified API for both CEX and DEX exchanges.
/// Methods that an exchange doesn't support return `Err(CcxtError::NotSupported)`.
#[async_trait]
pub trait Exchange: Send + Sync {
    // === Identity ===

    /// Exchange identifier (lowercase, e.g., "binance", "uniswap")
    fn id(&self) -> &str;

    /// Exchange full name (e.g., "Binance", "Uniswap V3")
    fn name(&self) -> &str;

    /// Exchange type (CEX or DEX)
    fn exchange_type(&self) -> ExchangeType;

    /// Get exchange capabilities/features
    fn has(&self) -> &ExchangeFeatures;

    // === Market Data (Public) ===

    /// Load markets from exchange and cache internally
    async fn load_markets(&self) -> Result<Vec<Market>>;

    /// Fetch all markets from exchange (no caching)
    async fn fetch_markets(&self) -> Result<Vec<Market>>;

    /// Fetch all currencies/tokens
    async fn fetch_currencies(&self) -> Result<Vec<Currency>>;

    /// Fetch ticker for a single symbol
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol (e.g., "BTC/USDT", "ETH/USDC")
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker>;

    /// Fetch tickers for multiple symbols
    ///
    /// # Arguments
    /// * `symbols` - Optional list of symbols. If None, fetches all tickers.
    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>>;

    /// Fetch order book (bids and asks)
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `limit` - Optional depth limit
    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook>;

    /// Fetch OHLCV (candlestick) data
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `timeframe` - Candle timeframe (1m, 5m, 1h, 1d, etc.)
    /// * `since` - Start timestamp (milliseconds)
    /// * `limit` - Maximum number of candles
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>>;

    /// Fetch recent trades
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `since` - Start timestamp (milliseconds)
    /// * `limit` - Maximum number of trades
    async fn fetch_trades(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>>;

    /// Fetch exchange status
    async fn fetch_status(&self) -> Result<ExchangeStatus>;

    // === Trading (Private - requires credentials) ===

    /// Create an order
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `order_type` - Market, Limit, etc.
    /// * `side` - Buy or Sell
    /// * `amount` - Order amount (in base currency)
    /// * `price` - Order price (None for market orders)
    /// * `params` - Exchange-specific parameters
    async fn create_order(
        &self,
        symbol: &str,
        order_type: OrderType,
        side: OrderSide,
        amount: Decimal,
        price: Option<Decimal>,
        params: Option<&Params>,
    ) -> Result<Order>;

    /// Cancel an order
    ///
    /// # Arguments
    /// * `id` - Order ID
    /// * `symbol` - Optional symbol (required by some exchanges)
    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order>;

    /// Edit an existing order
    ///
    /// # Arguments
    /// * `id` - Order ID
    /// * `symbol` - Unified symbol
    /// * `order_type` - New order type
    /// * `side` - New side
    /// * `amount` - New amount (None to keep current)
    /// * `price` - New price (None to keep current)
    async fn edit_order(
        &self,
        id: &str,
        symbol: &str,
        order_type: OrderType,
        side: OrderSide,
        amount: Option<Decimal>,
        price: Option<Decimal>,
    ) -> Result<Order>;

    /// Fetch a single order by ID
    async fn fetch_order(&self, id: &str, symbol: Option<&str>) -> Result<Order>;

    /// Fetch all orders (open + closed)
    async fn fetch_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>>;

    /// Fetch open orders
    async fn fetch_open_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>>;

    /// Fetch closed orders
    async fn fetch_closed_orders(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Order>>;

    /// Fetch user's own trades
    async fn fetch_my_trades(
        &self,
        symbol: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>>;

    // === Account ===

    /// Fetch account balances
    async fn fetch_balance(&self) -> Result<Balances>;

    /// Fetch deposit address for a currency
    async fn fetch_deposit_address(&self, code: &str) -> Result<DepositAddress>;

    /// Fetch deposit history
    async fn fetch_deposits(
        &self,
        code: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Deposit>>;

    /// Fetch withdrawal history
    async fn fetch_withdrawals(
        &self,
        code: Option<&str>,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>>;

    /// Withdraw funds
    ///
    /// # Arguments
    /// * `code` - Currency code
    /// * `amount` - Withdrawal amount
    /// * `address` - Withdrawal address
    /// * `tag` - Optional address tag/memo
    async fn withdraw(
        &self,
        code: &str,
        amount: Decimal,
        address: &str,
        tag: Option<&str>,
    ) -> Result<Withdrawal>;

    /// Transfer funds between accounts
    ///
    /// # Arguments
    /// * `code` - Currency code
    /// * `amount` - Transfer amount
    /// * `from_account` - Source account type
    /// * `to_account` - Destination account type
    async fn transfer(
        &self,
        code: &str,
        amount: Decimal,
        from_account: &str,
        to_account: &str,
    ) -> Result<Transfer>;

    // === Derivatives / Futures ===

    /// Fetch open positions
    async fn fetch_positions(&self, symbols: Option<&[&str]>) -> Result<Vec<Position>>;

    /// Fetch funding rate for a symbol
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate>;

    /// Set leverage for a symbol
    ///
    /// # Arguments
    /// * `leverage` - Leverage multiplier (e.g., 10 for 10x)
    /// * `symbol` - Trading symbol
    async fn set_leverage(&self, leverage: u32, symbol: &str) -> Result<()>;

    /// Set margin mode for a symbol
    ///
    /// # Arguments
    /// * `mode` - Margin mode (Isolated or Cross)
    /// * `symbol` - Trading symbol
    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()>;

    // === Convenience Methods (CCXT-style helpers) ===
    // These are provided as default implementations

    /// Create a market buy order (convenience method)
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `amount` - Order amount (in base currency)
    /// * `params` - Exchange-specific parameters
    async fn create_market_buy_order(
        &self,
        symbol: &str,
        amount: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        self.create_order(symbol, OrderType::Market, OrderSide::Buy, amount, None, params)
            .await
    }

    /// Create a market sell order (convenience method)
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `amount` - Order amount (in base currency)
    /// * `params` - Exchange-specific parameters
    async fn create_market_sell_order(
        &self,
        symbol: &str,
        amount: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        self.create_order(symbol, OrderType::Market, OrderSide::Sell, amount, None, params)
            .await
    }

    /// Create a limit buy order (convenience method)
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `amount` - Order amount (in base currency)
    /// * `price` - Limit price
    /// * `params` - Exchange-specific parameters
    async fn create_limit_buy_order(
        &self,
        symbol: &str,
        amount: Decimal,
        price: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        self.create_order(
            symbol,
            OrderType::Limit,
            OrderSide::Buy,
            amount,
            Some(price),
            params,
        )
        .await
    }

    /// Create a limit sell order (convenience method)
    ///
    /// # Arguments
    /// * `symbol` - Unified symbol
    /// * `amount` - Order amount (in base currency)
    /// * `price` - Limit price
    /// * `params` - Exchange-specific parameters
    async fn create_limit_sell_order(
        &self,
        symbol: &str,
        amount: Decimal,
        price: Decimal,
        params: Option<&Params>,
    ) -> Result<Order> {
        self.create_order(
            symbol,
            OrderType::Limit,
            OrderSide::Sell,
            amount,
            Some(price),
            params,
        )
        .await
    }
}
