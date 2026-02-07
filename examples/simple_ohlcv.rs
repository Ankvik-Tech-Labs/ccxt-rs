//! Simple OHLCV Fetching Example
//!
//! This is a basic example showing how to fetch candlestick data
//! for Bitcoin (BTC/USDT) from Binance.
//!
//! Usage:
//!   cargo run --example simple_ohlcv --features binance

use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create Binance exchange instance
    let binance = ccxt::binance::Binance::builder()
        .sandbox(false)
        .build()?;

    println!("Fetching BTC/USDT hourly candles from {}...\n", binance.name());

    // Fetch last 10 hourly candles for BTC/USDT
    let candles = binance
        .fetch_ohlcv(
            "BTC/USDT",           // Symbol
            Timeframe::OneHour,   // Timeframe (1h candles)
            None,                 // Since (None = recent data)
            Some(10),             // Limit (number of candles)
        )
        .await?;

    println!("Fetched {} candles:\n", candles.len());

    // Print header
    println!("{:<20} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "Timestamp", "Open", "High", "Low", "Close", "Volume");
    println!("{}", "-".repeat(92));

    // Print each candle
    for candle in &candles {
        // Convert timestamp to readable format
        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(
            candle.timestamp / 1000,
            0,
        )
        .unwrap();

        println!(
            "{:<20} ${:>11} ${:>11} ${:>11} ${:>11} {:>12}",
            datetime.format("%Y-%m-%d %H:%M"),
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume.round_dp(2)
        );
    }

    // Calculate some basic stats
    println!("\n=== Statistics ===");

    let first = candles.first().unwrap();
    let last = candles.last().unwrap();

    let price_change = last.close - first.close;
    let price_change_pct = (price_change / first.close) * rust_decimal::Decimal::from(100);

    println!("Starting price: ${}", first.close);
    println!("Ending price:   ${}", last.close);
    println!("Price change:   ${} ({}{}%)",
        price_change.round_dp(2),
        if price_change >= rust_decimal::Decimal::ZERO { "+" } else { "" },
        price_change_pct.round_dp(2)
    );

    // Find highest and lowest
    let highest = candles.iter()
        .max_by(|a, b| a.high.cmp(&b.high))
        .unwrap();
    let lowest = candles.iter()
        .min_by(|a, b| a.low.cmp(&b.low))
        .unwrap();

    println!("Highest:        ${}", highest.high);
    println!("Lowest:         ${}", lowest.low);

    // Total volume
    let total_volume: rust_decimal::Decimal = candles.iter()
        .map(|c| c.volume)
        .sum();
    println!("Total volume:   {} BTC", total_volume.round_dp(2));

    Ok(())
}
