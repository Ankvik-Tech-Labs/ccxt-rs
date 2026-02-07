//! Test Binance public API implementation
//!
//! This example demonstrates fetching public market data from Binance
//! using the unified ccxt-rs API.

use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== CCXT-RS Binance Public API Test ===\n");

    // Create Binance client (no credentials needed for public API)
    let binance = ccxt::binance::Binance::builder()
        .sandbox(false) // Use production API
        .build()?;

    println!("Exchange: {} ({})", binance.name(), binance.id());
    println!("Type: {:?}\n", binance.exchange_type());

    // Test 1: Fetch ticker
    println!("1. Fetching BTC/USDT ticker...");
    match binance.fetch_ticker("BTC/USDT").await {
        Ok(ticker) => {
            println!("   Symbol: {}", ticker.symbol);
            println!("   Last: ${}", ticker.last.unwrap());
            println!("   Bid: ${}", ticker.bid.unwrap());
            println!("   Ask: ${}", ticker.ask.unwrap());
            println!("   24h High: ${}", ticker.high.unwrap());
            println!("   24h Low: ${}", ticker.low.unwrap());
            println!("   24h Volume: {} BTC", ticker.base_volume.unwrap());
            println!("   24h Change: {}%", ticker.percentage.unwrap());
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    // Test 2: Fetch order book
    println!("2. Fetching BTC/USDT order book (top 5)...");
    match binance.fetch_order_book("BTC/USDT", Some(5)).await {
        Ok(orderbook) => {
            println!("   Symbol: {}", orderbook.symbol);
            if let Some(best_bid) = orderbook.best_bid() {
                println!("   Best Bid: ${} ({}  BTC)", best_bid.0, best_bid.1);
            }
            if let Some(best_ask) = orderbook.best_ask() {
                println!("   Best Ask: ${} ({} BTC)", best_ask.0, best_ask.1);
            }
            if let Some(spread) = orderbook.spread() {
                println!("   Spread: ${}", spread);
            }
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    // Test 3: Fetch recent trades
    println!("3. Fetching recent BTC/USDT trades (5)...");
    match binance.fetch_trades("BTC/USDT", None, Some(5)).await {
        Ok(trades) => {
            println!("   Fetched {} trades:", trades.len());
            for (i, trade) in trades.iter().enumerate() {
                println!(
                    "   {}. {:?} {} BTC @ ${} ({})",
                    i + 1,
                    trade.side,
                    trade.amount,
                    trade.price,
                    trade.id
                );
            }
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    // Test 4: Fetch OHLCV (candlesticks)
    println!("4. Fetching BTC/USDT 1h candles (last 3)...");
    match binance.fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(3)).await {
        Ok(candles) => {
            println!("   Fetched {} candles:", candles.len());
            for (i, candle) in candles.iter().enumerate() {
                println!(
                    "   {}. O: ${} H: ${} L: ${} C: ${} V: {}",
                    i + 1,
                    candle.open,
                    candle.high,
                    candle.low,
                    candle.close,
                    candle.volume
                );
            }
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    // Test 5: Fetch markets
    println!("5. Fetching markets (first 3)...");
    match binance.fetch_markets().await {
        Ok(markets) => {
            println!("   Total markets: {}", markets.len());
            println!("   First 3 markets:");
            for (i, market) in markets.iter().take(3).enumerate() {
                println!(
                    "   {}. {} ({}): {} trading, Precision: price={:?} amount={:?}",
                    i + 1,
                    market.symbol,
                    market.market_type,
                    if market.active { "Active" } else { "Inactive" },
                    market.precision.price,
                    market.precision.amount
                );
            }
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    // Test 6: Fetch multiple tickers
    println!("6. Fetching tickers for BTC/USDT and ETH/USDT...");
    match binance.fetch_tickers(Some(&["BTC/USDT", "ETH/USDT"])).await {
        Ok(tickers) => {
            println!("   Fetched {} tickers:", tickers.len());
            for ticker in tickers {
                println!(
                    "   {}: ${} (24h change: {}%)",
                    ticker.symbol,
                    ticker.last.unwrap_or_default(),
                    ticker.percentage.unwrap_or_default()
                );
            }
            println!("   ✓ Success\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
            return Err(e);
        }
    }

    println!("=== All tests completed successfully! ===");

    Ok(())
}
