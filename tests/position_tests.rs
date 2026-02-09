//! Position Tests (Tier 5)
//!
//! Test derivatives/futures position management across exchanges.
//! All tests are #[ignore] and require sandbox credentials.
//!
//! Run with:
//!   BINANCE_SANDBOX_API_KEY=... BINANCE_SANDBOX_SECRET=... \
//!   cargo test --all-features -- --ignored position_ --test-threads=1

mod validators;

fn env_or_skip(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// =============================================================================
// BINANCE POSITION TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_positions {
    use super::*;
    use ccxt::binance::Binance;
    use ccxt::prelude::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn build_sandbox() -> Option<Binance> {
        let api_key = env_or_skip("BINANCE_SANDBOX_API_KEY")?;
        let secret = env_or_skip("BINANCE_SANDBOX_SECRET")?;
        Some(
            Binance::builder()
                .api_key(api_key)
                .secret(secret)
                .sandbox(true)
                .build()
                .expect("Failed to build Binance sandbox"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn position_binance_open_and_close() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "BTC/USDT";

        // Set margin mode (may already be set)
        match exchange.set_margin_mode(MarginMode::Cross, symbol).await {
            Ok(()) | Err(CcxtError::MarginModeAlreadySet(_)) => {}
            Err(e) => { eprintln!("SKIP: Cannot set margin mode: {}", e); return; }
        }

        // Set leverage
        let _ = exchange.set_leverage(10, symbol).await;

        // Open long
        let open_order = match exchange
            .create_order(symbol, OrderType::Market, OrderSide::Buy, dec!(0.001), None, None)
            .await
        {
            Ok(o) => o,
            Err(CcxtError::InsufficientFunds(_)) => {
                eprintln!("SKIP: Insufficient funds");
                return;
            }
            Err(e) => panic!("Open order failed: {}", e),
        };

        assert!(!open_order.id.is_empty());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Fetch positions
        let positions = exchange.fetch_positions(None).await.unwrap();
        let pos = positions.iter().find(|p| p.symbol.contains("BTC"));

        if let Some(pos) = pos {
            assert!(pos.contracts > Decimal::ZERO, "contracts should be > 0");
            assert!(pos.entry_price.is_some(), "entry_price should be set");

            if let Some(entry) = pos.entry_price {
                assert!(entry > Decimal::ZERO, "entry_price should be > 0");
            }

            if let Some(leverage) = pos.leverage {
                assert!(leverage > Decimal::ZERO, "leverage should be > 0");
            }

            let errors = validators::validate_position(pos);
            assert!(errors.is_empty(), "Position validation: {:?}", errors);
        } else {
            eprintln!("Warning: BTC position not found after market buy");
        }

        // Close position
        let mut close_params = HashMap::new();
        close_params.insert(
            "reduceOnly".to_string(),
            serde_json::Value::String("true".to_string()),
        );

        match exchange
            .create_order(symbol, OrderType::Market, OrderSide::Sell, dec!(0.001), None, Some(&close_params))
            .await
        {
            Ok(_) => {}
            Err(e) => eprintln!("Close position error: {}", e),
        }

        // Verify closed
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let final_positions = exchange.fetch_positions(None).await.unwrap();
        let btc_pos = final_positions.iter().find(|p| p.symbol.contains("BTC"));
        assert!(btc_pos.is_none(), "BTC position should be closed");
    }

    #[tokio::test]
    #[ignore]
    async fn position_binance_leverage_changes() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "BTC/USDT";

        // Set leverage 10x
        exchange.set_leverage(10, symbol).await.unwrap();

        // Change to 20x
        exchange.set_leverage(20, symbol).await.unwrap();

        // Change back to 5x
        exchange.set_leverage(5, symbol).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn position_binance_margin_mode_changes() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "ETH/USDT"; // Use ETH to avoid conflict with other tests

        // Set to cross
        match exchange.set_margin_mode(MarginMode::Cross, symbol).await {
            Ok(()) => {}
            Err(CcxtError::MarginModeAlreadySet(_)) => {}
            Err(e) => { eprintln!("SKIP: {}", e); return; }
        }

        // Try to set to isolated
        match exchange.set_margin_mode(MarginMode::Isolated, symbol).await {
            Ok(()) => {
                // Set back to cross
                match exchange.set_margin_mode(MarginMode::Cross, symbol).await {
                    Ok(()) => {}
                    Err(CcxtError::MarginModeAlreadySet(_)) => {}
                    Err(e) => eprintln!("Warning: {}", e),
                }
            }
            Err(CcxtError::MarginModeAlreadySet(_)) => {
                // Already isolated, that's fine
            }
            Err(e) => eprintln!("Could not change to isolated: {}", e),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn position_binance_margin_mode_already_set() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "BTC/USDT";

        // Set cross mode
        match exchange.set_margin_mode(MarginMode::Cross, symbol).await {
            Ok(()) => {}
            Err(CcxtError::MarginModeAlreadySet(_)) => {}
            Err(e) => { eprintln!("SKIP: {}", e); return; }
        }

        // Set cross mode again — should get MarginModeAlreadySet
        let result = exchange.set_margin_mode(MarginMode::Cross, symbol).await;
        assert!(
            matches!(result, Ok(()) | Err(CcxtError::MarginModeAlreadySet(_))),
            "Expected Ok or MarginModeAlreadySet, got: {:?}",
            result
        );
    }
}

// =============================================================================
// BYBIT POSITION TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_positions {
    use super::*;
    use ccxt::bybit::Bybit;
    use ccxt::prelude::*;
    use rust_decimal_macros::dec;

    fn build_sandbox() -> Option<Bybit> {
        let api_key = env_or_skip("BYBIT_SANDBOX_API_KEY")?;
        let secret = env_or_skip("BYBIT_SANDBOX_SECRET")?;
        Some(
            Bybit::builder()
                .api_key(api_key)
                .secret(secret)
                .sandbox(true)
                .build()
                .expect("Failed to build Bybit sandbox"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn position_bybit_leverage_changes() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "BTC/USDT";
        exchange.set_leverage(10, symbol).await.unwrap();
        exchange.set_leverage(20, symbol).await.unwrap();
        exchange.set_leverage(5, symbol).await.unwrap();
    }
}

// =============================================================================
// OKX POSITION TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_positions {
    use super::*;
    use ccxt::okx::Okx;
    use ccxt::prelude::*;
    use rust_decimal_macros::dec;

    fn build_sandbox() -> Option<Okx> {
        let api_key = env_or_skip("OKX_SANDBOX_API_KEY")?;
        let secret = env_or_skip("OKX_SANDBOX_SECRET")?;
        let passphrase = env_or_skip("OKX_SANDBOX_PASSPHRASE")?;
        Some(
            Okx::builder()
                .api_key(api_key)
                .secret(secret)
                .passphrase(passphrase)
                .sandbox(true)
                .build()
                .expect("Failed to build OKX sandbox"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn position_okx_leverage_changes() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let symbol = "BTC/USDT";
        exchange.set_leverage(10, symbol).await.unwrap();
        exchange.set_leverage(20, symbol).await.unwrap();
        exchange.set_leverage(5, symbol).await.unwrap();
    }
}
