//! Bybit Public API Example
//!
//! This example demonstrates fetching public market data from Bybit using the unified CCXT API.
//!
//! Usage:
//!   cargo run --example bybit_public_api --features bybit

use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== CCXT-RS Bybit Public API Test ===\n");

    // Create Bybit exchange instance
    let bybit = ccxt::bybit::Bybit::builder()
        .sandbox(false)  // Use mainnet
        .build()?;

    println!("Exchange: {} ({})", bybit.name(), bybit.id());
    println!("Type: {:?}\n", bybit.exchange_type());

    // =========================================================================
    // 1. Fetch Single Ticker
    // =========================================================================
    println!("1. Fetching BTC/USDT ticker...");
    let ticker = bybit.fetch_ticker("BTC/USDT").await?;

    println!("   Symbol: {}", ticker.symbol);
    println!("   Last: ${}", ticker.last.unwrap());
    println!("   Bid: ${}", ticker.bid.unwrap());
    println!("   Ask: ${}", ticker.ask.unwrap());
    println!("   24h High: ${}", ticker.high.unwrap());
    println!("   24h Low: ${}", ticker.low.unwrap());
    println!("   24h Volume: {} BTC", ticker.base_volume.unwrap());
    println!("   24h Change: {}%", ticker.percentage.unwrap().round_dp(3));
    println!("   ✓ Success\n");

    // =========================================================================
    // 2. Fetch Order Book
    // =========================================================================
    println!("2. Fetching BTC/USDT order book (top 5)...");
    let orderbook = bybit.fetch_order_book("BTC/USDT", Some(5)).await?;

    println!("   Symbol: {}", orderbook.symbol);
    println!(
        "   Best Bid: ${} ({} BTC)",
        orderbook.bids[0].0,
        orderbook.bids[0].1.round_dp(5)
    );
    println!(
        "   Best Ask: ${} ({} BTC)",
        orderbook.asks[0].0,
        orderbook.asks[0].1.round_dp(5)
    );
    println!("   Spread: ${}", orderbook.asks[0].0 - orderbook.bids[0].0);
    println!("   ✓ Success\n");

    // =========================================================================
    // 3. Fetch Recent Trades
    // =========================================================================
    println!("3. Fetching recent BTC/USDT trades (5)...");
    let trades = bybit.fetch_trades("BTC/USDT", None, Some(5)).await?;

    println!("   Fetched {} trades:", trades.len());
    for (i, trade) in trades.iter().enumerate() {
        println!(
            "   {}. {:?} {} BTC @ ${} ({})",
            i + 1,
            trade.side,
            trade.amount.round_dp(5),
            trade.price,
            trade.id
        );
    }
    println!("   ✓ Success\n");

    // =========================================================================
    // 4. Fetch OHLCV (Candlestick) Data
    // =========================================================================
    println!("4. Fetching BTC/USDT 1h candles (last 3)...");
    let ohlcv = bybit
        .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(3))
        .await?;

    println!("   Fetched {} candles:", ohlcv.len());
    for (i, candle) in ohlcv.iter().enumerate() {
        println!(
            "   {}. O: ${} H: ${} L: ${} C: ${} V: {}",
            i + 1,
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume.round_dp(2)
        );
    }
    println!("   ✓ Success\n");

    // =========================================================================
    // 5. Fetch Markets
    // =========================================================================
    println!("5. Fetching markets (first 3)...");
    let markets = bybit.fetch_markets().await?;

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

    // =========================================================================
    // 6. Fetch Multiple Tickers
    // =========================================================================
    println!("6. Fetching tickers for BTC/USDT and ETH/USDT...");
    let tickers = bybit
        .fetch_tickers(Some(&["BTC/USDT", "ETH/USDT"]))
        .await?;

    println!("   Fetched {} tickers:", tickers.len());
    for ticker in tickers.iter() {
        println!(
            "   {}: ${} (24h change: {}%)",
            ticker.symbol,
            ticker.last.unwrap(),
            ticker.percentage.unwrap().round_dp(3)
        );
    }
    println!("   ✓ Success\n");

    // =========================================================================
    // 7. Compare Different Timeframes
    // =========================================================================
    println!("7. Comparing different timeframes (last candle)...");

    let timeframes = vec![
        ("1 minute", Timeframe::OneMinute),
        ("5 minutes", Timeframe::FiveMinutes),
        ("1 hour", Timeframe::OneHour),
        ("1 day", Timeframe::OneDay),
    ];

    for (name, timeframe) in timeframes {
        let ohlcv = bybit
            .fetch_ohlcv("BTC/USDT", timeframe, None, Some(1))
            .await?;

        if let Some(candle) = ohlcv.first() {
            println!(
                "   {} - Close: ${}, Volume: {} BTC",
                name,
                candle.close,
                candle.volume.round_dp(2)
            );
        }
    }
    println!("   ✓ Success\n");

    println!("=== All tests completed successfully! ===");

    Ok(())
}
