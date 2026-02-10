//! WebSocket Integration Tests
//!
//! All tests are #[ignore] and require live network connections.
//! Public tests connect to real exchange WebSocket endpoints.
//! Private tests require sandbox credentials via environment variables.
//!
//! Run public tests:
//!   cargo test --all-features -- --ignored ws_ --test-threads=1
//!
//! Run private tests (example for Binance):
//!   BINANCE_SANDBOX_API_KEY=... BINANCE_SANDBOX_SECRET=... \
//!     cargo test --all-features -- --ignored ws_binance_private --test-threads=1

/// Helper: read an env var, return None if missing (test will skip).
fn env_or_skip(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// =============================================================================
// BINANCE WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_ws {
    use super::env_or_skip;
    use ccxt::base::ws::{ExchangeWs, WsConfig};
    use ccxt::binance::ws::BinanceWs;
    use std::time::Duration;

    #[tokio::test]
    #[ignore]
    async fn ws_binance_ticker_stream() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        let mut stream = ws.watch_ticker("BTC/USDT").await.unwrap();

        // Should receive at least one ticker within 10 seconds
        let ticker = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for ticker")
            .expect("Stream ended unexpectedly");

        assert_eq!(ticker.symbol, "BTC/USDT");
        assert!(ticker.last.is_some(), "Ticker should have a last price");
        assert!(
            ticker.last.unwrap() > rust_decimal::Decimal::ZERO,
            "Last price should be positive"
        );

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_orderbook_stream() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT", Some(20)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT");
        // Note: depth updates may not have full book until snapshot is applied

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_orderbook_depth_sorted() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT", Some(20)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT");

        // Verify bids sorted descending (highest first)
        for window in ob.bids.windows(2) {
            assert!(
                window[0].0 >= window[1].0,
                "Bids not sorted descending: {} < {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify asks sorted ascending (lowest first)
        for window in ob.asks.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "Asks not sorted ascending: {} > {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify no zero amounts
        for (price, amount) in &ob.bids {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Bid has zero amount at price {}", price);
        }
        for (price, amount) in &ob.asks {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Ask has zero amount at price {}", price);
        }

        // Verify spread is non-negative (if both sides present)
        if !ob.bids.is_empty() && !ob.asks.is_empty() {
            let best_bid = ob.bids[0].0;
            let best_ask = ob.asks[0].0;
            assert!(
                best_ask >= best_bid,
                "Negative spread: best_bid={} > best_ask={}",
                best_bid,
                best_ask
            );
        }

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_trade_stream() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        let mut stream = ws.watch_trades("BTC/USDT").await.unwrap();

        // BTC/USDT trades should come frequently
        let trade = tokio::time::timeout(Duration::from_secs(15), stream.next())
            .await
            .expect("Timeout waiting for trade")
            .expect("Stream ended unexpectedly");

        assert_eq!(trade.symbol, "BTC/USDT");
        assert!(trade.price > rust_decimal::Decimal::ZERO);
        assert!(trade.amount > rust_decimal::Decimal::ZERO);

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_multiple_subscriptions() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        // Subscribe to multiple streams on the same connection
        let mut btc_stream = ws.watch_ticker("BTC/USDT").await.unwrap();
        let mut eth_stream = ws.watch_ticker("ETH/USDT").await.unwrap();

        // Both should receive data
        let btc_ticker = tokio::time::timeout(Duration::from_secs(10), btc_stream.next())
            .await
            .expect("Timeout for BTC ticker")
            .expect("BTC stream ended");

        let eth_ticker = tokio::time::timeout(Duration::from_secs(10), eth_stream.next())
            .await
            .expect("Timeout for ETH ticker")
            .expect("ETH stream ended");

        assert_eq!(btc_ticker.symbol, "BTC/USDT");
        assert_eq!(eth_ticker.symbol, "ETH/USDT");

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_ohlcv_stream() {
        let config = WsConfig::default();
        let ws = BinanceWs::new(false, config);

        let mut stream = ws
            .watch_ohlcv("BTC/USDT", ccxt::prelude::Timeframe::OneMinute)
            .await
            .unwrap();

        let candle = tokio::time::timeout(Duration::from_secs(90), stream.next())
            .await
            .expect("Timeout waiting for OHLCV")
            .expect("Stream ended unexpectedly");

        assert!(candle.timestamp > 0);

        ws.close().await.unwrap();
    }

    // === Private Stream Tests ===

    #[tokio::test]
    #[ignore]
    async fn ws_binance_private_watch_orders() {
        let api_key = match env_or_skip("BINANCE_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BINANCE_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BinanceWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_orders(None).await.expect("Auth/connect failed for watch_orders");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_private_watch_balance() {
        let api_key = match env_or_skip("BINANCE_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BINANCE_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BinanceWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_balance().await.expect("Auth/connect failed for watch_balance");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_private_watch_my_trades() {
        let api_key = match env_or_skip("BINANCE_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BINANCE_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BinanceWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_my_trades(None).await.expect("Auth/connect failed for watch_my_trades");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_binance_private_watch_positions() {
        let api_key = match env_or_skip("BINANCE_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BINANCE_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BinanceWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_positions(None).await.expect("Auth/connect failed for watch_positions");
        ws.close().await.unwrap();
    }
}

// =============================================================================
// BYBIT WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_ws {
    use super::env_or_skip;
    use ccxt::base::ws::{ExchangeWs, WsConfig};
    use ccxt::bybit::ws::BybitWs;
    use std::time::Duration;

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_ticker_stream() {
        let config = WsConfig::default();
        let ws = BybitWs::new(false, config);

        let mut stream = ws.watch_ticker("BTC/USDT").await.unwrap();

        let ticker = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for ticker")
            .expect("Stream ended unexpectedly");

        assert_eq!(ticker.symbol, "BTC/USDT");
        assert!(ticker.last.is_some());

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_orderbook_stream() {
        let config = WsConfig::default();
        let ws = BybitWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT", Some(20)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT");

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_orderbook_depth_sorted() {
        let config = WsConfig::default();
        let ws = BybitWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT", Some(20)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT");

        // Verify bids sorted descending (highest first)
        for window in ob.bids.windows(2) {
            assert!(
                window[0].0 >= window[1].0,
                "Bids not sorted descending: {} < {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify asks sorted ascending (lowest first)
        for window in ob.asks.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "Asks not sorted ascending: {} > {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify no zero amounts
        for (price, amount) in &ob.bids {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Bid has zero amount at price {}", price);
        }
        for (price, amount) in &ob.asks {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Ask has zero amount at price {}", price);
        }

        // Verify spread is non-negative (if both sides present)
        if !ob.bids.is_empty() && !ob.asks.is_empty() {
            let best_bid = ob.bids[0].0;
            let best_ask = ob.asks[0].0;
            assert!(
                best_ask >= best_bid,
                "Negative spread: best_bid={} > best_ask={}",
                best_bid,
                best_ask
            );
        }

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_trade_stream() {
        let config = WsConfig::default();
        let ws = BybitWs::new(false, config);

        let mut stream = ws.watch_trades("BTC/USDT").await.unwrap();

        let trade = tokio::time::timeout(Duration::from_secs(15), stream.next())
            .await
            .expect("Timeout waiting for trade")
            .expect("Stream ended unexpectedly");

        assert_eq!(trade.symbol, "BTC/USDT");
        assert!(trade.price > rust_decimal::Decimal::ZERO);
        assert!(trade.amount > rust_decimal::Decimal::ZERO);

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_multiple_subscriptions() {
        let config = WsConfig::default();
        let ws = BybitWs::new(false, config);

        let mut btc_stream = ws.watch_ticker("BTC/USDT").await.unwrap();
        let mut eth_stream = ws.watch_ticker("ETH/USDT").await.unwrap();

        let btc_ticker = tokio::time::timeout(Duration::from_secs(10), btc_stream.next())
            .await
            .expect("Timeout for BTC ticker")
            .expect("BTC stream ended");

        let eth_ticker = tokio::time::timeout(Duration::from_secs(10), eth_stream.next())
            .await
            .expect("Timeout for ETH ticker")
            .expect("ETH stream ended");

        assert_eq!(btc_ticker.symbol, "BTC/USDT");
        assert_eq!(eth_ticker.symbol, "ETH/USDT");

        ws.close().await.unwrap();
    }

    // === Private Stream Tests ===

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_private_watch_orders() {
        let api_key = match env_or_skip("BYBIT_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BYBIT_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BybitWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_orders(None).await.expect("Auth/connect failed for watch_orders");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_private_watch_balance() {
        let api_key = match env_or_skip("BYBIT_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BYBIT_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BybitWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_balance().await.expect("Auth/connect failed for watch_balance");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_private_watch_positions() {
        let api_key = match env_or_skip("BYBIT_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BYBIT_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BybitWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_positions(None).await.expect("Auth/connect failed for watch_positions");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_bybit_private_watch_my_trades() {
        let api_key = match env_or_skip("BYBIT_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("BYBIT_SANDBOX_SECRET") { Some(s) => s, None => return };

        let ws = BybitWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret);

        let _stream = ws.watch_my_trades(None).await.expect("Auth/connect failed for watch_my_trades");
        ws.close().await.unwrap();
    }
}

// =============================================================================
// OKX WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_ws {
    use super::env_or_skip;
    use ccxt::base::ws::{ExchangeWs, WsConfig};
    use ccxt::okx::ws::OkxWs;
    use std::time::Duration;

    #[tokio::test]
    #[ignore]
    async fn ws_okx_ticker_stream() {
        let config = WsConfig::default();
        let ws = OkxWs::new(false, config);

        let mut stream = ws.watch_ticker("BTC/USDT").await.unwrap();

        let ticker = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for ticker")
            .expect("Stream ended unexpectedly");

        assert_eq!(ticker.symbol, "BTC/USDT");
        assert!(ticker.last.is_some());

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_orderbook_stream() {
        let config = WsConfig::default();
        let ws = OkxWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT", Some(5)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT");

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_orderbook_depth_sorted() {
        let config = WsConfig::default();
        let ws = OkxWs::new(false, config);

        let mut stream = ws.watch_order_book("BTC/USDT:USDT", Some(20)).await.unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USDT:USDT");

        // Verify bids sorted descending (highest first)
        for window in ob.bids.windows(2) {
            assert!(
                window[0].0 >= window[1].0,
                "Bids not sorted descending: {} < {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify asks sorted ascending (lowest first)
        for window in ob.asks.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "Asks not sorted ascending: {} > {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify no zero amounts
        for (price, amount) in &ob.bids {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Bid has zero amount at price {}", price);
        }
        for (price, amount) in &ob.asks {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Ask has zero amount at price {}", price);
        }

        // Verify spread is non-negative (if both sides present)
        if !ob.bids.is_empty() && !ob.asks.is_empty() {
            let best_bid = ob.bids[0].0;
            let best_ask = ob.asks[0].0;
            assert!(
                best_ask >= best_bid,
                "Negative spread: best_bid={} > best_ask={}",
                best_bid,
                best_ask
            );
        }

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_trade_stream() {
        let config = WsConfig::default();
        let ws = OkxWs::new(false, config);

        let mut stream = ws.watch_trades("BTC/USDT").await.unwrap();

        let trade = tokio::time::timeout(Duration::from_secs(15), stream.next())
            .await
            .expect("Timeout waiting for trade")
            .expect("Stream ended unexpectedly");

        assert_eq!(trade.symbol, "BTC/USDT");
        assert!(trade.price > rust_decimal::Decimal::ZERO);
        assert!(trade.amount > rust_decimal::Decimal::ZERO);

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_multiple_subscriptions() {
        let config = WsConfig::default();
        let ws = OkxWs::new(false, config);

        let mut btc_stream = ws.watch_ticker("BTC/USDT").await.unwrap();
        let mut eth_stream = ws.watch_ticker("ETH/USDT").await.unwrap();

        let btc_ticker = tokio::time::timeout(Duration::from_secs(10), btc_stream.next())
            .await
            .expect("Timeout for BTC ticker")
            .expect("BTC stream ended");

        let eth_ticker = tokio::time::timeout(Duration::from_secs(10), eth_stream.next())
            .await
            .expect("Timeout for ETH ticker")
            .expect("ETH stream ended");

        assert_eq!(btc_ticker.symbol, "BTC/USDT");
        assert_eq!(eth_ticker.symbol, "ETH/USDT");

        ws.close().await.unwrap();
    }

    // === Private Stream Tests ===

    #[tokio::test]
    #[ignore]
    async fn ws_okx_private_watch_orders() {
        let api_key = match env_or_skip("OKX_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("OKX_SANDBOX_SECRET") { Some(s) => s, None => return };
        let passphrase = match env_or_skip("OKX_SANDBOX_PASSPHRASE") { Some(p) => p, None => return };

        let ws = OkxWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret, passphrase);

        let _stream = ws.watch_orders(None).await.expect("Auth/connect failed for watch_orders");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_private_watch_balance() {
        let api_key = match env_or_skip("OKX_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("OKX_SANDBOX_SECRET") { Some(s) => s, None => return };
        let passphrase = match env_or_skip("OKX_SANDBOX_PASSPHRASE") { Some(p) => p, None => return };

        let ws = OkxWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret, passphrase);

        let _stream = ws.watch_balance().await.expect("Auth/connect failed for watch_balance");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_private_watch_positions() {
        let api_key = match env_or_skip("OKX_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("OKX_SANDBOX_SECRET") { Some(s) => s, None => return };
        let passphrase = match env_or_skip("OKX_SANDBOX_PASSPHRASE") { Some(p) => p, None => return };

        let ws = OkxWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret, passphrase);

        let _stream = ws.watch_positions(None).await.expect("Auth/connect failed for watch_positions");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_okx_private_watch_my_trades() {
        let api_key = match env_or_skip("OKX_SANDBOX_API_KEY") { Some(k) => k, None => return };
        let secret = match env_or_skip("OKX_SANDBOX_SECRET") { Some(s) => s, None => return };
        let passphrase = match env_or_skip("OKX_SANDBOX_PASSPHRASE") { Some(p) => p, None => return };

        let ws = OkxWs::new(true, WsConfig::default())
            .with_credentials(api_key, secret, passphrase);

        let _stream = ws.watch_my_trades(None).await.expect("Auth/connect failed for watch_my_trades");
        ws.close().await.unwrap();
    }
}

// =============================================================================
// HYPERLIQUID WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "hyperliquid")]
mod hyperliquid_ws {
    use super::env_or_skip;
    use ccxt::base::ws::{ExchangeWs, WsConfig};
    use ccxt::hyperliquid::ws::HyperliquidWs;
    use std::time::Duration;

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_ticker_stream() {
        let config = WsConfig::default();
        let ws = HyperliquidWs::new(false, config);

        let mut stream = ws.watch_ticker("BTC/USD:USDC").await.unwrap();

        let ticker = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for ticker")
            .expect("Stream ended unexpectedly");

        assert_eq!(ticker.symbol, "BTC/USD:USDC");
        assert!(ticker.last.is_some());

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_orderbook_stream() {
        let config = WsConfig::default();
        let ws = HyperliquidWs::new(false, config);

        let mut stream = ws
            .watch_order_book("BTC/USD:USDC", None)
            .await
            .unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USD:USDC");

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_orderbook_depth_sorted() {
        let config = WsConfig::default();
        let ws = HyperliquidWs::new(false, config);

        let mut stream = ws
            .watch_order_book("BTC/USD:USDC", Some(20))
            .await
            .unwrap();

        let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("Timeout waiting for orderbook")
            .expect("Stream ended unexpectedly");

        assert_eq!(ob.symbol, "BTC/USD:USDC");

        // Verify bids sorted descending (highest first)
        for window in ob.bids.windows(2) {
            assert!(
                window[0].0 >= window[1].0,
                "Bids not sorted descending: {} < {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify asks sorted ascending (lowest first)
        for window in ob.asks.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "Asks not sorted ascending: {} > {}",
                window[0].0,
                window[1].0
            );
        }

        // Verify no zero amounts
        for (price, amount) in &ob.bids {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Bid has zero amount at price {}", price);
        }
        for (price, amount) in &ob.asks {
            assert!(*amount > rust_decimal::Decimal::ZERO, "Ask has zero amount at price {}", price);
        }

        // Verify spread is non-negative (if both sides present)
        if !ob.bids.is_empty() && !ob.asks.is_empty() {
            let best_bid = ob.bids[0].0;
            let best_ask = ob.asks[0].0;
            assert!(
                best_ask >= best_bid,
                "Negative spread: best_bid={} > best_ask={}",
                best_bid,
                best_ask
            );
        }

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_trade_stream() {
        let config = WsConfig::default();
        let ws = HyperliquidWs::new(false, config);

        let mut stream = ws.watch_trades("BTC/USD:USDC").await.unwrap();

        let trade = tokio::time::timeout(Duration::from_secs(15), stream.next())
            .await
            .expect("Timeout waiting for trade")
            .expect("Stream ended unexpectedly");

        assert_eq!(trade.symbol, "BTC/USD:USDC");
        assert!(trade.price > rust_decimal::Decimal::ZERO);
        assert!(trade.amount > rust_decimal::Decimal::ZERO);

        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_multiple_subscriptions() {
        let config = WsConfig::default();
        let ws = HyperliquidWs::new(false, config);

        let mut btc_stream = ws.watch_ticker("BTC/USD:USDC").await.unwrap();
        let mut eth_stream = ws.watch_ticker("ETH/USD:USDC").await.unwrap();

        let btc_ticker = tokio::time::timeout(Duration::from_secs(10), btc_stream.next())
            .await
            .expect("Timeout for BTC ticker")
            .expect("BTC stream ended");

        let eth_ticker = tokio::time::timeout(Duration::from_secs(10), eth_stream.next())
            .await
            .expect("Timeout for ETH ticker")
            .expect("ETH stream ended");

        assert_eq!(btc_ticker.symbol, "BTC/USD:USDC");
        assert_eq!(eth_ticker.symbol, "ETH/USD:USDC");

        ws.close().await.unwrap();
    }

    // === Private Stream Tests ===

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_private_watch_orders() {
        let private_key = match env_or_skip("HYPERLIQUID_PRIVATE_KEY") { Some(k) => k, None => return };
        let signer = ccxt::hyperliquid::signer::HyperliquidSigner::new(&private_key, true)
            .expect("Failed to create signer");
        let address = signer.address_hex();

        let ws = HyperliquidWs::new(true, WsConfig::default())
            .with_user_address(address);

        let _stream = ws.watch_orders(None).await.expect("Connect failed for watch_orders");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_private_watch_positions() {
        let private_key = match env_or_skip("HYPERLIQUID_PRIVATE_KEY") { Some(k) => k, None => return };
        let signer = ccxt::hyperliquid::signer::HyperliquidSigner::new(&private_key, true)
            .expect("Failed to create signer");
        let address = signer.address_hex();

        let ws = HyperliquidWs::new(true, WsConfig::default())
            .with_user_address(address);

        let _stream = ws.watch_positions(None).await.expect("Connect failed for watch_positions");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_private_watch_my_trades() {
        let private_key = match env_or_skip("HYPERLIQUID_PRIVATE_KEY") { Some(k) => k, None => return };
        let signer = ccxt::hyperliquid::signer::HyperliquidSigner::new(&private_key, true)
            .expect("Failed to create signer");
        let address = signer.address_hex();

        let ws = HyperliquidWs::new(true, WsConfig::default())
            .with_user_address(address);

        let _stream = ws.watch_my_trades(None).await.expect("Connect failed for watch_my_trades");
        ws.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn ws_hyperliquid_watch_balance_not_supported() {
        let ws = HyperliquidWs::new(false, WsConfig::default());
        let result = ws.watch_balance().await;
        match result {
            Err(ccxt::base::errors::CcxtError::NotSupported(_)) => {} // expected
            Err(e) => panic!("Expected NotSupported, got: {:?}", e),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }
}
