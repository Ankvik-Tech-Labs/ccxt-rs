//! Fetch OHLCV (Candlestick) Data Example
//!
//! This example demonstrates how to fetch historical price data (OHLCV - Open, High, Low, Close, Volume)
//! for BTC/USDT across different timeframes.
//!
//! Usage:
//!   cargo run --example fetch_ohlcv --features binance

use ccxt::prelude::*;
use chrono::{DateTime, Utc};

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== OHLCV Data Fetching Example ===\n");

    // Create Binance exchange instance
    let binance = ccxt::binance::Binance::builder()
        .sandbox(false)
        .build()?;

    println!("Exchange: {} ({})\n", binance.name(), binance.id());

    // =========================================================================
    // Example 1: Fetch Recent Candles (Last 10)
    // =========================================================================
    println!("1. Fetching last 10 candles for BTC/USDT (1 hour timeframe)...");
    let ohlcv_recent = binance
        .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(10))
        .await?;

    println!("   Fetched {} candles:\n", ohlcv_recent.len());
    print_ohlcv_table(&ohlcv_recent);
    print_statistics(&ohlcv_recent);
    println!();

    // =========================================================================
    // Example 2: Fetch Specific Date Range
    // =========================================================================
    println!("2. Fetching BTC/USDT 1h candles for last 24 hours...");

    // Calculate timestamp for 24 hours ago (in milliseconds)
    let now = Utc::now().timestamp_millis();
    let twenty_four_hours_ago = now - (24 * 60 * 60 * 1000);

    let ohlcv_24h = binance
        .fetch_ohlcv(
            "BTC/USDT",
            Timeframe::OneHour,
            Some(twenty_four_hours_ago),
            None, // No limit, fetch all since timestamp
        )
        .await?;

    println!("   Fetched {} candles in the last 24 hours", ohlcv_24h.len());
    println!("   First candle: {}", format_timestamp(ohlcv_24h.first().unwrap().timestamp));
    println!("   Last candle:  {}", format_timestamp(ohlcv_24h.last().unwrap().timestamp));
    print_statistics(&ohlcv_24h);
    println!();

    // =========================================================================
    // Example 3: Different Timeframes
    // =========================================================================
    println!("3. Comparing different timeframes (last 5 candles each)...\n");

    let timeframes = vec![
        ("1 minute", Timeframe::OneMinute),
        ("5 minutes", Timeframe::FiveMinutes),
        ("15 minutes", Timeframe::FifteenMinutes),
        ("1 hour", Timeframe::OneHour),
        ("4 hours", Timeframe::FourHours),
        ("1 day", Timeframe::OneDay),
    ];

    for (name, timeframe) in timeframes {
        let ohlcv = binance
            .fetch_ohlcv("BTC/USDT", timeframe, None, Some(5))
            .await?;

        if let Some(last_candle) = ohlcv.last() {
            println!(
                "   {} | Close: ${:>10} | Volume: {:>10} BTC | Range: ${} - ${}",
                name.to_uppercase().to_string() + &" ".repeat(15 - name.len()),
                last_candle.close,
                last_candle.volume,
                last_candle.low,
                last_candle.high
            );
        }
    }
    println!();

    // =========================================================================
    // Example 4: Large Dataset (Last 100 Candles)
    // =========================================================================
    println!("4. Fetching large dataset (last 100 daily candles)...");
    let ohlcv_100 = binance
        .fetch_ohlcv("BTC/USDT", Timeframe::OneDay, None, Some(100))
        .await?;

    println!("   Fetched {} candles", ohlcv_100.len());
    println!("   Date range: {} to {}",
        format_timestamp(ohlcv_100.first().unwrap().timestamp),
        format_timestamp(ohlcv_100.last().unwrap().timestamp)
    );

    // Calculate some interesting metrics
    let highest = ohlcv_100.iter()
        .max_by(|a, b| a.high.cmp(&b.high))
        .unwrap();
    let lowest = ohlcv_100.iter()
        .min_by(|a, b| a.low.cmp(&b.low))
        .unwrap();

    println!("\n   === 100-Day Analysis ===");
    println!("   Highest: ${} on {}", highest.high, format_timestamp(highest.timestamp));
    println!("   Lowest:  ${} on {}", lowest.low, format_timestamp(lowest.timestamp));
    print_statistics(&ohlcv_100);
    println!();

    // =========================================================================
    // Example 5: Multiple Symbols
    // =========================================================================
    println!("5. Fetching OHLCV for multiple symbols (last candle, 1h)...\n");

    let symbols = vec!["BTC/USDT", "ETH/USDT", "BNB/USDT", "SOL/USDT", "XRP/USDT"];

    for symbol in symbols {
        let ohlcv = binance
            .fetch_ohlcv(symbol, Timeframe::OneHour, None, Some(1))
            .await?;

        if let Some(candle) = ohlcv.first() {
            let change_pct = ((candle.close - candle.open) / candle.open) * rust_decimal::Decimal::from(100);
            let change_symbol = if change_pct >= rust_decimal::Decimal::ZERO { "+" } else { "" };

            println!(
                "   {:>10} | ${:>12} | {}{}% | Vol: {:>10}",
                symbol,
                candle.close,
                change_symbol,
                change_pct.round_dp(2),
                candle.volume.round_dp(2)
            );
        }
    }
    println!();

    // =========================================================================
    // Example 6: Pagination (Fetching More Than Exchange Limit)
    // =========================================================================
    println!("6. Demonstrating pagination (fetching 200 candles in batches)...");

    // Most exchanges limit OHLCV requests to ~500-1000 candles per request
    // To fetch more, we need to paginate
    let mut all_candles: Vec<OHLCV> = Vec::new();
    let batch_size = 100;
    let total_needed = 200;

    // Start from 200 hours ago
    let start_time = now - (200 * 60 * 60 * 1000);
    let mut current_since = start_time;

    while all_candles.len() < total_needed {
        let batch = binance
            .fetch_ohlcv(
                "BTC/USDT",
                Timeframe::OneHour,
                Some(current_since),
                Some(batch_size),
            )
            .await?;

        if batch.is_empty() {
            break;
        }

        // Update since to last candle timestamp + 1 hour
        if let Some(last_candle) = batch.last() {
            current_since = last_candle.timestamp + (60 * 60 * 1000); // +1 hour
        }

        all_candles.extend(batch);

        if all_candles.len() >= total_needed {
            all_candles.truncate(total_needed);
            break;
        }
    }

    println!("   Fetched {} candles through pagination", all_candles.len());
    println!("   Date range: {} to {}",
        format_timestamp(all_candles.first().unwrap().timestamp),
        format_timestamp(all_candles.last().unwrap().timestamp)
    );
    print_statistics(&all_candles);
    println!();

    println!("=== Examples Complete ===");

    Ok(())
}

/// Print OHLCV data in a formatted table
fn print_ohlcv_table(ohlcv: &[OHLCV]) {
    println!("   {:^19} | {:>12} | {:>12} | {:>12} | {:>12} | {:>10}",
        "Time", "Open", "High", "Low", "Close", "Volume");
    println!("   {}", "-".repeat(95));

    for candle in ohlcv.iter().take(10) {
        println!(
            "   {} | ${:>11} | ${:>11} | ${:>11} | ${:>11} | {:>10}",
            format_timestamp(candle.timestamp),
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume.round_dp(2)
        );
    }
}

/// Calculate and print basic statistics
fn print_statistics(ohlcv: &[OHLCV]) {
    if ohlcv.is_empty() {
        return;
    }

    let sum: rust_decimal::Decimal = ohlcv.iter().map(|c| c.close).sum();
    let avg = sum / rust_decimal::Decimal::from(ohlcv.len());

    let max_close = ohlcv.iter()
        .map(|c| c.close)
        .max()
        .unwrap();

    let min_close = ohlcv.iter()
        .map(|c| c.close)
        .min()
        .unwrap();

    let total_volume: rust_decimal::Decimal = ohlcv.iter().map(|c| c.volume).sum();

    let price_change = ohlcv.last().unwrap().close - ohlcv.first().unwrap().close;
    let price_change_pct = (price_change / ohlcv.first().unwrap().close) * rust_decimal::Decimal::from(100);

    println!("\n   === Statistics ===");
    println!("   Candles:       {}", ohlcv.len());
    println!("   Avg Close:     ${}", avg.round_dp(2));
    println!("   Max Close:     ${}", max_close);
    println!("   Min Close:     ${}", min_close);
    println!("   Total Volume:  {} BTC", total_volume.round_dp(2));
    println!("   Price Change:  ${} ({}{}%)",
        price_change.round_dp(2),
        if price_change >= rust_decimal::Decimal::ZERO { "+" } else { "" },
        price_change_pct.round_dp(2)
    );
}

/// Format timestamp to readable date
fn format_timestamp(timestamp: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp(timestamp / 1000, 0)
        .unwrap_or_else(|| Utc::now());
    dt.format("%Y-%m-%d %H:%M").to_string()
}
