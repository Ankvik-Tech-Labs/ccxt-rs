//! Pool management and on-chain price queries for Uniswap V3
//!
//! This module handles:
//! - Discovering pools via The Graph subgraph
//! - Resolving symbols to pool addresses
//! - Querying on-chain pool state (sqrtPriceX96, liquidity, etc.)
//! - Converting Uniswap math to human-readable prices

use crate::base::errors::{CcxtError, Result};
use crate::dex::{EvmProvider, SubgraphClient};
use crate::uniswap::constants::FeeTier;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::RwLock;

// Uniswap V3 Pool ABI
sol! {
    interface IUniswapV3Pool {
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );

        function token0() external view returns (address);
        function token1() external view returns (address);
        function fee() external view returns (uint24);
        function liquidity() external view returns (uint128);
    }
}

/// Pool metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolInfo {
    pub address: Address,
    pub token0_address: Address,
    pub token1_address: Address,
    pub token0_symbol: String,
    pub token1_symbol: String,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub fee_tier: FeeTier,
    pub liquidity: String,
    pub tvl_usd: Decimal,
}

/// Pool manager for discovering and querying Uniswap V3 pools
pub struct PoolManager {
    subgraph: Arc<SubgraphClient>,
    provider: Arc<EvmProvider>,
    pools_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
}

impl PoolManager {
    /// Create a new pool manager
    pub fn new(subgraph: Arc<SubgraphClient>, provider: Arc<EvmProvider>) -> Self {
        Self {
            subgraph,
            provider,
            pools_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get reference to the subgraph client
    pub fn subgraph(&self) -> &Arc<SubgraphClient> {
        &self.subgraph
    }

    /// Discover all pools with TVL above threshold
    pub async fn discover_pools(&self, min_tvl_usd: f64) -> Result<Vec<PoolInfo>> {
        let query = r#"
            query GetPools($minTvl: BigDecimal) {
                pools(
                    first: 1000
                    where: { totalValueLockedUSD_gt: $minTvl }
                    orderBy: totalValueLockedUSD
                    orderDirection: desc
                ) {
                    id
                    token0 {
                        id
                        symbol
                        decimals
                    }
                    token1 {
                        id
                        symbol
                        decimals
                    }
                    feeTier
                    liquidity
                    totalValueLockedUSD
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("minTvl".to_string(), json!(min_tvl_usd.to_string()));

        let data = self.subgraph.query(query, Some(variables)).await?;

        let pools = data["pools"]
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Pools is not an array".to_string()))?;

        let mut pool_infos = Vec::new();

        for pool in pools {
            if let Ok(pool_info) = self.parse_pool_info(pool) {
                pool_infos.push(pool_info);
            }
        }

        // Cache discovered pools
        let mut cache = self.pools_cache.write().await;
        for pool in &pool_infos {
            let key = format!(
                "{}/{}/{}",
                pool.token0_symbol, pool.token1_symbol, pool.fee_tier.as_basis_points()
            );
            cache.insert(key.clone(), pool.clone());

            // Also cache reversed order
            let key_reversed = format!(
                "{}/{}/{}",
                pool.token1_symbol, pool.token0_symbol, pool.fee_tier.as_basis_points()
            );
            cache.insert(key_reversed, pool.clone());
        }

        Ok(pool_infos)
    }

    /// Get pool for a specific pair and fee tier
    pub async fn get_pool_exact(
        &self,
        base: &str,
        quote: &str,
        fee_tier: FeeTier,
    ) -> Result<PoolInfo> {
        // Check cache first
        let cache_key = format!("{}/{}/{}", base, quote, fee_tier.as_basis_points());
        {
            let cache = self.pools_cache.read().await;
            if let Some(pool) = cache.get(&cache_key) {
                return Ok(pool.clone());
            }
        }

        // Query subgraph
        let query = r#"
            query GetPoolExact($token0: String!, $token1: String!, $fee: Int!) {
                pools(
                    where: {
                        or: [
                            { token0_: { symbol: $token0 }, token1_: { symbol: $token1 }, feeTier: $fee },
                            { token0_: { symbol: $token1 }, token1_: { symbol: $token0 }, feeTier: $fee }
                        ]
                    }
                    first: 1
                ) {
                    id
                    token0 {
                        id
                        symbol
                        decimals
                    }
                    token1 {
                        id
                        symbol
                        decimals
                    }
                    feeTier
                    liquidity
                    totalValueLockedUSD
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("token0".to_string(), json!(base));
        variables.insert("token1".to_string(), json!(quote));
        variables.insert("fee".to_string(), json!(fee_tier.as_basis_points()));

        let data = self.subgraph.query(query, Some(variables)).await?;

        let pools = data["pools"]
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Pools is not an array".to_string()))?;

        if pools.is_empty() {
            return Err(CcxtError::BadSymbol(format!(
                "Pool not found: {}/{} with fee tier {}",
                base,
                quote,
                fee_tier.as_basis_points()
            )));
        }

        let pool_info = self.parse_pool_info(&pools[0])?;

        // Cache the result
        let mut cache = self.pools_cache.write().await;
        cache.insert(cache_key, pool_info.clone());

        Ok(pool_info)
    }

    /// Get pool with highest liquidity for a pair
    pub async fn get_pool_highest_liquidity(&self, base: &str, quote: &str) -> Result<PoolInfo> {
        let query = r#"
            query GetPoolHighestLiquidity($token0: String!, $token1: String!) {
                pools(
                    where: {
                        or: [
                            { token0_: { symbol: $token0 }, token1_: { symbol: $token1 } },
                            { token0_: { symbol: $token1 }, token1_: { symbol: $token0 } }
                        ]
                    }
                    orderBy: totalValueLockedUSD
                    orderDirection: desc
                    first: 1
                ) {
                    id
                    token0 {
                        id
                        symbol
                        decimals
                    }
                    token1 {
                        id
                        symbol
                        decimals
                    }
                    feeTier
                    liquidity
                    totalValueLockedUSD
                }
            }
        "#;

        let mut variables = HashMap::new();
        variables.insert("token0".to_string(), json!(base));
        variables.insert("token1".to_string(), json!(quote));

        let data = self.subgraph.query(query, Some(variables)).await?;

        let pools = data["pools"]
            .as_array()
            .ok_or_else(|| CcxtError::ParseError("Pools is not an array".to_string()))?;

        if pools.is_empty() {
            return Err(CcxtError::BadSymbol(format!(
                "No pools found for pair: {}/{}",
                base, quote
            )));
        }

        self.parse_pool_info(&pools[0])
    }

    /// Get current price from on-chain slot0
    pub async fn get_current_price(
        &self,
        pool_address: Address,
        token0_decimals: u8,
        token1_decimals: u8,
        inverted: bool,
    ) -> Result<Decimal> {
        use alloy::sol_types::SolCall;

        // Build the slot0() call data
        let call_data = IUniswapV3Pool::slot0Call {}.abi_encode();

        // Create transaction request
        let tx = alloy::rpc::types::TransactionRequest::default()
            .to(pool_address)
            .input(call_data.into());

        // Execute the call
        let result = self
            .provider
            .provider()
            .call(&tx)
            .await
            .map_err(|e| CcxtError::NetworkError(format!("Failed to call slot0: {}", e)))?;

        // Decode the response
        let slot0_return = IUniswapV3Pool::slot0Call::abi_decode_returns(&result, false)
            .map_err(|e| CcxtError::ParseError(format!("Failed to decode slot0 response: {}", e)))?;

        // Convert U160 to u128 for the price calculation
        let sqrt_price_x96: u128 = slot0_return
            .sqrtPriceX96
            .try_into()
            .map_err(|_| CcxtError::ParseError("sqrtPriceX96 overflow".to_string()))?;

        sqrt_price_x96_to_price(sqrt_price_x96, token0_decimals, token1_decimals, inverted)
    }

    /// Parse pool info from subgraph response
    fn parse_pool_info(&self, pool: &Value) -> Result<PoolInfo> {
        let address_str = pool["id"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing pool id".to_string()))?;
        let address = Address::from_str(address_str)
            .map_err(|_| CcxtError::ParseError(format!("Invalid address: {}", address_str)))?;

        let token0 = &pool["token0"];
        let token1 = &pool["token1"];

        let token0_address_str = token0["id"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing token0 id".to_string()))?;
        let token0_address = Address::from_str(token0_address_str).map_err(|_| {
            CcxtError::ParseError(format!("Invalid token0 address: {}", token0_address_str))
        })?;

        let token1_address_str = token1["id"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing token1 id".to_string()))?;
        let token1_address = Address::from_str(token1_address_str).map_err(|_| {
            CcxtError::ParseError(format!("Invalid token1 address: {}", token1_address_str))
        })?;

        let token0_symbol = token0["symbol"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing token0 symbol".to_string()))?
            .to_string();

        let token1_symbol = token1["symbol"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing token1 symbol".to_string()))?
            .to_string();

        let token0_decimals = token0["decimals"]
            .as_str()
            .and_then(|s| s.parse::<u8>().ok())
            .ok_or_else(|| CcxtError::ParseError("Missing token0 decimals".to_string()))?;

        let token1_decimals = token1["decimals"]
            .as_str()
            .and_then(|s| s.parse::<u8>().ok())
            .ok_or_else(|| CcxtError::ParseError("Missing token1 decimals".to_string()))?;

        let fee_tier_value = if let Some(s) = pool["feeTier"].as_str() {
            s.parse::<u32>().ok()
        } else if let Some(n) = pool["feeTier"].as_u64() {
            Some(n as u32)
        } else {
            None
        }
        .ok_or_else(|| CcxtError::ParseError("Missing fee tier".to_string()))?;

        let fee_tier = FeeTier::from_basis_points(fee_tier_value)?;

        let liquidity = pool["liquidity"]
            .as_str()
            .unwrap_or("0")
            .to_string();

        let tvl_usd_str = pool["totalValueLockedUSD"]
            .as_str()
            .unwrap_or("0");
        let tvl_usd = Decimal::from_str(tvl_usd_str).unwrap_or(Decimal::ZERO);

        Ok(PoolInfo {
            address,
            token0_address,
            token1_address,
            token0_symbol,
            token1_symbol,
            token0_decimals,
            token1_decimals,
            fee_tier,
            liquidity,
            tvl_usd,
        })
    }
}

/// Convert Uniswap sqrtPriceX96 to human-readable price
///
/// Formula: price = (sqrtPriceX96 / 2^96)^2
/// Then adjust for token decimals
pub fn sqrt_price_x96_to_price(
    sqrt_price_x96: u128,
    token0_decimals: u8,
    token1_decimals: u8,
    inverted: bool,
) -> Result<Decimal> {
    // Convert to U256 for calculations
    let sqrt_price = U256::from(sqrt_price_x96);
    let q96 = U256::from(1u128 << 96);

    // price = (sqrtPrice / 2^96)^2
    let price_raw = sqrt_price
        .checked_mul(sqrt_price)
        .ok_or_else(|| CcxtError::ParseError("Price calculation overflow".to_string()))?
        .checked_div(q96)
        .ok_or_else(|| CcxtError::ParseError("Price calculation division error".to_string()))?
        .checked_div(q96)
        .ok_or_else(|| CcxtError::ParseError("Price calculation division error".to_string()))?;

    // Adjust for token decimals
    let decimal_adjustment = if token1_decimals >= token0_decimals {
        U256::from(10u128.pow((token1_decimals - token0_decimals) as u32))
    } else {
        U256::from(1)
    };

    let adjusted_price = if token1_decimals >= token0_decimals {
        price_raw
            .checked_mul(decimal_adjustment)
            .ok_or_else(|| CcxtError::ParseError("Decimal adjustment overflow".to_string()))?
    } else {
        price_raw
            .checked_div(U256::from(10u128.pow((token0_decimals - token1_decimals) as u32)))
            .ok_or_else(|| CcxtError::ParseError("Decimal adjustment error".to_string()))?
    };

    // Convert to Decimal
    let price_str = adjusted_price.to_string();
    let mut price = Decimal::from_str(&price_str)
        .map_err(|_| CcxtError::ParseError(format!("Invalid price: {}", price_str)))?;

    // Scale down by 10^18 (Uniswap uses fixed-point math)
    price = price / Decimal::from(10u128.pow(18));

    // Invert if needed (when base is token1)
    if inverted {
        price = Decimal::ONE / price;
    }

    Ok(price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqrt_price_conversion() {
        // Known sqrtPriceX96 value for USDC/WETH
        // This is an example - actual values will vary
        let sqrt_price_x96: u128 = 1461446703485210103287273052203988822378723970342;

        // USDC has 6 decimals, WETH has 18 decimals
        let result = sqrt_price_x96_to_price(sqrt_price_x96, 6, 18, false);
        assert!(result.is_ok());

        let price = result.unwrap();
        assert!(price > Decimal::ZERO);
    }

    #[test]
    fn test_pool_info_parsing() {
        // This would require mocking the subgraph response
        // For now, just verify the struct can be created
        let pool_info = PoolInfo {
            address: Address::ZERO,
            token0_address: Address::ZERO,
            token1_address: Address::ZERO,
            token0_symbol: "WETH".to_string(),
            token1_symbol: "USDC".to_string(),
            token0_decimals: 18,
            token1_decimals: 6,
            fee_tier: FeeTier::Medium,
            liquidity: "1000000".to_string(),
            tvl_usd: Decimal::from(1000000),
        };

        assert_eq!(pool_info.token0_symbol, "WETH");
        assert_eq!(pool_info.fee_tier, FeeTier::Medium);
    }
}
