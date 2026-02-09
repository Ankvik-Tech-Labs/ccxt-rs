//! WebSocket Integration Tests
//!
//! All tests are #[ignore] and require live network connections.
//! Public tests connect to real exchange WebSocket endpoints.
//!
//! Run with:
//!   cargo test --all-features -- --ignored ws_ --test-threads=1

// =============================================================================
// BINANCE WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_ws {
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
}

// =============================================================================
// BYBIT WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_ws {
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
}

// =============================================================================
// OKX WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_ws {
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
}

// =============================================================================
// HYPERLIQUID WEBSOCKET TESTS
// =============================================================================

#[cfg(feature = "hyperliquid")]
mod hyperliquid_ws {
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
}
