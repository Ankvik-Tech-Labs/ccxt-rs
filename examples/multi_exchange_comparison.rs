//! Multi-Exchange Comparison Example
//!
//! This example demonstrates fetching data from all three exchanges (Binance, Bybit, OKX)
//! simultaneously and comparing prices for arbitrage opportunities.
//!
//! Usage:
//!   cargo run --example multi_exchange_comparison --all-features

use ccxt::prelude::*;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Multi-Exchange Comparison ===\n");

    // Create all three exchange instances
    let binance = ccxt::binance::Binance::builder().sandbox(false).build()?;
    let bybit = ccxt::bybit::Bybit::builder().sandbox(false).build()?;
    let okx = ccxt::okx::Okx::builder().use_aws(false).build()?;

    println!("Exchanges initialized:");
    println!("  - {}", binance.name());
    println!("  - {}", bybit.name());
    println!("  - {}\n", okx.name());

    // =========================================================================
    // 1. Concurrent Ticker Fetch
    // =========================================================================
    println!("1. Fetching BTC/USDT ticker from all exchanges concurrently...");

    let (ticker_binance, ticker_bybit, ticker_okx) = tokio::try_join!(
        binance.fetch_ticker("BTC/USDT"),
        bybit.fetch_ticker("BTC/USDT"),
        okx.fetch_ticker("BTC/USDT"),
    )?;

    println!("\n   Ticker Comparison:");
    println!("   ┌───────────┬──────────────┬──────────────┬──────────────┬──────────┐");
    println!("   │ Exchange  │ Last Price   │ Bid          │ Ask          │ 24h Chg% │");
    println!("   ├───────────┼──────────────┼──────────────┼──────────────┼──────────┤");
    println!(
        "   │ Binance   │ ${:<11} │ ${:<11} │ ${:<11} │ {:>7}% │",
        ticker_binance.last.unwrap(),
        ticker_binance.bid.unwrap(),
        ticker_binance.ask.unwrap(),
        ticker_binance.percentage.unwrap().round_dp(2)
    );
    println!(
        "   │ Bybit     │ ${:<11} │ ${:<11} │ ${:<11} │ {:>7}% │",
        ticker_bybit.last.unwrap(),
        ticker_bybit.bid.unwrap(),
        ticker_bybit.ask.unwrap(),
        ticker_bybit.percentage.unwrap().round_dp(2)
    );
    println!(
        "   │ OKX       │ ${:<11} │ ${:<11} │ ${:<11} │ {:>7}% │",
        ticker_okx.last.unwrap(),
        ticker_okx.bid.unwrap(),
        ticker_okx.ask.unwrap(),
        ticker_okx.percentage.unwrap().round_dp(2)
    );
    println!("   └───────────┴──────────────┴──────────────┴──────────────┴──────────┘\n");

    // =========================================================================
    // 2. Arbitrage Opportunity Detection
    // =========================================================================
    println!("2. Analyzing arbitrage opportunities...");

    let prices = vec![
        ("Binance", ticker_binance.last.unwrap()),
        ("Bybit", ticker_bybit.last.unwrap()),
        ("OKX", ticker_okx.last.unwrap()),
    ];

    let min = prices.iter().min_by_key(|(_, p)| p).unwrap();
    let max = prices.iter().max_by_key(|(_, p)| p).unwrap();
    let spread = max.1 - min.1;
    let spread_pct = (spread / min.1) * Decimal::from(100);

    println!("\n   Arbitrage Analysis:");
    println!("   📉 Lowest:  {} @ ${}", min.0, min.1);
    println!("   📈 Highest: {} @ ${}", max.0, max.1);
    println!("   💰 Spread:  ${} ({}%)", spread.round_dp(2), spread_pct.round_dp(4));

    if spread_pct > Decimal::from_str_exact("0.1").unwrap() {
        println!("   ⚠️  Potential arbitrage opportunity detected!");
    } else {
        println!("   ✅ Prices are fairly aligned across exchanges.");
    }
    println!();

    // =========================================================================
    // 3. Market Count Comparison
    // =========================================================================
    println!("3. Fetching market counts...");

    let (markets_binance, markets_bybit, markets_okx) = tokio::try_join!(
        binance.fetch_markets(),
        bybit.fetch_markets(),
        okx.fetch_markets(),
    )?;

    println!("\n   Market Counts:");
    println!("   - Binance: {} trading pairs", markets_binance.len());
    println!("   - Bybit:   {} trading pairs", markets_bybit.len());
    println!("   - OKX:     {} trading pairs", markets_okx.len());
    println!(
        "   - Total:   {} trading pairs across all exchanges\n",
        markets_binance.len() + markets_bybit.len() + markets_okx.len()
    );

    // =========================================================================
    // 4. Multi-Symbol Ticker Comparison
    // =========================================================================
    println!("4. Fetching tickers for multiple symbols...");

    let symbols = vec!["BTC/USDT", "ETH/USDT", "BNB/USDT"];

    println!("\n   Symbol Comparison:");
    println!("   ┌────────────┬──────────────┬──────────────┬──────────────┐");
    println!("   │ Symbol     │ Binance      │ Bybit        │ OKX          │");
    println!("   ├────────────┼──────────────┼──────────────┼──────────────┤");

    for symbol in &symbols {
        let (t1, t2, t3) = tokio::try_join!(
            binance.fetch_ticker(symbol),
            bybit.fetch_ticker(symbol),
            okx.fetch_ticker(symbol),
        )?;

        println!(
            "   │ {:<10} │ ${:<11} │ ${:<11} │ ${:<11} │",
            symbol,
            t1.last.unwrap(),
            t2.last.unwrap(),
            t3.last.unwrap()
        );
    }
    println!("   └────────────┴──────────────┴──────────────┴──────────────┘\n");

    // =========================================================================
    // 5. OHLCV Data Comparison
    // =========================================================================
    println!("5. Fetching OHLCV data (last 1h candle) from all exchanges...");

    let (ohlcv_binance, ohlcv_bybit, ohlcv_okx) = tokio::try_join!(
        binance.fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(1)),
        bybit.fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(1)),
        okx.fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(1)),
    )?;

    println!("\n   OHLCV Comparison (Last 1h Candle):");
    println!("   ┌───────────┬──────────────┬──────────────┬──────────────┬──────────────┐");
    println!("   │ Exchange  │ Open         │ High         │ Low          │ Close        │");
    println!("   ├───────────┼──────────────┼──────────────┼──────────────┼──────────────┤");

    if let Some(candle) = ohlcv_binance.first() {
        println!(
            "   │ Binance   │ ${:<11} │ ${:<11} │ ${:<11} │ ${:<11} │",
            candle.open, candle.high, candle.low, candle.close
        );
    }

    if let Some(candle) = ohlcv_bybit.first() {
        println!(
            "   │ Bybit     │ ${:<11} │ ${:<11} │ ${:<11} │ ${:<11} │",
            candle.open, candle.high, candle.low, candle.close
        );
    }

    if let Some(candle) = ohlcv_okx.first() {
        println!(
            "   │ OKX       │ ${:<11} │ ${:<11} │ ${:<11} │ ${:<11} │",
            candle.open, candle.high, candle.low, candle.close
        );
    }

    println!("   └───────────┴──────────────┴──────────────┴──────────────┴──────────────┘\n");

    // =========================================================================
    // 6. Volume Comparison
    // =========================================================================
    println!("6. Volume comparison (24h)...");

    println!("\n   24h Volume Comparison:");
    println!("   - Binance: {} BTC", ticker_binance.base_volume.unwrap().round_dp(2));
    println!("   - Bybit:   {} BTC", ticker_bybit.base_volume.unwrap().round_dp(2));
    println!("   - OKX:     {} BTC", ticker_okx.base_volume.unwrap().round_dp(2));

    let total_volume = ticker_binance.base_volume.unwrap()
        + ticker_bybit.base_volume.unwrap()
        + ticker_okx.base_volume.unwrap();
    println!("   - Total:   {} BTC\n", total_volume.round_dp(2));

    // =========================================================================
    // Summary
    // =========================================================================
    println!("=== Summary ===");
    println!("✅ Successfully fetched data from all 3 exchanges");
    println!("✅ All exchanges using the same unified API");
    println!("✅ Concurrent fetching for maximum performance");
    println!("✅ Type-safe Decimal arithmetic for all prices");
    println!("\n🎉 Multi-exchange integration working perfectly!");

    Ok(())
}
