//! Uniswap V3 swap integration tests
//!
//! These tests require external connections and are marked `#[ignore]`.
//! Run with:
//!   cargo test --features uniswap -- --ignored uniswap_swap --test-threads=1
//!
//! Environment variables:
//!   ARBITRUM_RPC_URL  — Arbitrum RPC endpoint (default: public endpoint)
//!   SWAP_PRIVATE_KEY  — Private key hex for executing swaps (optional)

#[cfg(feature = "uniswap")]
mod uniswap_swap {
    use ccxt::uniswap::swap::{
        apply_slippage, decimal_to_u256_with_decimals, parse_slippage_bps,
        u256_to_decimal_with_decimals,
    };
    use alloy::primitives::U256;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // ─── Unit tests (no network) ────────────────────────────────────────────

    #[test]
    fn test_slippage_math_round_trip() {
        // 1000 tokens at 1% slippage → 990 minimum
        let amount = U256::from(1000u64);
        let min_out = apply_slippage(amount, 100); // 100 bps = 1%
        assert_eq!(min_out, U256::from(990u64));
    }

    #[test]
    fn test_slippage_zero_bps() {
        let amount = U256::from(1000u64);
        let min_out = apply_slippage(amount, 0);
        assert_eq!(min_out, amount, "0 slippage should return full amount");
    }

    #[test]
    fn test_amount_conversion_usdc() {
        // 100 USDC (6 decimals)
        let d = Decimal::from(100);
        let u = decimal_to_u256_with_decimals(d, 6);
        assert_eq!(u, U256::from(100_000_000u64));

        let back = u256_to_decimal_with_decimals(u, 6);
        assert_eq!(back, Decimal::from(100));
    }

    #[test]
    fn test_amount_conversion_weth() {
        // 0.5 WETH (18 decimals)
        let d = Decimal::from_str("0.5").unwrap();
        let u = decimal_to_u256_with_decimals(d, 18);
        assert_eq!(u, U256::from_str("500000000000000000").unwrap());

        let back = u256_to_decimal_with_decimals(u, 18);
        assert_eq!(back, d);
    }

    #[test]
    fn test_parse_slippage_bps_default() {
        assert_eq!(parse_slippage_bps(None), 50, "Default should be 50bps (0.5%)");
    }

    #[test]
    fn test_parse_slippage_custom() {
        use serde_json::json;
        let mut params = std::collections::HashMap::new();
        params.insert("slippage".to_string(), json!(2.0)); // 2% → 200bps
        assert_eq!(parse_slippage_bps(Some(&params)), 200);
    }

    // ─── Integration tests (require live RPC) ───────────────────────────────

    fn arbitrum_rpc() -> String {
        std::env::var("ARBITRUM_RPC_URL")
            .unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string())
    }

    #[tokio::test]
    #[ignore] // Requires live Arbitrum RPC
    async fn test_quote_weth_usdc_arbitrum() {
        use alloy::primitives::Address;
        use ccxt::uniswap::swap::quote_exact_input_single;

        // QuoterV2 on Arbitrum
        let quoter: Address = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e"
            .parse()
            .unwrap();
        let weth: Address = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"
            .parse()
            .unwrap();
        let usdc: Address = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
            .parse()
            .unwrap();

        // Quote 0.01 WETH → USDC (0.05% fee pool)
        let amount_in = U256::from(10_000_000_000_000_000u64); // 0.01 WETH
        let quote = quote_exact_input_single(&arbitrum_rpc(), quoter, weth, usdc, 500, amount_in)
            .await
            .expect("Quote should succeed");

        let usdc_out = u256_to_decimal_with_decimals(quote, 6);
        println!("0.01 WETH → {} USDC", usdc_out);

        // Sanity check: 0.01 WETH should be worth > $1 and < $10_000
        assert!(
            usdc_out > Decimal::from(1),
            "Expected > 1 USDC for 0.01 WETH, got {}",
            usdc_out
        );
        assert!(
            usdc_out < Decimal::from(10_000),
            "Expected < $10k for 0.01 WETH, got {}",
            usdc_out
        );
    }

    #[tokio::test]
    #[ignore] // Requires SWAP_PRIVATE_KEY env var with funded Arbitrum account
    async fn test_full_swap_weth_usdc() {
        use ccxt::uniswap::UniswapV3;
        use ccxt::base::exchange::Exchange;

        let private_key = match std::env::var("SWAP_PRIVATE_KEY") {
            Ok(k) => k,
            Err(_) => {
                println!("Skipping: SWAP_PRIVATE_KEY not set");
                return;
            }
        };

        let api_key = std::env::var("THE_GRAPH_API_KEY")
            .unwrap_or_else(|_| "test".to_string());

        let uniswap = UniswapV3::builder()
            .chain("arbitrum")
            .subgraph_api_key(api_key)
            .rpc_url(arbitrum_rpc())
            .private_key(&private_key)
            .build()
            .await
            .expect("Builder should succeed");

        // Create a small market sell order: sell 0.001 WETH for USDC
        let order: ccxt::types::Order = uniswap
            .create_order(
                "WETH/USDC:V3:500",
                ccxt::types::OrderType::Market,
                ccxt::types::OrderSide::Sell,
                Decimal::from_str("0.001").unwrap(),
                None,
                None,
            )
            .await
            .expect("Swap should succeed");

        println!("Swap order: {:?}", order);
        assert_eq!(order.status, ccxt::types::OrderStatus::Closed);
        assert!(!order.id.is_empty(), "Should have a transaction hash");
        assert!(order.cost.unwrap_or_default() > Decimal::ZERO, "Cost should be positive");
    }
}
