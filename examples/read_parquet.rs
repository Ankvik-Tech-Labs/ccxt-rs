//! Read and display Parquet data exported from Binance
//!
//! This example demonstrates how to read the Parquet files generated
//! by the binance_export_parquet example.

use polars::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Reading Binance Parquet Files ===\n");

    // 1. Read OHLCV data
    println!("1. BTC/USDT 1h OHLCV Data (first 5 candles):");
    let df_ohlcv = LazyFrame::scan_parquet("data/binance/btc_usdt_1h.parquet", Default::default())?
        .limit(5)
        .collect()?;
    println!("{}\n", df_ohlcv);

    // 2. Read Trades data
    println!("2. BTC/USDT Trades (first 5 trades):");
    let df_trades = LazyFrame::scan_parquet("data/binance/btc_usdt_trades.parquet", Default::default())?
        .limit(5)
        .collect()?;
    println!("{}\n", df_trades);

    // 3. Read Tickers
    println!("3. Ticker Snapshots:");
    let df_tickers = LazyFrame::scan_parquet("data/binance/tickers_snapshot.parquet", Default::default())?
        .collect()?;
    println!("{}\n", df_tickers);

    // 4. Read OrderBook
    println!("4. BTC/USDT Order Book (first 5 levels):");
    let df_orderbook = LazyFrame::scan_parquet("data/binance/btc_usdt_orderbook.parquet", Default::default())?
        .limit(5)
        .collect()?;
    println!("{}\n", df_orderbook);

    // 5. Show statistics
    println!("=== Statistics ===");
    let ohlcv_stats = LazyFrame::scan_parquet("data/binance/btc_usdt_1h.parquet", Default::default())?
        .select([
            col("close").min().alias("min_close"),
            col("close").max().alias("max_close"),
            col("close").mean().alias("avg_close"),
            col("volume").sum().alias("total_volume"),
        ])
        .collect()?;
    println!("\nOHLCV Statistics:");
    println!("{}", ohlcv_stats);

    Ok(())
}
