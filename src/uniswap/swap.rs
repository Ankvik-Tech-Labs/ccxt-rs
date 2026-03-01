//! Uniswap V3 swap execution helpers
//!
//! Provides quote and swap execution using:
//! - QuoterV2: off-chain quote via `quoteExactInputSingle` (view call, no gas)
//! - SwapRouter02: on-chain execution via `exactInputSingle`

use crate::base::errors::{CcxtError, Result};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, Uint, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};
use rust_decimal::Decimal;
use std::str::FromStr;

// QuoterV2 ABI — view function, no gas required
sol! {
    #[sol(rpc)]
    interface IQuoterV2 {
        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }

        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
            external
            returns (
                uint256 amountOut,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );
    }
}

// SwapRouter02 ABI — state-changing, requires wallet
sol! {
    #[sol(rpc)]
    interface ISwapRouter02 {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params)
            external
            payable
            returns (uint256 amountOut);
    }
}

// ERC20 approve — needed when allowance < amountIn
sol! {
    #[sol(rpc)]
    interface IERC20Approve {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// Get a price quote for an exact input swap.
///
/// Calls QuoterV2's `quoteExactInputSingle` (view-only, no gas cost).
///
/// # Arguments
/// * `rpc_url` — RPC endpoint
/// * `quoter_address` — QuoterV2 contract address
/// * `token_in` — Input token address
/// * `token_out` — Output token address
/// * `fee` — Pool fee tier (100 / 500 / 3000 / 10000)
/// * `amount_in` — Raw token amount in (with decimals applied)
pub async fn quote_exact_input_single(
    rpc_url: &str,
    quoter_address: Address,
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
) -> Result<U256> {
    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse().map_err(|e| {
            CcxtError::ConfigError(format!("Invalid RPC URL '{}': {}", rpc_url, e))
        })?);

    let quoter = IQuoterV2::new(quoter_address, &provider);

    let params = IQuoterV2::QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: amount_in,
        fee: Uint::<24, 1>::from(fee),
        sqrtPriceLimitX96: Uint::<160, 3>::ZERO,
    };

    let result = quoter
        .quoteExactInputSingle(params)
        .call()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("QuoterV2 call failed: {}", e)))?;

    Ok(result.amountOut)
}

/// Apply slippage to a quoted amount to get the minimum acceptable output.
///
/// `slippage_bps` — basis points, e.g. 50 = 0.5%
pub fn apply_slippage(amount: U256, slippage_bps: u64) -> U256 {
    // min_out = amount * (10000 - slippage_bps) / 10000
    let numerator = amount * U256::from(10_000u64 - slippage_bps);
    numerator / U256::from(10_000u64)
}

/// Check allowance and approve if needed.
///
/// Returns the approval tx hash if an approval was submitted, `None` if already approved.
pub async fn ensure_allowance(
    rpc_url: &str,
    signer: &PrivateKeySigner,
    token: Address,
    spender: Address,
    amount_in: U256,
) -> Result<Option<B256>> {
    let wallet = EthereumWallet::from(signer.clone());
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().map_err(|e| {
            CcxtError::ConfigError(format!("Invalid RPC URL: {}", e))
        })?);

    let erc20 = IERC20Approve::new(token, &provider);

    // Check current allowance
    let current = erc20
        .allowance(signer.address(), spender)
        .call()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("allowance() call failed: {}", e)))?
        ._0;

    if current >= amount_in {
        return Ok(None);
    }

    tracing::info!(
        "Approving {} for spender {} (current={}, needed={})",
        token,
        spender,
        current,
        amount_in
    );

    // Approve max uint256 to avoid repeated approvals
    let pending = erc20
        .approve(spender, U256::MAX)
        .send()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("approve() send failed: {}", e)))?;

    let receipt = pending
        .get_receipt()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("approve() receipt failed: {}", e)))?;

    tracing::info!("Approval confirmed in tx {}", receipt.transaction_hash);

    Ok(Some(receipt.transaction_hash))
}

/// Execute a Uniswap V3 exact-input-single swap via SwapRouter02.
///
/// Returns the transaction hash.
///
/// # Arguments
/// * `rpc_url` — RPC endpoint
/// * `router_address` — SwapRouter02 address
/// * `signer` — Private key signer
/// * `token_in` — Input token address
/// * `token_out` — Output token address
/// * `fee` — Pool fee tier (100 / 500 / 3000 / 10000)
/// * `amount_in` — Raw amount in
/// * `amount_out_min` — Minimum acceptable output (post-slippage)
/// * `recipient` — Address to receive the output tokens
pub async fn execute_swap(
    rpc_url: &str,
    router_address: Address,
    signer: &PrivateKeySigner,
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
    amount_out_min: U256,
    recipient: Address,
) -> Result<B256> {
    let wallet = EthereumWallet::from(signer.clone());
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().map_err(|e| {
            CcxtError::ConfigError(format!("Invalid RPC URL: {}", e))
        })?);

    let router = ISwapRouter02::new(router_address, &provider);

    let params = ISwapRouter02::ExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        fee: Uint::<24, 1>::from(fee),
        recipient,
        amountIn: amount_in,
        amountOutMinimum: amount_out_min,
        sqrtPriceLimitX96: Uint::<160, 3>::ZERO,
    };

    tracing::info!(
        "Executing swap: {} {} → {} (fee={}bps, min_out={})",
        amount_in,
        token_in,
        token_out,
        fee,
        amount_out_min
    );

    let pending = router
        .exactInputSingle(params)
        .send()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("exactInputSingle send failed: {}", e)))?;

    let receipt = pending
        .get_receipt()
        .await
        .map_err(|e| CcxtError::AlloyError(format!("swap receipt failed: {}", e)))?;

    tracing::info!("Swap confirmed in tx {}", receipt.transaction_hash);

    Ok(receipt.transaction_hash)
}

/// Convert a `Decimal` amount to `U256` with the given token decimals.
pub fn decimal_to_u256_with_decimals(amount: Decimal, decimals: u8) -> U256 {
    let multiplier = Decimal::from(10u64.pow(decimals as u32));
    let scaled = (amount * multiplier).trunc();
    let s = scaled.to_string();
    U256::from_str_radix(s.trim_start_matches('-'), 10).unwrap_or(U256::ZERO)
}

/// Convert a `U256` raw amount to `Decimal` with the given token decimals.
pub fn u256_to_decimal_with_decimals(amount: U256, decimals: u8) -> Decimal {
    let s = amount.to_string();
    let raw = Decimal::from_str(&s).unwrap_or(Decimal::ZERO);
    raw / Decimal::from(10u64.pow(decimals as u32))
}

/// Parse slippage from params, defaulting to 50 bps (0.5%).
///
/// Accepts `params["slippage"]` as a percentage string (e.g., "0.5") or bps int (e.g., 50).
pub fn parse_slippage_bps(params: Option<&crate::base::exchange::Params>) -> u64 {
    let default = 50u64; // 0.5%
    if let Some(p) = params {
        if let Some(v) = p.get("slippage") {
            if let Some(f) = v.as_f64() {
                // Interpret as percentage (0.5 → 50 bps)
                return (f * 100.0) as u64;
            }
            if let Some(i) = v.as_u64() {
                return i;
            }
        }
    }
    default
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_decimal_to_u256_with_decimals() {
        // 1.5 USDC (6 decimals) → 1_500_000
        let d = Decimal::from_str("1.5").unwrap();
        let u = decimal_to_u256_with_decimals(d, 6);
        assert_eq!(u, U256::from(1_500_000u64));

        // 1 ETH (18 decimals)
        let d = Decimal::from(1);
        let u = decimal_to_u256_with_decimals(d, 18);
        assert_eq!(u, U256::from_str("1000000000000000000").unwrap());
    }

    #[test]
    fn test_u256_to_decimal_with_decimals() {
        // 1_000_000 → 1.0 USDC
        let u = U256::from(1_000_000u64);
        let d = u256_to_decimal_with_decimals(u, 6);
        assert_eq!(d, Decimal::from(1));
    }

    #[test]
    fn test_apply_slippage_50bps() {
        // 1000 with 0.5% slippage → 995
        let amount = U256::from(1000u64);
        let min_out = apply_slippage(amount, 50);
        assert_eq!(min_out, U256::from(995u64));
    }

    #[test]
    fn test_apply_slippage_100bps() {
        // 1000 with 1% slippage → 990
        let amount = U256::from(1000u64);
        let min_out = apply_slippage(amount, 100);
        assert_eq!(min_out, U256::from(990u64));
    }

    #[test]
    fn test_parse_slippage_bps_default() {
        assert_eq!(parse_slippage_bps(None), 50);
    }

    #[test]
    fn test_parse_slippage_bps_from_params() {
        use serde_json::json;
        let mut params = std::collections::HashMap::new();
        params.insert("slippage".to_string(), json!(1.0)); // 1% → 100bps
        assert_eq!(parse_slippage_bps(Some(&params)), 100);
    }

    #[tokio::test]
    #[ignore] // Requires live Arbitrum RPC
    async fn test_quote_weth_usdc_arbitrum() {
        let rpc_url = std::env::var("ARBITRUM_RPC_URL")
            .unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string());

        // QuoterV2 on Arbitrum
        let quoter: Address = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e"
            .parse()
            .unwrap();
        // WETH on Arbitrum
        let weth: Address = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"
            .parse()
            .unwrap();
        // USDC on Arbitrum
        let usdc: Address = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
            .parse()
            .unwrap();

        // Quote 0.01 WETH → USDC (0.05% fee pool)
        let amount_in = U256::from(10_000_000_000_000_000u64); // 0.01 ETH in wei
        let quote = quote_exact_input_single(&rpc_url, quoter, weth, usdc, 500, amount_in)
            .await
            .unwrap();

        // Should be > 0 and roughly $15-50 worth of USDC
        let usdc_out = u256_to_decimal_with_decimals(quote, 6);
        println!("0.01 WETH → {} USDC", usdc_out);
        assert!(usdc_out > Decimal::from(1), "Expected more than 1 USDC for 0.01 WETH");
    }
}
