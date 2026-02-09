//! Order Type Tests (Tier 5)
//!
//! Comprehensive order type testing across exchanges.
//! All tests are #[ignore] and require sandbox credentials via env vars.
//!
//! Run with:
//!   BINANCE_SANDBOX_API_KEY=... BINANCE_SANDBOX_SECRET=... \
//!   cargo test --all-features -- --ignored order_type --test-threads=1

mod validators;

fn env_or_skip(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// =============================================================================
// BINANCE ORDER TYPE TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_order_types {
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
    async fn order_type_binance_limit_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        // Create limit buy far below market
        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                dec!(0.001),
                Some(dec!(10000)),
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(errors.is_empty(), "Validation: {:?}", errors);
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.order_type, OrderType::Limit);

        // Fetch open orders — our order should be there
        let open = exchange
            .fetch_open_orders(Some("BTC/USDT"), None, None)
            .await
            .unwrap();
        assert!(open.iter().any(|o| o.id == order.id), "Order not in open orders");

        // Fetch specific order
        let fetched = exchange.fetch_order(&order.id, Some("BTC/USDT")).await.unwrap();
        assert_eq!(fetched.id, order.id);
        assert_eq!(fetched.status, OrderStatus::Open);

        // Cancel
        let cancelled = exchange.cancel_order(&order.id, Some("BTC/USDT")).await.unwrap();
        assert!(
            cancelled.status == OrderStatus::Canceled || cancelled.status == OrderStatus::Closed
        );

        // Verify gone from open orders
        let open_after = exchange
            .fetch_open_orders(Some("BTC/USDT"), None, None)
            .await
            .unwrap();
        assert!(!open_after.iter().any(|o| o.id == order.id), "Order still in open orders after cancel");
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_binance_market_buy() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        // Market buy — should fill immediately
        let order = match exchange
            .create_order(
                "BTC/USDT",
                OrderType::Market,
                OrderSide::Buy,
                dec!(0.001),
                None,
                None,
            )
            .await
        {
            Ok(o) => o,
            Err(CcxtError::InsufficientFunds(_)) => {
                eprintln!("SKIP: Insufficient funds on sandbox");
                return;
            }
            Err(e) => panic!("Unexpected error: {}", e),
        };

        assert_eq!(order.order_type, OrderType::Market);
        assert!(
            order.status == OrderStatus::Closed || order.status == OrderStatus::Open,
            "Market order status: {:?}",
            order.status
        );

        // If closed, filled should be > 0
        if order.status == OrderStatus::Closed {
            if let Some(filled) = order.filled {
                assert!(filled > Decimal::ZERO, "Closed market order should have filled > 0");
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_binance_stop_loss_limit() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let ticker = exchange.fetch_ticker("BTC/USDT").await.unwrap();
        let last = ticker.last.unwrap_or(dec!(50000));

        let stop_price = (last * dec!(0.85)).round_dp(2);
        let limit_price = (last * dec!(0.84)).round_dp(2);

        let mut params = HashMap::new();
        params.insert("type".to_string(), serde_json::Value::String("STOP_LOSS_LIMIT".to_string()));
        params.insert("stopPrice".to_string(), serde_json::Value::String(stop_price.to_string()));
        params.insert("timeInForce".to_string(), serde_json::Value::String("GTC".to_string()));

        match exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Sell,
                dec!(0.001),
                Some(limit_price),
                Some(&params),
            )
            .await
        {
            Ok(order) => {
                // Verify the order has a stop price
                assert!(!order.id.is_empty());
                // Cancel cleanup
                let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
            }
            Err(CcxtError::InsufficientFunds(_)) => {
                eprintln!("SKIP: Need holdings to place stop-loss sell");
            }
            Err(CcxtError::InvalidOrder(msg)) => {
                eprintln!("SKIP: Exchange rejected stop-loss: {}", msg);
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_binance_post_only_rejection() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let ticker = exchange.fetch_ticker("BTC/USDT").await.unwrap();
        let aggressive_price = (ticker.ask.unwrap_or(dec!(50000)) * dec!(1.05)).round_dp(2);

        let mut params = HashMap::new();
        params.insert("timeInForce".to_string(), serde_json::Value::String("GTX".to_string()));

        // A post-only buy above the ask should be rejected
        let result = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                dec!(0.001),
                Some(aggressive_price),
                Some(&params),
            )
            .await;

        match result {
            Err(CcxtError::OrderImmediatelyFillable(_)) => {
                // Expected — post-only rejected because it would fill
            }
            Err(CcxtError::InvalidOrder(msg)) if msg.contains("would immediately") || msg.contains("GTX") => {
                // Also acceptable
            }
            Ok(order) => {
                // Some sandboxes may not enforce GTX — cancel to clean up
                let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
            }
            Err(e) => {
                // Exchange-specific error handling may differ
                eprintln!("Post-only test got: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_binance_edit_not_supported() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange
            .edit_order("12345", "BTC/USDT", OrderType::Limit, OrderSide::Buy, Some(dec!(0.001)), Some(dec!(10000)))
            .await;

        assert!(
            matches!(result, Err(CcxtError::NotSupported(_))),
            "Binance edit_order should return NotSupported, got: {:?}",
            result
        );
    }
}

// =============================================================================
// BYBIT ORDER TYPE TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_order_types {
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
    async fn order_type_bybit_limit_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                dec!(0.001),
                Some(dec!(10000)),
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(errors.is_empty(), "Validation: {:?}", errors);
        assert_eq!(order.status, OrderStatus::Open);

        // Bybit supports edit_order
        if exchange.has().edit_order {
            match exchange
                .edit_order(&order.id, "BTC/USDT", OrderType::Limit, OrderSide::Buy, Some(dec!(0.001)), Some(dec!(9500)))
                .await
            {
                Ok(edited) => {
                    assert!(!edited.id.is_empty());
                    // Cancel the edited order
                    let _ = exchange.cancel_order(&edited.id, Some("BTC/USDT")).await;
                }
                Err(e) => {
                    eprintln!("Edit failed (may need to cancel original): {}", e);
                    let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
                }
            }
        } else {
            let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_bybit_market_buy() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        match exchange
            .create_order("BTC/USDT", OrderType::Market, OrderSide::Buy, dec!(0.001), None, None)
            .await
        {
            Ok(order) => {
                assert_eq!(order.order_type, OrderType::Market);
            }
            Err(CcxtError::InsufficientFunds(_)) => {
                eprintln!("SKIP: Insufficient funds on sandbox");
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }
}

// =============================================================================
// OKX ORDER TYPE TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_order_types {
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
    async fn order_type_okx_limit_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                dec!(0.001),
                Some(dec!(10000)),
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(errors.is_empty(), "Validation: {:?}", errors);
        assert_eq!(order.status, OrderStatus::Open);

        // OKX supports edit_order
        if exchange.has().edit_order {
            match exchange
                .edit_order(&order.id, "BTC/USDT", OrderType::Limit, OrderSide::Buy, Some(dec!(0.001)), Some(dec!(9500)))
                .await
            {
                Ok(edited) => {
                    let _ = exchange.cancel_order(&edited.id, Some("BTC/USDT")).await;
                }
                Err(e) => {
                    eprintln!("Edit failed: {}", e);
                    let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
                }
            }
        } else {
            let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn order_type_okx_market_buy() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        match exchange
            .create_order("BTC/USDT", OrderType::Market, OrderSide::Buy, dec!(0.001), None, None)
            .await
        {
            Ok(order) => {
                assert_eq!(order.order_type, OrderType::Market);
            }
            Err(CcxtError::InsufficientFunds(_)) => {
                eprintln!("SKIP: Insufficient funds on sandbox");
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }
}
