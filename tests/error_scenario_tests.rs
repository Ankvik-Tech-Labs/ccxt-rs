//! Error Scenario Tests (Tier 5)
//!
//! Test that exchanges return correct CcxtError variants for known error conditions.
//! All tests are #[ignore] and require sandbox credentials.
//!
//! Run with:
//!   BINANCE_SANDBOX_API_KEY=... BINANCE_SANDBOX_SECRET=... \
//!   cargo test --all-features -- --ignored error_scenario --test-threads=1

fn env_or_skip(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// =============================================================================
// BINANCE ERROR SCENARIOS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_errors {
    use super::*;
    use ccxt::binance::Binance;
    use ccxt::prelude::*;
    use rust_decimal_macros::dec;

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
    async fn error_scenario_binance_bad_symbol() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange.fetch_ticker("FAKECOIN/USDT").await;
        assert!(
            result.is_err(),
            "Fetching ticker for invalid symbol should fail"
        );

        if let Err(e) = result {
            assert!(
                matches!(e, CcxtError::BadSymbol(_) | CcxtError::BadRequest(_) | CcxtError::ExchangeError(_)),
                "Expected BadSymbol/BadRequest/ExchangeError, got: {:?}",
                e
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_binance_cancel_nonexistent() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange
            .cancel_order("999999999", Some("BTC/USDT"))
            .await;

        assert!(result.is_err(), "Cancelling non-existent order should fail");

        if let Err(e) = result {
            assert!(
                matches!(e, CcxtError::OrderNotFound(_) | CcxtError::InvalidOrder(_) | CcxtError::BadRequest(_)),
                "Expected OrderNotFound, got: {:?}",
                e
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_binance_bad_credentials() {
        let exchange = Binance::builder()
            .api_key("bad_api_key_that_doesnt_exist")
            .secret("bad_secret_that_doesnt_exist")
            .sandbox(true)
            .build()
            .unwrap();

        let result = exchange.fetch_balance().await;
        assert!(result.is_err(), "Bad credentials should fail");

        if let Err(e) = &result {
            assert!(
                e.is_auth_error() || matches!(e, CcxtError::BadRequest(_)),
                "Expected auth error, got: {:?}",
                e
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_binance_edit_not_supported() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange
            .edit_order("fake_id", "BTC/USDT", OrderType::Limit, OrderSide::Buy, Some(dec!(1)), Some(dec!(10000)))
            .await;

        assert!(
            matches!(result, Err(CcxtError::NotSupported(_))),
            "Binance edit_order should return NotSupported, got: {:?}",
            result
        );
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_binance_insufficient_funds() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        // Try to buy an absurdly large amount
        let result = exchange
            .create_order(
                "BTC/USDT",
                OrderType::Limit,
                OrderSide::Buy,
                dec!(999999),
                Some(dec!(100000)),
                None,
            )
            .await;

        // This should fail with InsufficientFunds or InvalidOrder
        if let Err(e) = result {
            assert!(
                matches!(e,
                    CcxtError::InsufficientFunds(_)
                    | CcxtError::InvalidOrder(_)
                    | CcxtError::BadRequest(_)
                ),
                "Expected InsufficientFunds or InvalidOrder, got: {:?}",
                e
            );
        }
        // If it somehow succeeds, cancel it
        else if let Ok(order) = result {
            let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_binance_auth_error_helpers() {
        // Test the is_auth_error() helper
        let auth_err = CcxtError::AuthenticationError("test".to_string());
        assert!(auth_err.is_auth_error());

        let perm_err = CcxtError::PermissionDenied("test".to_string());
        assert!(perm_err.is_auth_error());

        let nonce_err = CcxtError::InvalidNonce("test".to_string());
        assert!(nonce_err.is_auth_error());

        // Non-auth errors
        let order_err = CcxtError::OrderNotFound("test".to_string());
        assert!(!order_err.is_auth_error());

        let network_err = CcxtError::NetworkError("test".to_string());
        assert!(!network_err.is_auth_error());
        assert!(network_err.is_retryable());
    }
}

// =============================================================================
// BYBIT ERROR SCENARIOS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_errors {
    use super::*;
    use ccxt::bybit::Bybit;
    use ccxt::prelude::*;

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
    async fn error_scenario_bybit_cancel_nonexistent() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange
            .cancel_order("nonexistent-order-id-12345", Some("BTC/USDT"))
            .await;

        assert!(result.is_err(), "Cancelling non-existent order should fail");
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_bybit_bad_credentials() {
        let exchange = Bybit::builder()
            .api_key("invalid_key")
            .secret("invalid_secret")
            .sandbox(true)
            .build()
            .unwrap();

        let result = exchange.fetch_balance().await;
        assert!(result.is_err(), "Bad credentials should fail");
    }
}

// =============================================================================
// OKX ERROR SCENARIOS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_errors {
    use super::*;
    use ccxt::okx::Okx;
    use ccxt::prelude::*;

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
    async fn error_scenario_okx_cancel_nonexistent() {
        let exchange = match build_sandbox() {
            Some(e) => e,
            None => { eprintln!("SKIP: credentials not set"); return; }
        };

        let result = exchange
            .cancel_order("999999999", Some("BTC/USDT"))
            .await;

        assert!(result.is_err(), "Cancelling non-existent order should fail");
    }

    #[tokio::test]
    #[ignore]
    async fn error_scenario_okx_bad_credentials() {
        let exchange = Okx::builder()
            .api_key("invalid_key")
            .secret("invalid_secret")
            .passphrase("invalid_pass")
            .sandbox(true)
            .build()
            .unwrap();

        let result = exchange.fetch_balance().await;
        assert!(result.is_err(), "Bad credentials should fail");
    }
}
