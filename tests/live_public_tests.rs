//! Live Public API Tests (Tier 3)
//!
//! Hit real exchange APIs and validate output passes type validators.
//! All tests are marked #[ignore] — run with:
//!   cargo test --all-features -- --ignored live_public --test-threads=1

mod validators;

// =============================================================================
// BINANCE LIVE PUBLIC TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_live_public {
    use super::validators;
    use ccxt::binance::Binance;
    use ccxt::prelude::*;

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_ticker() {
        let exchange = Binance::builder().build().unwrap();
        let ticker = exchange.fetch_ticker("BTC/USDT").await.unwrap();
        let errors = validators::validate_ticker(&ticker);
        assert!(errors.is_empty(), "Ticker validation failed: {:?}", errors);
        assert_eq!(ticker.symbol, "BTC/USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_tickers() {
        let exchange = Binance::builder().build().unwrap();
        let tickers = exchange
            .fetch_tickers(Some(&["BTC/USDT", "ETH/USDT"]))
            .await
            .unwrap();
        assert!(tickers.len() >= 2, "Expected at least 2 tickers");
        for ticker in &tickers {
            let errors = validators::validate_ticker(ticker);
            assert!(
                errors.is_empty(),
                "Ticker {} validation failed: {:?}",
                ticker.symbol,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_order_book() {
        let exchange = Binance::builder().build().unwrap();
        let ob = exchange
            .fetch_order_book("BTC/USDT", Some(5))
            .await
            .unwrap();
        let errors = validators::validate_order_book(&ob);
        assert!(
            errors.is_empty(),
            "OrderBook validation failed: {:?}",
            errors
        );
        assert_eq!(ob.symbol, "BTC/USDT");
        assert_eq!(ob.bids.len(), 5);
        assert_eq!(ob.asks.len(), 5);
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_trades() {
        let exchange = Binance::builder().build().unwrap();
        let trades = exchange
            .fetch_trades("BTC/USDT", None, Some(5))
            .await
            .unwrap();
        assert_eq!(trades.len(), 5, "Expected 5 trades");
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade {} validation failed: {:?}",
                trade.id,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_ohlcv() {
        let exchange = Binance::builder().build().unwrap();
        let ohlcv = exchange
            .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(3))
            .await
            .unwrap();
        assert_eq!(ohlcv.len(), 3, "Expected 3 candles");
        for candle in &ohlcv {
            let errors = validators::validate_ohlcv(candle);
            assert!(errors.is_empty(), "OHLCV validation failed: {:?}", errors);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_binance_fetch_markets() {
        let exchange = Binance::builder().build().unwrap();
        let markets = exchange.fetch_markets().await.unwrap();
        assert!(!markets.is_empty(), "Markets should not be empty");

        // Validate first 10 markets
        for market in markets.iter().take(10) {
            let errors = validators::validate_market(market);
            assert!(
                errors.is_empty(),
                "Market {} validation failed: {:?}",
                market.symbol,
                errors
            );
        }
    }
}

// =============================================================================
// BYBIT LIVE PUBLIC TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_live_public {
    use super::validators;
    use ccxt::bybit::Bybit;
    use ccxt::prelude::*;

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_ticker() {
        let exchange = Bybit::builder().build().unwrap();
        let ticker = exchange.fetch_ticker("BTC/USDT").await.unwrap();
        let errors = validators::validate_ticker(&ticker);
        assert!(errors.is_empty(), "Ticker validation failed: {:?}", errors);
        assert_eq!(ticker.symbol, "BTC/USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_tickers() {
        let exchange = Bybit::builder().build().unwrap();
        let tickers = exchange
            .fetch_tickers(Some(&["BTC/USDT", "ETH/USDT"]))
            .await
            .unwrap();
        assert!(tickers.len() >= 2, "Expected at least 2 tickers");
        for ticker in &tickers {
            let errors = validators::validate_ticker(ticker);
            assert!(
                errors.is_empty(),
                "Ticker {} validation failed: {:?}",
                ticker.symbol,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_order_book() {
        let exchange = Bybit::builder().build().unwrap();
        let ob = exchange
            .fetch_order_book("BTC/USDT", Some(5))
            .await
            .unwrap();
        let errors = validators::validate_order_book(&ob);
        assert!(
            errors.is_empty(),
            "OrderBook validation failed: {:?}",
            errors
        );
        assert_eq!(ob.symbol, "BTC/USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_trades() {
        let exchange = Bybit::builder().build().unwrap();
        let trades = exchange
            .fetch_trades("BTC/USDT", None, Some(5))
            .await
            .unwrap();
        assert!(!trades.is_empty(), "Expected some trades");
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade {} validation failed: {:?}",
                trade.id,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_ohlcv() {
        let exchange = Bybit::builder().build().unwrap();
        let ohlcv = exchange
            .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(3))
            .await
            .unwrap();
        assert_eq!(ohlcv.len(), 3, "Expected 3 candles");
        for candle in &ohlcv {
            let errors = validators::validate_ohlcv(candle);
            assert!(errors.is_empty(), "OHLCV validation failed: {:?}", errors);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_bybit_fetch_markets() {
        let exchange = Bybit::builder().build().unwrap();
        let markets = exchange.fetch_markets().await.unwrap();
        assert!(!markets.is_empty(), "Markets should not be empty");

        for market in markets.iter().take(10) {
            let errors = validators::validate_market(market);
            assert!(
                errors.is_empty(),
                "Market {} validation failed: {:?}",
                market.symbol,
                errors
            );
        }
    }
}

// =============================================================================
// OKX LIVE PUBLIC TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_live_public {
    use super::validators;
    use ccxt::okx::Okx;
    use ccxt::prelude::*;

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_ticker() {
        let exchange = Okx::builder().build().unwrap();
        let ticker = exchange.fetch_ticker("BTC/USDT").await.unwrap();
        let errors = validators::validate_ticker(&ticker);
        assert!(errors.is_empty(), "Ticker validation failed: {:?}", errors);
        assert_eq!(ticker.symbol, "BTC/USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_tickers() {
        let exchange = Okx::builder().build().unwrap();
        let tickers = exchange
            .fetch_tickers(Some(&["BTC/USDT", "ETH/USDT"]))
            .await
            .unwrap();
        assert!(tickers.len() >= 2, "Expected at least 2 tickers");
        for ticker in &tickers {
            let errors = validators::validate_ticker(ticker);
            assert!(
                errors.is_empty(),
                "Ticker {} validation failed: {:?}",
                ticker.symbol,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_order_book() {
        let exchange = Okx::builder().build().unwrap();
        let ob = exchange
            .fetch_order_book("BTC/USDT", Some(5))
            .await
            .unwrap();
        let errors = validators::validate_order_book(&ob);
        assert!(
            errors.is_empty(),
            "OrderBook validation failed: {:?}",
            errors
        );
        assert_eq!(ob.symbol, "BTC/USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_trades() {
        let exchange = Okx::builder().build().unwrap();
        let trades = exchange
            .fetch_trades("BTC/USDT", None, Some(5))
            .await
            .unwrap();
        assert!(!trades.is_empty(), "Expected some trades");
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade {} validation failed: {:?}",
                trade.id,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_ohlcv() {
        let exchange = Okx::builder().build().unwrap();
        let ohlcv = exchange
            .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(3))
            .await
            .unwrap();
        assert_eq!(ohlcv.len(), 3, "Expected 3 candles");
        for candle in &ohlcv {
            let errors = validators::validate_ohlcv(candle);
            assert!(errors.is_empty(), "OHLCV validation failed: {:?}", errors);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_okx_fetch_markets() {
        let exchange = Okx::builder().build().unwrap();
        let markets = exchange.fetch_markets().await.unwrap();
        assert!(!markets.is_empty(), "Markets should not be empty");

        for market in markets.iter().take(10) {
            let errors = validators::validate_market(market);
            assert!(
                errors.is_empty(),
                "Market {} validation failed: {:?}",
                market.symbol,
                errors
            );
        }
    }
}

// =============================================================================
// HYPERLIQUID LIVE PUBLIC TESTS
// =============================================================================

#[cfg(feature = "hyperliquid")]
mod hyperliquid_live_public {
    use super::validators;
    use ccxt::hyperliquid::Hyperliquid;
    use ccxt::prelude::*;

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_ticker() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let ticker = exchange.fetch_ticker("BTC/USD:USDC").await.unwrap();
        let errors = validators::validate_ticker(&ticker);
        assert!(errors.is_empty(), "Ticker validation failed: {:?}", errors);
        assert_eq!(ticker.symbol, "BTC/USD:USDC");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_tickers() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let tickers = exchange
            .fetch_tickers(Some(&["BTC/USD:USDC", "ETH/USD:USDC"]))
            .await
            .unwrap();
        assert!(tickers.len() >= 2, "Expected at least 2 tickers");
        for ticker in &tickers {
            let errors = validators::validate_ticker(ticker);
            assert!(
                errors.is_empty(),
                "Ticker {} validation failed: {:?}",
                ticker.symbol,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_order_book() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let ob = exchange
            .fetch_order_book("BTC/USD:USDC", Some(5))
            .await
            .unwrap();
        let errors = validators::validate_order_book(&ob);
        assert!(
            errors.is_empty(),
            "OrderBook validation failed: {:?}",
            errors
        );
        assert_eq!(ob.symbol, "BTC/USD:USDC");
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_trades() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let trades = exchange
            .fetch_trades("BTC/USD:USDC", None, Some(5))
            .await
            .unwrap();
        assert!(!trades.is_empty(), "Expected some trades");
        for trade in &trades {
            let errors = validators::validate_trade(trade);
            assert!(
                errors.is_empty(),
                "Trade {} validation failed: {:?}",
                trade.id,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_ohlcv() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let ohlcv = exchange
            .fetch_ohlcv("BTC/USD:USDC", Timeframe::OneHour, None, Some(3))
            .await
            .unwrap();
        assert_eq!(ohlcv.len(), 3, "Expected 3 candles");
        for candle in &ohlcv {
            let errors = validators::validate_ohlcv(candle);
            assert!(errors.is_empty(), "OHLCV validation failed: {:?}", errors);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_markets() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let markets = exchange.fetch_markets().await.unwrap();
        assert!(!markets.is_empty(), "Markets should not be empty");

        for market in markets.iter().take(10) {
            let errors = validators::validate_market(market);
            assert!(
                errors.is_empty(),
                "Market {} validation failed: {:?}",
                market.symbol,
                errors
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_public_hl_fetch_funding_rate() {
        let exchange = Hyperliquid::builder().build().unwrap();
        let funding_rate = exchange.fetch_funding_rate("BTC/USD:USDC").await.unwrap();
        let errors = validators::validate_funding_rate(&funding_rate);
        assert!(
            errors.is_empty(),
            "FundingRate validation failed: {:?}",
            errors
        );
        assert_eq!(funding_rate.symbol, "BTC/USD:USDC");
    }
}
