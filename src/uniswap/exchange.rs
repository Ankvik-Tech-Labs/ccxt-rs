//! Uniswap V3 DEX exchange implementation
//!
//! This module provides a unified API for interacting with Uniswap V3 pools
//! across Ethereum, Polygon, and Arbitrum chains.
//!
//! # Symbol Format
//!
//! Uniswap V3 supports two symbol formats:
//!
//! - **Extended format**: `BASE/QUOTE:V3:FEE_TIER`
//!   - Example: `WETH/USDC:V3:3000` (0.3% fee pool)
//!   - Fee tiers: 100 (0.01%), 500 (0.05%), 3000 (0.3%), 10000 (1%)
//!
//! - **Short format**: `BASE/QUOTE`
//!   - Example: `WETH/USDC`
//!   - Automatically resolves to highest TVL pool for that pair
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use ccxt::{Exchange, UniswapV3};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Get API key from The Graph
//!     let api_key = std::env::var("THE_GRAPH_API_KEY")?;
//!
//!     // Build exchange for Ethereum
//!     let uniswap = UniswapV3::builder()
//!         .chain("ethereum")
//!         .subgraph_api_key(api_key)
//!         .build()
//!         .await?;
//!
//!     // Fetch markets
//!     let markets = uniswap.fetch_markets().await?;
//!     println!("Found {} pools", markets.len());
//!
//!     // Fetch ticker (default pool - highest TVL)
//!     let ticker = uniswap.fetch_ticker("WETH/USDC").await?;
//!     println!("WETH/USDC price: ${}", ticker.last.unwrap());
//!
//!     // Fetch ticker for specific fee tier
//!     let ticker = uniswap.fetch_ticker("WETH/USDC:V3:3000").await?;
//!     println!("WETH/USDC (0.3% pool) price: ${}", ticker.last.unwrap());
//!
//!     // Fetch recent trades
//!     let trades = uniswap.fetch_trades("WETH/USDC:V3:3000", None, Some(10)).await?;
//!
//!     // Fetch OHLCV candles (1h or 1d only)
//!     let ohlcv = uniswap.fetch_ohlcv(
//!         "WETH/USDC:V3:3000",
//!         ccxt::types::Timeframe::OneHour,
//!         None,
//!         Some(24)
//!     ).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Supported Chains
//!
//! - Ethereum (chain_id: 1)
//! - Polygon (chain_id: 137)
//! - Arbitrum (chain_id: 42161)
//!
//! # API Key
//!
//! Get a free API key from The Graph:
//! https://thegraph.com/studio/apikeys/
//!
//! # Limitations
//!
//! - No swap execution (read-only in this phase)
//! - OHLCV supports only 1h and 1d timeframes
//! - Requires The Graph subgraph (free tier available)
//! - New pools may take 1-5 minutes to appear in subgraph

use crate::base::{
    errors::{CcxtError, Result},
    exchange::{Exchange, ExchangeFeatures, ExchangeType},
};
use crate::dex::{EvmProvider, SubgraphClient};
use crate::types::*;
use crate::uniswap::{
    constants::{get_chain_config, get_chain_config_by_name, ChainConfig},
    parsers::*,
    pools::PoolManager,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;

/// Uniswap V3 DEX exchange
pub struct UniswapV3 {
    chain_config: &'static ChainConfig,
    pool_manager: PoolManager,
    markets: Arc<RwLock<Option<Vec<Market>>>>,
    features: ExchangeFeatures,
}

impl UniswapV3 {
    /// Create a new builder for UniswapV3
    pub fn builder() -> UniswapV3Builder {
        UniswapV3Builder::new()
    }

    /// Get pool manager
    pub fn pool_manager(&self) -> &PoolManager {
        &self.pool_manager
    }

    /// Get chain configuration
    pub fn chain_config(&self) -> &'static ChainConfig {
        self.chain_config
    }
}

/// Builder for UniswapV3 exchange
pub struct UniswapV3Builder {
    chain_id: Option<u64>,
    chain_name: Option<String>,
    subgraph_api_key: Option<String>,
    subgraph_url: Option<String>,
    rpc_url: Option<String>,
    rate_limit: bool,
    timeout: Duration,
}

impl UniswapV3Builder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            chain_id: None,
            chain_name: None,
            subgraph_api_key: None,
            subgraph_url: None,
            rpc_url: None,
            rate_limit: true,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set chain by ID (e.g., 1 for Ethereum)
    pub fn chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Set chain by name (e.g., "ethereum", "polygon", "arbitrum")
    pub fn chain(mut self, chain_name: &str) -> Self {
        self.chain_name = Some(chain_name.to_string());
        self
    }

    /// Set The Graph API key (required)
    pub fn subgraph_api_key(mut self, api_key: String) -> Self {
        self.subgraph_api_key = Some(api_key);
        self
    }

    /// Set custom subgraph URL (optional, for self-hosted)
    pub fn subgraph_url(mut self, url: String) -> Self {
        self.subgraph_url = Some(url);
        self
    }

    /// Set custom RPC URL (optional, uses chain default)
    pub fn rpc_url(mut self, url: String) -> Self {
        self.rpc_url = Some(url);
        self
    }

    /// Enable/disable rate limiting (default: true)
    pub fn rate_limit(mut self, enabled: bool) -> Self {
        self.rate_limit = enabled;
        self
    }

    /// Set request timeout (default: 30s)
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the UniswapV3 instance
    pub async fn build(self) -> Result<UniswapV3> {
        // Validate required fields
        let _api_key = self.subgraph_api_key.ok_or_else(|| {
            CcxtError::ConfigError("Subgraph API key is required. Get one from https://thegraph.com/studio/apikeys/".to_string())
        })?;

        // Get chain config
        let chain_config = if let Some(chain_id) = self.chain_id {
            get_chain_config(chain_id).ok_or_else(|| {
                CcxtError::ConfigError(format!("Unsupported chain ID: {}", chain_id))
            })?
        } else if let Some(chain_name) = &self.chain_name {
            get_chain_config_by_name(chain_name).ok_or_else(|| {
                CcxtError::ConfigError(format!("Unsupported chain: {}", chain_name))
            })?
        } else {
            return Err(CcxtError::ConfigError(
                "Either chain_id or chain name must be specified".to_string(),
            ));
        };

        // Build subgraph URL
        let subgraph_url = if let Some(url) = self.subgraph_url {
            url
        } else {
            // Use the chain config's subgraph endpoint
            chain_config.subgraph_v3.to_string()
        };

        // Build RPC URL
        let rpc_url = self.rpc_url.unwrap_or_else(|| chain_config.rpc_url.to_string());

        // Create subgraph client
        let subgraph = Arc::new(SubgraphClient::new(subgraph_url));

        // Create EVM provider
        let provider = Arc::new(EvmProvider::new(&rpc_url, chain_config.chain_id).await?);

        // Create pool manager
        let pool_manager = PoolManager::new(subgraph, provider);

        // Define features
        let features = ExchangeFeatures {
            fetch_ticker: true,
            fetch_tickers: false,
            fetch_order_book: false,
            fetch_ohlcv: true,
            fetch_trades: true,
            fetch_markets: true,
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
        };

        Ok(UniswapV3 {
            chain_config,
            pool_manager,
            markets: Arc::new(RwLock::new(None)),
            features,
        })
    }
}

impl Default for UniswapV3Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Exchange for UniswapV3 {
    fn id(&self) -> &str {
        "uniswap"
    }

    fn name(&self) -> &str {
        "Uniswap V3"
    }

    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Dex
    }

    fn has(&self) -> &ExchangeFeatures {
        &self.features
    }

    async fn load_markets(&self) -> Result<Vec<Market>> {
        let markets = self.fetch_markets().await?;
        *self.markets.write().await = Some(markets.clone());
        Ok(markets)
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        // Discover pools with TVL > $10k
        let pools = self.pool_manager.discover_pools(10000.0).await?;

        let mut markets = Vec::new();

        for pool in pools {
            // Determine base/quote order (token0/token1 or token1/token0)
            let (base, quote, fee_tier) = (
                &pool.token0_symbol,
                &pool.token1_symbol,
                pool.fee_tier,
            );

            match parse_pool_to_market(
                &serde_json::to_value(&pool).unwrap_or_default(),
                base,
                quote,
                fee_tier,
            ) {
                Ok(market) => markets.push(market),
                Err(_) => continue, // Skip pools that fail to parse
            }

            // Also add reversed pair
            match parse_pool_to_market(
                &serde_json::to_value(&pool).unwrap_or_default(),
                quote,
                base,
                fee_tier,
            ) {
                Ok(market) => markets.push(market),
                Err(_) => continue,
            }
        }

        Ok(markets)
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let parsed = parse_uniswap_symbol(symbol)?;

        // Get pool info
        let pool = if let Some(fee_tier) = parsed.fee_tier {
            self.pool_manager
                .get_pool_exact(&parsed.base, &parsed.quote, fee_tier)
                .await?
        } else {
            self.pool_manager
                .get_pool_highest_liquidity(&parsed.base, &parsed.quote)
                .await?
        };

        // Determine if tokens are inverted
        let inverted = pool.token0_symbol != parsed.base;

        // Get current price from on-chain
        let price = self
            .pool_manager
            .get_current_price(
                pool.address,
                pool.token0_decimals,
                pool.token1_decimals,
                inverted,
            )
            .await?;

        let symbol_formatted =
            format_uniswap_symbol(&parsed.base, &parsed.quote, Some(pool.fee_tier));

        Ok(Ticker {
            symbol: symbol_formatted,
            timestamp: chrono::Utc::now().timestamp_millis(),
            datetime: chrono::Utc::now().to_rfc3339(),
            high: None,
            low: None,
            bid: None,
            bid_volume: None,
            ask: None,
            ask_volume: None,
            vwap: None,
            open: None,
            close: Some(price),
            last: Some(price),
            previous_close: None,
            change: None,
            percentage: None,
            average: None,
            base_volume: None,
            quote_volume: None,
            info: Some(serde_json::json!({
                "pool_address": pool.address.to_string(),
                "tvl_usd": pool.tvl_usd,
                "liquidity": pool.liquidity,
            })),
        })
    }

    async fn fetch_trades(
        &self,
        symbol: &str,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        let parsed = parse_uniswap_symbol(symbol)?;

        // Get pool info
        let pool = if let Some(fee_tier) = parsed.fee_tier {
            self.pool_manager
                .get_pool_exact(&parsed.base, &parsed.quote, fee_tier)
                .await?
        } else {
            self.pool_manager
                .get_pool_highest_liquidity(&parsed.base, &parsed.quote)
                .await?
        };

        let inverted = pool.token0_symbol != parsed.base;

        // Build GraphQL query for swaps
        let query = r#"
            query GetSwaps($poolId: ID!, $minTime: Int!, $limit: Int!) {
                swaps(
                    where: { pool: $poolId, timestamp_gte: $minTime }
                    orderBy: timestamp
                    orderDirection: desc
                    first: $limit
                ) {
                    id
                    timestamp
                    amount0
                    amount1
                    amountUSD
                    sqrtPriceX96
                    tick
                    sender
                    recipient
                }
            }
        "#;

        let min_time = since.unwrap_or(0) / 1000; // Convert to seconds
        let limit = limit.unwrap_or(100).min(1000) as i64; // Cap at 1000

        let mut variables = HashMap::new();
        variables.insert("poolId".to_string(), serde_json::json!(pool.address.to_string().to_lowercase()));
        variables.insert("minTime".to_string(), serde_json::json!(min_time));
        variables.insert("limit".to_string(), serde_json::json!(limit));

        let subgraph = self.pool_manager.subgraph();
        let data = subgraph.query(query, Some(variables)).await?;

        let swaps = &data["swaps"];

        let symbol_formatted =
            format_uniswap_symbol(&parsed.base, &parsed.quote, Some(pool.fee_tier));

        parse_swaps_to_trades(
            swaps,
            &symbol_formatted,
            &parsed.base,
            &parsed.quote,
            pool.token0_decimals,
            pool.token1_decimals,
            pool.fee_tier,
            inverted,
        )
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        since: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<OHLCV>> {
        // Only support 1h and 1d timeframes
        let (data_type, _period_seconds) = match timeframe {
            Timeframe::OneHour => ("poolHourData", 3600),
            Timeframe::OneDay => ("poolDayData", 86400),
            _ => {
                return Err(CcxtError::NotSupported(format!(
                    "Timeframe {:?} not supported. Only 1h and 1d are available.",
                    timeframe
                )))
            }
        };

        let parsed = parse_uniswap_symbol(symbol)?;

        // Get pool info
        let pool = if let Some(fee_tier) = parsed.fee_tier {
            self.pool_manager
                .get_pool_exact(&parsed.base, &parsed.quote, fee_tier)
                .await?
        } else {
            self.pool_manager
                .get_pool_highest_liquidity(&parsed.base, &parsed.quote)
                .await?
        };

        let query = format!(
            r#"
            query GetCandles($poolId: ID!, $minTime: Int!, $limit: Int!) {{
                {}(
                    where: {{ pool: $poolId, periodStartUnix_gte: $minTime }}
                    orderBy: periodStartUnix
                    orderDirection: asc
                    first: $limit
                ) {{
                    periodStartUnix
                    open
                    high
                    low
                    close
                    volumeToken0
                    volumeToken1
                    volumeUSD
                }}
            }}
        "#,
            data_type
        );

        let min_time = since.unwrap_or(0) / 1000; // Convert to seconds
        let limit = limit.unwrap_or(100).min(1000) as i64; // Cap at 1000

        let mut variables = HashMap::new();
        variables.insert("poolId".to_string(), serde_json::json!(pool.address.to_string().to_lowercase()));
        variables.insert("minTime".to_string(), serde_json::json!(min_time));
        variables.insert("limit".to_string(), serde_json::json!(limit));

        let subgraph = self.pool_manager.subgraph();
        let data = subgraph.query(&query, Some(variables)).await?;

        let candles = &data[data_type];

        let symbol_formatted =
            format_uniswap_symbol(&parsed.base, &parsed.quote, Some(pool.fee_tier));

        parse_candles_to_ohlcv(candles, &symbol_formatted)
    }

    // === Unsupported Market Data Methods ===

    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(CcxtError::NotSupported(
            "fetch_currencies not supported".to_string(),
        ))
    }

    async fn fetch_tickers(&self, _symbols: Option<&[&str]>) -> Result<Vec<Ticker>> {
        Err(CcxtError::NotSupported(
            "fetch_tickers not supported".to_string(),
        ))
    }

    async fn fetch_order_book(&self, _symbol: &str, _limit: Option<u32>) -> Result<OrderBook> {
        Err(CcxtError::NotSupported(
            "fetch_order_book not supported for Uniswap V3".to_string(),
        ))
    }

    async fn fetch_status(&self) -> Result<ExchangeStatus> {
        Err(CcxtError::NotSupported(
            "fetch_status not supported".to_string(),
        ))
    }

    // === Unsupported Trading Methods ===

    async fn create_order(
        &self,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Decimal,
        _price: Option<Decimal>,
        _params: Option<&crate::base::exchange::Params>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "create_order not supported in this phase. Swap execution coming in Phase 2D."
                .to_string(),
        ))
    }

    async fn cancel_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "cancel_order not supported".to_string(),
        ))
    }

    async fn edit_order(
        &self,
        _id: &str,
        _symbol: &str,
        _order_type: OrderType,
        _side: OrderSide,
        _amount: Option<Decimal>,
        _price: Option<Decimal>,
    ) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "edit_order not supported".to_string(),
        ))
    }

    // === Unsupported Order Query Methods ===

    async fn fetch_order(&self, _id: &str, _symbol: Option<&str>) -> Result<Order> {
        Err(CcxtError::NotSupported(
            "fetch_order not supported".to_string(),
        ))
    }

    async fn fetch_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported(
            "fetch_orders not supported".to_string(),
        ))
    }

    async fn fetch_open_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported(
            "fetch_open_orders not supported".to_string(),
        ))
    }

    async fn fetch_closed_orders(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Order>> {
        Err(CcxtError::NotSupported(
            "fetch_closed_orders not supported".to_string(),
        ))
    }

    async fn fetch_my_trades(
        &self,
        _symbol: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Trade>> {
        Err(CcxtError::NotSupported(
            "fetch_my_trades not supported".to_string(),
        ))
    }

    // === Unsupported Account Methods ===

    async fn fetch_balance(&self) -> Result<Balances> {
        Err(CcxtError::NotSupported(
            "fetch_balance not supported".to_string(),
        ))
    }

    async fn fetch_deposit_address(&self, _code: &str) -> Result<DepositAddress> {
        Err(CcxtError::NotSupported(
            "fetch_deposit_address not supported".to_string(),
        ))
    }

    async fn fetch_deposits(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Deposit>> {
        Err(CcxtError::NotSupported(
            "fetch_deposits not supported".to_string(),
        ))
    }

    async fn fetch_withdrawals(
        &self,
        _code: Option<&str>,
        _since: Option<i64>,
        _limit: Option<u32>,
    ) -> Result<Vec<Withdrawal>> {
        Err(CcxtError::NotSupported(
            "fetch_withdrawals not supported".to_string(),
        ))
    }

    async fn withdraw(
        &self,
        _code: &str,
        _amount: Decimal,
        _address: &str,
        _tag: Option<&str>,
    ) -> Result<Withdrawal> {
        Err(CcxtError::NotSupported(
            "withdraw not supported".to_string(),
        ))
    }

    async fn transfer(
        &self,
        _code: &str,
        _amount: Decimal,
        _from_account: &str,
        _to_account: &str,
    ) -> Result<Transfer> {
        Err(CcxtError::NotSupported(
            "transfer not supported".to_string(),
        ))
    }

    // === Unsupported Derivatives Methods ===

    async fn fetch_positions(&self, _symbols: Option<&[&str]>) -> Result<Vec<Position>> {
        Err(CcxtError::NotSupported(
            "fetch_positions not supported".to_string(),
        ))
    }

    async fn fetch_funding_rate(&self, _symbol: &str) -> Result<FundingRate> {
        Err(CcxtError::NotSupported(
            "fetch_funding_rate not supported".to_string(),
        ))
    }

    async fn set_leverage(&self, _leverage: u32, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported(
            "set_leverage not supported".to_string(),
        ))
    }

    async fn set_margin_mode(&self, _mode: MarginMode, _symbol: &str) -> Result<()> {
        Err(CcxtError::NotSupported(
            "set_margin_mode not supported".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let builder = UniswapV3::builder();
        assert!(builder.chain_id.is_none());
        assert!(builder.subgraph_api_key.is_none());
    }

    #[test]
    fn test_builder_chain_configuration() {
        let builder = UniswapV3::builder()
            .chain("ethereum")
            .subgraph_api_key("test_key".to_string());

        assert_eq!(builder.chain_name, Some("ethereum".to_string()));
        assert_eq!(
            builder.subgraph_api_key,
            Some("test_key".to_string())
        );
    }
}
