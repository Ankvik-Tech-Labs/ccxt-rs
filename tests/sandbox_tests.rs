//! Sandbox Private API Tests (Tier 4)
//!
//! Test private API methods against exchange sandbox/testnet environments.
//! All tests are marked #[ignore] and require credentials via env vars.
//!
//! Run with:
//!   BINANCE_SANDBOX_API_KEY=... BINANCE_SANDBOX_SECRET=... \
//!   cargo test --all-features -- --ignored sandbox --test-threads=1

mod validators;

/// Helper to get env var or skip test
fn env_or_skip(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// =============================================================================
// BINANCE SANDBOX TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_sandbox {
    use super::*;
    use ccxt::binance::Binance;
    use ccxt::prelude::*;
    use rust_decimal::Decimal;

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
    async fn sandbox_binance_fetch_balance() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BINANCE_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let balances = exchange.fetch_balance().await.unwrap();
        let errors = validators::validate_balances(&balances);
        assert!(
            errors.is_empty(),
            "Balance validation failed: {:?}",
            errors
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_binance_order_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BINANCE_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        // Create a limit buy order far below market
        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                Decimal::new(1, 4), // 0.0001 BTC
                Some(Decimal::new(10000, 0)), // $10,000 (far below market)
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(
            errors.is_empty(),
            "Order creation validation failed: {:?}",
            errors
        );
        assert_eq!(order.symbol, "BTC/USDT");
        assert_eq!(order.side, OrderSide::Buy);

        // Fetch open orders
        let open_orders = exchange
            .fetch_open_orders(Some("BTC/USDT"), None, None)
            .await
            .unwrap();
        assert!(
            open_orders.iter().any(|o| o.id == order.id),
            "Order {} should be in open orders",
            order.id
        );

        // Cancel the order
        let cancelled = exchange
            .cancel_order(&order.id, Some("BTC/USDT"))
            .await
            .unwrap();
        assert!(
            cancelled.status == OrderStatus::Canceled || cancelled.status == OrderStatus::Closed,
            "Order should be cancelled, got: {:?}",
            cancelled.status
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_binance_fetch_my_trades() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BINANCE_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let trades = exchange
            .fetch_my_trades(Some("BTC/USDT"), None, Some(5))
            .await
            .unwrap();
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade validation failed: {:?}",
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_binance_fetch_funding_rate() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BINANCE_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let fr = exchange.fetch_funding_rate("BTC/USDT:USDT").await.unwrap();
        let errors = validators::validate_funding_rate(&fr);
        assert!(
            errors.is_empty(),
            "Funding rate validation failed: {:?}",
            errors
        );
    }
}

// =============================================================================
// BYBIT SANDBOX TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_sandbox {
    use super::*;
    use ccxt::bybit::Bybit;
    use ccxt::prelude::*;
    use rust_decimal::Decimal;

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
    async fn sandbox_bybit_fetch_balance() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BYBIT_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let balances = exchange.fetch_balance().await.unwrap();
        let errors = validators::validate_balances(&balances);
        assert!(
            errors.is_empty(),
            "Balance validation failed: {:?}",
            errors
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_bybit_order_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BYBIT_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                Decimal::new(1, 4),
                Some(Decimal::new(10000, 0)),
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(
            errors.is_empty(),
            "Order creation validation failed: {:?}",
            errors
        );

        let open_orders = exchange
            .fetch_open_orders(Some("BTC/USDT"), None, None)
            .await
            .unwrap();
        assert!(
            open_orders.iter().any(|o| o.id == order.id),
            "Order should be in open orders"
        );

        let cancelled = exchange
            .cancel_order(&order.id, Some("BTC/USDT"))
            .await
            .unwrap();
        assert!(
            cancelled.status == OrderStatus::Canceled || cancelled.status == OrderStatus::Closed,
            "Order should be cancelled"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_bybit_fetch_my_trades() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BYBIT_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let trades = exchange
            .fetch_my_trades(Some("BTC/USDT"), None, Some(5))
            .await
            .unwrap();
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade validation failed: {:?}",
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_bybit_fetch_funding_rate() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: BYBIT_SANDBOX_API_KEY/SECRET not set");
                return;
            }
        };

        let fr = exchange.fetch_funding_rate("BTC/USDT:USDT").await.unwrap();
        let errors = validators::validate_funding_rate(&fr);
        assert!(
            errors.is_empty(),
            "Funding rate validation failed: {:?}",
            errors
        );
    }
}

// =============================================================================
// OKX SANDBOX TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_sandbox {
    use super::*;
    use ccxt::okx::Okx;
    use ccxt::prelude::*;
    use rust_decimal::Decimal;

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
    async fn sandbox_okx_fetch_balance() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: OKX_SANDBOX_API_KEY/SECRET/PASSPHRASE not set");
                return;
            }
        };

        let balances = exchange.fetch_balance().await.unwrap();
        let errors = validators::validate_balances(&balances);
        assert!(
            errors.is_empty(),
            "Balance validation failed: {:?}",
            errors
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_okx_order_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: OKX_SANDBOX_API_KEY/SECRET/PASSPHRASE not set");
                return;
            }
        };

        let order = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                Decimal::new(1, 4),
                Some(Decimal::new(10000, 0)),
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(
            errors.is_empty(),
            "Order creation validation failed: {:?}",
            errors
        );

        let open_orders = exchange
            .fetch_open_orders(Some("BTC/USDT"), None, None)
            .await
            .unwrap();
        assert!(
            open_orders.iter().any(|o| o.id == order.id),
            "Order should be in open orders"
        );

        let cancelled = exchange
            .cancel_order(&order.id, Some("BTC/USDT"))
            .await
            .unwrap();
        assert!(
            cancelled.status == OrderStatus::Canceled || cancelled.status == OrderStatus::Closed,
            "Order should be cancelled"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_okx_fetch_my_trades() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: OKX_SANDBOX_API_KEY/SECRET/PASSPHRASE not set");
                return;
            }
        };

        let trades = exchange
            .fetch_my_trades(Some("BTC/USDT"), None, Some(5))
            .await
            .unwrap();
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade validation failed: {:?}",
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_okx_fetch_funding_rate() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: OKX_SANDBOX_API_KEY/SECRET/PASSPHRASE not set");
                return;
            }
        };

        let fr = exchange.fetch_funding_rate("BTC/USDT:USDT").await.unwrap();
        let errors = validators::validate_funding_rate(&fr);
        assert!(
            errors.is_empty(),
            "Funding rate validation failed: {:?}",
            errors
        );
    }
}

// =============================================================================
// HYPERLIQUID SANDBOX TESTS
// =============================================================================

#[cfg(feature = "hyperliquid")]
mod hyperliquid_sandbox {
    use super::*;
    use ccxt::hyperliquid::Hyperliquid;
    use ccxt::prelude::*;
    use rust_decimal::Decimal;

    fn build_sandbox() -> Option<Hyperliquid> {
        let private_key = env_or_skip("HYPERLIQUID_PRIVATE_KEY")?;
        Some(
            Hyperliquid::builder()
                .private_key(private_key)
                .sandbox(true)
                .build()
                .expect("Failed to build Hyperliquid sandbox"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_hl_fetch_balance() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: HYPERLIQUID_PRIVATE_KEY not set");
                return;
            }
        };

        let balances = exchange.fetch_balance().await.unwrap();
        let errors = validators::validate_balances(&balances);
        assert!(
            errors.is_empty(),
            "Balance validation failed: {:?}",
            errors
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_hl_order_lifecycle() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: HYPERLIQUID_PRIVATE_KEY not set");
                return;
            }
        };

        // Create a limit buy order far below market
        let order = exchange
            .create_order(
                "BTC/USD:USDC",
                OrderType::Limit,
                OrderSide::Buy,
                Decimal::new(1, 4), // 0.0001 BTC
                Some(Decimal::new(10000, 0)), // $10,000 (far below market)
                None,
            )
            .await
            .unwrap();

        let errors = validators::validate_order(&order);
        assert!(
            errors.is_empty(),
            "Order creation validation failed: {:?}",
            errors
        );
        assert_eq!(order.symbol, "BTC/USD:USDC");
        assert_eq!(order.side, OrderSide::Buy);

        // Fetch open orders
        let open_orders = exchange
            .fetch_open_orders(Some("BTC/USD:USDC"), None, None)
            .await
            .unwrap();
        assert!(
            open_orders.iter().any(|o| o.id == order.id),
            "Order {} should be in open orders",
            order.id
        );

        // Cancel the order
        let cancelled = exchange
            .cancel_order(&order.id, Some("BTC/USD:USDC"))
            .await
            .unwrap();
        assert!(
            cancelled.status == OrderStatus::Canceled || cancelled.status == OrderStatus::Closed,
            "Order should be cancelled, got: {:?}",
            cancelled.status
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_hl_fetch_positions() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: HYPERLIQUID_PRIVATE_KEY not set");
                return;
            }
        };

        let positions = exchange
            .fetch_positions(Some(&["BTC/USD:USDC"]))
            .await
            .unwrap();
        for position in &positions {
            let errors = validators::validate_position(position);
            assert!(
                errors.is_empty(),
                "Position validation failed: {:?}",
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn sandbox_hl_fetch_my_trades() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => {
                eprintln!("SKIP: HYPERLIQUID_PRIVATE_KEY not set");
                return;
            }
        };

        let trades = exchange
            .fetch_my_trades(Some("BTC/USD:USDC"), None, Some(5))
            .await
            .unwrap();
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade validation failed: {:?}",
                errors
            );
        }
    }
}
