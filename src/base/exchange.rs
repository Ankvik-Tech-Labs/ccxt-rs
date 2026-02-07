//! Core Exchange trait and types
//!
//! All exchanges (CEX and DEX) implement the `Exchange` trait, providing a unified API.

use crate::base::errors::{CcxtError, Result};
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
    pub fetch_time: bool,
    pub fetch_bids_asks: bool,
    pub fetch_last_prices: bool,
    pub fetch_mark_prices: bool,
    pub fetch_l2_order_book: bool,
    pub fetch_index_ohlcv: bool,
    pub fetch_mark_ohlcv: bool,

    // === Trading ===
    pub create_order: bool,
    pub create_market_order: bool,
    pub create_limit_order: bool,
    pub cancel_order: bool,
    pub cancel_all_orders: bool,
    pub edit_order: bool,

    // === Batch Operations ===
    pub create_orders: bool,
    pub cancel_orders: bool,
    pub edit_orders: bool,

    // === Advanced Order Types ===
    pub create_stop_order: bool,
    pub create_stop_limit_order: bool,
    pub create_stop_market_order: bool,
    pub create_stop_loss_order: bool,
    pub create_take_profit_order: bool,
    pub create_trigger_order: bool,
    pub create_post_only_order: bool,
    pub create_reduce_only_order: bool,
    pub create_trailing_amount_order: bool,
    pub create_trailing_percent_order: bool,
    pub create_order_with_take_profit_and_stop_loss: bool,

    // === Order Queries ===
    pub fetch_order: bool,
    pub fetch_orders: bool,
    pub fetch_open_orders: bool,
    pub fetch_closed_orders: bool,
    pub fetch_canceled_orders: bool,
    pub fetch_my_trades: bool,
    pub fetch_order_trades: bool,

    // === Account ===
    pub fetch_balance: bool,
    pub fetch_accounts: bool,
    pub fetch_deposit_address: bool,
    pub fetch_deposit_addresses: bool,
    pub create_deposit_address: bool,
    pub fetch_deposits: bool,
    pub fetch_withdrawals: bool,
    pub withdraw: bool,
    pub transfer: bool,
    pub fetch_transfers: bool,

    // === Fees ===
    pub fetch_trading_fee: bool,
    pub fetch_trading_fees: bool,
    pub fetch_deposit_withdraw_fee: bool,
    pub fetch_deposit_withdraw_fees: bool,

    // === Derivatives/Futures ===
    pub fetch_positions: bool,
    pub fetch_position: bool,
    pub close_position: bool,
    pub close_all_positions: bool,
    pub set_position_mode: bool,
    pub fetch_position_mode: bool,
    pub fetch_position_history: bool,
    pub fetch_funding_rate: bool,
    pub fetch_funding_rates: bool,
    pub fetch_funding_rate_history: bool,
    pub fetch_funding_history: bool,
    pub set_leverage: bool,
    pub set_margin_mode: bool,
    pub fetch_leverage: bool,
    pub fetch_leverages: bool,
    pub fetch_leverage_tiers: bool,
    pub fetch_margin_mode: bool,
    pub add_margin: bool,
    pub reduce_margin: bool,
    pub set_margin: bool,

    // === Margin Borrowing ===
    pub borrow_cross_margin: bool,
    pub borrow_isolated_margin: bool,
    pub repay_cross_margin: bool,
    pub repay_isolated_margin: bool,
    pub fetch_borrow_rate: bool,
    pub fetch_borrow_rates: bool,
    pub fetch_cross_borrow_rate: bool,
    pub fetch_isolated_borrow_rate: bool,

    // === Options ===
    pub fetch_option: bool,
    pub fetch_option_chain: bool,
    pub fetch_greeks: bool,

    // === Open Interest & Liquidations ===
    pub fetch_open_interest: bool,
    pub fetch_open_interest_history: bool,
    pub fetch_liquidations: bool,
    pub fetch_my_liquidations: bool,
    pub fetch_long_short_ratio: bool,
    pub fetch_long_short_ratio_history: bool,

    // === Ledger & Conversions ===
    pub fetch_ledger: bool,
    pub fetch_convert_quote: bool,
    pub fetch_convert_trade_history: bool,

    // === Advanced Features ===
    pub margin_trading: bool,
    pub futures_trading: bool,
    pub options_trading: bool,
    pub swap_trading: bool,
    pub sandbox: bool,
    pub ws: bool,
}

impl Default for ExchangeFeatures {
    fn default() -> Self {
        Self {
            // Market Data
            fetch_ticker: false,
            fetch_tickers: false,
            fetch_order_book: false,
            fetch_ohlcv: false,
            fetch_trades: false,
            fetch_markets: false,
            fetch_currencies: false,
            fetch_status: false,
            fetch_time: false,
            fetch_bids_asks: false,
            fetch_last_prices: false,
            fetch_mark_prices: false,
            fetch_l2_order_book: false,
            fetch_index_ohlcv: false,
            fetch_mark_ohlcv: false,

            // Trading
            create_order: false,
            create_market_order: false,
            create_limit_order: false,
            cancel_order: false,
            cancel_all_orders: false,
            edit_order: false,

            // Batch Operations
            create_orders: false,
            cancel_orders: false,
            edit_orders: false,

            // Advanced Order Types
            create_stop_order: false,
            create_stop_limit_order: false,
            create_stop_market_order: false,
            create_stop_loss_order: false,
            create_take_profit_order: false,
            create_trigger_order: false,
            create_post_only_order: false,
            create_reduce_only_order: false,
            create_trailing_amount_order: false,
            create_trailing_percent_order: false,
            create_order_with_take_profit_and_stop_loss: false,

            // Order Queries
            fetch_order: false,
            fetch_orders: false,
            fetch_open_orders: false,
            fetch_closed_orders: false,
            fetch_canceled_orders: false,
            fetch_my_trades: false,
            fetch_order_trades: false,

            // Account
            fetch_balance: false,
            fetch_accounts: false,
            fetch_deposit_address: false,
            fetch_deposit_addresses: false,
            create_deposit_address: false,
            fetch_deposits: false,
            fetch_withdrawals: false,
            withdraw: false,
            transfer: false,
            fetch_transfers: false,

            // Fees
            fetch_trading_fee: false,
            fetch_trading_fees: false,
            fetch_deposit_withdraw_fee: false,
            fetch_deposit_withdraw_fees: false,

            // Derivatives/Futures
            fetch_positions: false,
            fetch_position: false,
            close_position: false,
            close_all_positions: false,
            set_position_mode: false,
            fetch_position_mode: false,
            fetch_position_history: false,
            fetch_funding_rate: false,
            fetch_funding_rates: false,
            fetch_funding_rate_history: false,
            fetch_funding_history: false,
            set_leverage: false,
            set_margin_mode: false,
            fetch_leverage: false,
            fetch_leverages: false,
            fetch_leverage_tiers: false,
            fetch_margin_mode: false,
            add_margin: false,
            reduce_margin: false,
            set_margin: false,

            // Margin Borrowing
            borrow_cross_margin: false,
            borrow_isolated_margin: false,
            repay_cross_margin: false,
            repay_isolated_margin: false,
            fetch_borrow_rate: false,
            fetch_borrow_rates: false,
            fetch_cross_borrow_rate: false,
            fetch_isolated_borrow_rate: false,

            // Options
            fetch_option: false,
            fetch_option_chain: false,
            fetch_greeks: false,

            // Open Interest & Liquidations
            fetch_open_interest: false,
            fetch_open_interest_history: false,
            fetch_liquidations: false,
            fetch_my_liquidations: false,
            fetch_long_short_ratio: false,
            fetch_long_short_ratio_history: false,

            // Ledger & Conversions
            fetch_ledger: false,
            fetch_convert_quote: false,
            fetch_convert_trade_history: false,

            // Advanced Features
            margin_trading: false,
            futures_trading: false,
            options_trading: false,
            swap_trading: false,
            sandbox: false,
            ws: false,
        }
    }
}

/// Core exchange trait implemented by all exchanges
///
/// This trait provides a unified API for both CEX and DEX exchanges.
/// Methods that an exchange doesn't support return `Err(CcxtError::NotSupported)`.
///
/// All new methods have default implementations that return `NotSupported`,
/// so existing exchange implementations continue to compile without changes.
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

    // ========================================================================
    // Market Data (Public)
    // ========================================================================

    /// Load markets from exchange and cache internally
    async fn load_markets(&self) -> Result<Vec<Market>>;

    /// Fetch all markets from exchange (no caching)
    async fn fetch_markets(&self) -> Result<Vec<Market>>;

    /// Fetch all currencies/tokens
    async fn fetch_currencies(&self) -> Result<Vec<Currency>>;

    /// Fetch ticker for a single symbol
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker>;

    /// Fetch tickers for multiple symbols
    async fn fetch_tickers(&self, symbols: Option<&[&str]>) -> Result<Vec<Ticker>>;

    /// Fetch order book (bids and asks)
    async fn fetch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook>;

    /// Fetch OHLCV (candlestick) data
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>>;

    /// Fetch recent trades
    async fn fetch_trades(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>>;

    /// Fetch exchange status
    async fn fetch_status(&self) -> Result<ExchangeStatus>;

    /// Fetch exchange server time (milliseconds)
    async fn fetch_time(&self) -> Result<i64> {
        Err(CcxtError::NotSupported("fetch_time".into()))
    }

    /// Fetch best bid/ask for multiple symbols
    async fn fetch_bids_asks(&self, _symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        Err(CcxtError::NotSupported("fetch_bids_asks".into()))
    }

    /// Fetch mark prices for symbols (derivatives)
    async fn fetch_mark_prices(&self, _symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        Err(CcxtError::NotSupported("fetch_mark_prices".into()))
    }

    // ========================================================================
    // Trading (Private - requires credentials)
    // ========================================================================

    /// Create an order
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
    async fn cancel_order(&self, id: &str, symbol: Option<&str>) -> Result<Order>;

    /// Edit an existing order
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

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple orders at once
    async fn create_orders(&self, _orders: &[OrderRequest]) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("create_orders".into()))
    }

    /// Cancel multiple orders by ID
    async fn cancel_orders(
        &self,
        _ids: &[&str],
        _symbol: Option<&str>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("cancel_orders".into()))
    }

    /// Cancel all orders (optionally for a symbol)
    async fn cancel_all_orders(&self, _symbol: Option<&str>) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("cancel_all_orders".into()))
    }

    /// Edit multiple orders at once
    async fn edit_orders(&self, _orders: &[EditOrderRequest]) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("edit_orders".into()))
    }

    // ========================================================================
    // Advanced Order Types
    // ========================================================================

    /// Create a stop order
    async fn create_stop_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _stop_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_stop_order".into()))
    }

    /// Create a stop-limit order
    async fn create_stop_limit_order(
        &self,
        _symbol: &str,
        _side: OrderSide,
        _amount: Decimal,
        _price: Decimal,
        _stop_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_stop_limit_order".into()))
    }

    /// Create a stop-market order
    async fn create_stop_market_order(
        &self,
        _symbol: &str,
        _side: OrderSide,
        _amount: Decimal,
        _stop_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_stop_market_order".into()))
    }

    /// Create a stop-loss order
    async fn create_stop_loss_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _stop_loss_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_stop_loss_order".into()))
    }

    /// Create a take-profit order
    async fn create_take_profit_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _take_profit_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_take_profit_order".into()))
    }

    /// Create an order with take-profit and stop-loss attached
    async fn create_order_with_take_profit_and_stop_loss(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _take_profit_price: Option<Decimal>,
        _stop_loss_price: Option<Decimal>,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "create_order_with_take_profit_and_stop_loss".into(),
        ))
    }

    /// Create a trigger order
    async fn create_trigger_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _trigger_price: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("create_trigger_order".into()))
    }

    /// Create a trailing-amount order
    async fn create_trailing_amount_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _trailing_amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "create_trailing_amount_order".into(),
        ))
    }

    /// Create a trailing-percent order
    async fn create_trailing_percent_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _trailing_percent: Decimal,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "create_trailing_percent_order".into(),
        ))
    }

    // ========================================================================
    // Order Queries (expanded)
    // ========================================================================

    /// Fetch canceled orders
    async fn fetch_canceled_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("fetch_canceled_orders".into()))
    }

    /// Fetch trades for a specific order
    async fn fetch_order_trades(
        &self,
        _id: &str,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        Err(CcxtError::NotSupported("fetch_order_trades".into()))
    }

    // ========================================================================
    // Account
    // ========================================================================

    /// Fetch account balances
    async fn fetch_balance(&self) -> Result<Balances>;

    /// Fetch sub-accounts
    async fn fetch_accounts(&self) -> Result<Vec<Account>> {
        Err(CcxtError::NotSupported("fetch_accounts".into()))
    }

    /// Fetch deposit address for a currency
    async fn fetch_deposit_address(&self, code: &str) -> Result<DepositAddress>;

    /// Fetch deposit addresses for multiple currencies
    async fn fetch_deposit_addresses(
        &self,
        _codes: Option<&[&str]>,
    ) -> Result<Vec<DepositAddress>> {
        Err(CcxtError::NotSupported("fetch_deposit_addresses".into()))
    }

    /// Create a new deposit address
    async fn create_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported("create_deposit_address".into()))
    }

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
    async fn withdraw(
        &self,
        code: &str,
        amount: Decimal,
        address: &str,
        tag: Option<&str>,
    ) -> Result<Withdrawal>;

    /// Transfer funds between accounts
    async fn transfer(
        &self,
        code: &str,
        amount: Decimal,
        from_account: &str,
        to_account: &str,
    ) -> Result<Transfer>;

    /// Fetch transfers
    async fn fetch_transfers(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Transfer>> {
        Err(CcxtError::NotSupported("fetch_transfers".into()))
    }

    // ========================================================================
    // Fees
    // ========================================================================

    /// Fetch trading fee for a single symbol
    async fn fetch_trading_fee(&self, _symbol: &str) -> Result<TradingFees> {
        Err(CcxtError::NotSupported("fetch_trading_fee".into()))
    }

    /// Fetch trading fees for all symbols
    async fn fetch_trading_fees(&self) -> Result<Vec<TradingFees>> {
        Err(CcxtError::NotSupported("fetch_trading_fees".into()))
    }

    /// Fetch deposit/withdraw fee for a currency
    async fn fetch_deposit_withdraw_fee(&self, _code: &str) -> Result<DepositWithdrawFee> {
        Err(CcxtError::NotSupported(
            "fetch_deposit_withdraw_fee".into(),
        ))
    }

    /// Fetch deposit/withdraw fees for multiple currencies
    async fn fetch_deposit_withdraw_fees(
        &self,
        _codes: Option<&[&str]>,
    ) -> Result<Vec<DepositWithdrawFee>> {
        Err(CcxtError::NotSupported(
            "fetch_deposit_withdraw_fees".into(),
        ))
    }

    // ========================================================================
    // Derivatives / Futures
    // ========================================================================

    /// Fetch open positions
    async fn fetch_positions(&self, symbols: Option<&[&str]>) -> Result<Vec<Position>>;

    /// Fetch a single position
    async fn fetch_position(&self, _symbol: &str) -> Result<Position> {
        Err(CcxtError::NotSupported("fetch_position".into()))
    }

    /// Close a position (creates a reduce-only order)
    async fn close_position(
        &self,
        _symbol: &str,
        _side: Option<OrderSide>,
        _params: Option<&Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported("close_position".into()))
    }

    /// Close all positions
    async fn close_all_positions(&self, _params: Option<&Params>) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported("close_all_positions".into()))
    }

    /// Set position mode (hedged or one-way)
    async fn set_position_mode(
        &self,
        _hedged: bool,
        _symbol: Option<&str>,
    ) -> Result<()> {
        Err(CcxtError::NotSupported("set_position_mode".into()))
    }

    /// Fetch current position mode
    async fn fetch_position_mode(
        &self,
        _symbol: Option<&str>,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("fetch_position_mode".into()))
    }

    /// Fetch position history
    async fn fetch_position_history(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Position>> {
        Err(CcxtError::NotSupported("fetch_position_history".into()))
    }

    /// Fetch funding rate for a symbol
    async fn fetch_funding_rate(&self, symbol: &str) -> Result<FundingRate>;

    /// Fetch funding rates for multiple symbols
    async fn fetch_funding_rates(
        &self,
        _symbols: Option<&[&str]>,
    ) -> Result<Vec<FundingRate>> {
        Err(CcxtError::NotSupported("fetch_funding_rates".into()))
    }

    /// Fetch funding rate history
    async fn fetch_funding_rate_history(
        &self,
        _symbol: &str,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<FundingRateHistory>> {
        Err(CcxtError::NotSupported(
            "fetch_funding_rate_history".into(),
        ))
    }

    /// Fetch funding payment history (user's own)
    async fn fetch_funding_history(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<FundingHistory>> {
        Err(CcxtError::NotSupported("fetch_funding_history".into()))
    }

    /// Set leverage for a symbol
    async fn set_leverage(&self, leverage: u32, symbol: &str) -> Result<()>;

    /// Set margin mode for a symbol
    async fn set_margin_mode(&self, mode: MarginMode, symbol: &str) -> Result<()>;

    /// Fetch leverage for a symbol
    async fn fetch_leverage(&self, _symbol: &str) -> Result<Leverage> {
        Err(CcxtError::NotSupported("fetch_leverage".into()))
    }

    /// Fetch leverages for multiple symbols
    async fn fetch_leverages(
        &self,
        _symbols: Option<&[&str]>,
    ) -> Result<Vec<Leverage>> {
        Err(CcxtError::NotSupported("fetch_leverages".into()))
    }

    /// Fetch leverage tiers for a symbol
    async fn fetch_leverage_tiers(
        &self,
        _symbols: Option<&[&str]>,
    ) -> Result<HashMap<String, Vec<LeverageTier>>> {
        Err(CcxtError::NotSupported("fetch_leverage_tiers".into()))
    }

    /// Fetch margin mode for a symbol
    async fn fetch_margin_mode(
        &self,
        _symbol: &str,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("fetch_margin_mode".into()))
    }

    /// Add margin to a position
    async fn add_margin(
        &self,
        _symbol: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<MarginModification> {
        Err(CcxtError::NotSupported("add_margin".into()))
    }

    /// Reduce margin from a position
    async fn reduce_margin(
        &self,
        _symbol: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<MarginModification> {
        Err(CcxtError::NotSupported("reduce_margin".into()))
    }

    /// Set margin for a position
    async fn set_margin(
        &self,
        _symbol: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<MarginModification> {
        Err(CcxtError::NotSupported("set_margin".into()))
    }

    // ========================================================================
    // Margin Borrowing
    // ========================================================================

    /// Borrow on cross margin
    async fn borrow_cross_margin(
        &self,
        _code: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("borrow_cross_margin".into()))
    }

    /// Borrow on isolated margin
    async fn borrow_isolated_margin(
        &self,
        _symbol: &str,
        _code: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("borrow_isolated_margin".into()))
    }

    /// Repay cross margin borrow
    async fn repay_cross_margin(
        &self,
        _code: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("repay_cross_margin".into()))
    }

    /// Repay isolated margin borrow
    async fn repay_isolated_margin(
        &self,
        _symbol: &str,
        _code: &str,
        _amount: Decimal,
        _params: Option<&Params>,
    ) -> Result<serde_json::Value> {
        Err(CcxtError::NotSupported("repay_isolated_margin".into()))
    }

    /// Fetch borrow rate for a currency
    async fn fetch_borrow_rate(&self, _code: &str) -> Result<BorrowRate> {
        Err(CcxtError::NotSupported("fetch_borrow_rate".into()))
    }

    /// Fetch cross borrow rate for a currency
    async fn fetch_cross_borrow_rate(&self, _code: &str) -> Result<BorrowRate> {
        Err(CcxtError::NotSupported("fetch_cross_borrow_rate".into()))
    }

    /// Fetch isolated borrow rate for a symbol
    async fn fetch_isolated_borrow_rate(&self, _symbol: &str) -> Result<BorrowRate> {
        Err(CcxtError::NotSupported(
            "fetch_isolated_borrow_rate".into(),
        ))
    }

    // ========================================================================
    // Options & Advanced Derivatives
    // ========================================================================

    /// Fetch a single option contract
    async fn fetch_option(&self, _symbol: &str) -> Result<OptionContract> {
        Err(CcxtError::NotSupported("fetch_option".into()))
    }

    /// Fetch option chain for a currency
    async fn fetch_option_chain(&self, _code: &str) -> Result<Vec<OptionContract>> {
        Err(CcxtError::NotSupported("fetch_option_chain".into()))
    }

    /// Fetch greeks for an option symbol
    async fn fetch_greeks(&self, _symbol: &str) -> Result<Greeks> {
        Err(CcxtError::NotSupported("fetch_greeks".into()))
    }

    /// Fetch open interest for a symbol
    async fn fetch_open_interest(&self, _symbol: &str) -> Result<OpenInterest> {
        Err(CcxtError::NotSupported("fetch_open_interest".into()))
    }

    /// Fetch open interest history
    async fn fetch_open_interest_history(
        &self,
        _symbol: &str,
        _timeframe: Option<Timeframe>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<OpenInterest>> {
        Err(CcxtError::NotSupported(
            "fetch_open_interest_history".into(),
        ))
    }

    /// Fetch recent liquidations for a symbol
    async fn fetch_liquidations(
        &self,
        _symbol: &str,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Liquidation>> {
        Err(CcxtError::NotSupported("fetch_liquidations".into()))
    }

    /// Fetch user's own liquidations
    async fn fetch_my_liquidations(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Liquidation>> {
        Err(CcxtError::NotSupported("fetch_my_liquidations".into()))
    }

    /// Fetch long/short ratio for a symbol
    async fn fetch_long_short_ratio(
        &self,
        _symbol: &str,
        _timeframe: Option<Timeframe>,
        _params: Option<&Params>,
    ) -> Result<LongShortRatio> {
        Err(CcxtError::NotSupported("fetch_long_short_ratio".into()))
    }

    /// Fetch long/short ratio history
    async fn fetch_long_short_ratio_history(
        &self,
        _symbol: &str,
        _timeframe: Option<Timeframe>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<LongShortRatio>> {
        Err(CcxtError::NotSupported(
            "fetch_long_short_ratio_history".into(),
        ))
    }

    // ========================================================================
    // Ledger & Conversions
    // ========================================================================

    /// Fetch ledger entries
    async fn fetch_ledger(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<LedgerEntry>> {
        Err(CcxtError::NotSupported("fetch_ledger".into()))
    }

    /// Fetch a conversion quote
    async fn fetch_convert_quote(
        &self,
        _from_code: &str,
        _to_code: &str,
        _amount: Decimal,
    ) -> Result<Conversion> {
        Err(CcxtError::NotSupported("fetch_convert_quote".into()))
    }

    // ========================================================================
    // Convenience Methods (CCXT-style helpers)
    // ========================================================================

    /// Create a market buy order (convenience method)
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
