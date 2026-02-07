//! Export Binance data to Parquet files using Polars
//!
//! This example demonstrates how to fetch market data from Binance and export it
//! to Parquet files for backtesting or analysis. This mirrors CCXT's common pattern
//! of fetching data and storing it for later use.

use ccxt::prelude::*;
use polars::prelude::*;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== CCXT-RS Binance Data Export to Parquet ===\n");

    // Create output directory
    fs::create_dir_all("data/binance")
        .map_err(|e| CcxtError::ConfigError(format!("Failed to create directory: {}", e)))?;

    // Create Binance client
    let binance = ccxt::binance::Binance::builder()
        .sandbox(false)
        .build()?;

    println!("Fetching and exporting market data...\n");

    // 1. Export OHLCV (candlestick) data
    println!("1. Fetching BTC/USDT 1h OHLCV data (last 100 candles)...");
    let ohlcv_data = binance
        .fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(100))
        .await?;

    export_ohlcv_to_parquet(&ohlcv_data, "data/binance/btc_usdt_1h.parquet")?;
    println!("   ✓ Exported {} candles to data/binance/btc_usdt_1h.parquet\n", ohlcv_data.len());

    // 2. Export Trades data
    println!("2. Fetching BTC/USDT recent trades...");
    let trades_data = binance
        .fetch_trades("BTC/USDT", None, Some(1000))
        .await?;

    export_trades_to_parquet(&trades_data, "data/binance/btc_usdt_trades.parquet")?;
    println!("   ✓ Exported {} trades to data/binance/btc_usdt_trades.parquet\n", trades_data.len());

    // 3. Export Ticker data
    println!("3. Fetching tickers for multiple pairs...");
    let tickers = binance
        .fetch_tickers(Some(&["BTC/USDT", "ETH/USDT", "BNB/USDT", "SOL/USDT"]))
        .await?;

    export_tickers_to_parquet(&tickers, "data/binance/tickers_snapshot.parquet")?;
    println!("   ✓ Exported {} tickers to data/binance/tickers_snapshot.parquet\n", tickers.len());

    // 4. Export OrderBook snapshot
    println!("4. Fetching BTC/USDT order book...");
    let orderbook = binance
        .fetch_order_book("BTC/USDT", Some(100))
        .await?;

    export_orderbook_to_parquet(&orderbook, "data/binance/btc_usdt_orderbook.parquet")?;
    println!("   ✓ Exported order book to data/binance/btc_usdt_orderbook.parquet\n");

    println!("=== Export Complete ===");
    println!("\nTo read the data in Python:");
    println!("  import polars as pl");
    println!("  df = pl.read_parquet('data/binance/btc_usdt_1h.parquet')");
    println!("  print(df)");
    println!("\nOr in Rust:");
    println!("  let df = LazyFrame::scan_parquet('data/binance/btc_usdt_1h.parquet', Default::default())?;");
    println!("  let result = df.collect()?;");

    Ok(())
}

/// Export OHLCV data to Parquet format
fn export_ohlcv_to_parquet(ohlcv: &[OHLCV], path: &str) -> Result<()> {
    let timestamps: Vec<i64> = ohlcv.iter().map(|c| c.timestamp).collect();
    let opens: Vec<f64> = ohlcv.iter().map(|c| c.open.to_string().parse().unwrap()).collect();
    let highs: Vec<f64> = ohlcv.iter().map(|c| c.high.to_string().parse().unwrap()).collect();
    let lows: Vec<f64> = ohlcv.iter().map(|c| c.low.to_string().parse().unwrap()).collect();
    let closes: Vec<f64> = ohlcv.iter().map(|c| c.close.to_string().parse().unwrap()).collect();
    let volumes: Vec<f64> = ohlcv.iter().map(|c| c.volume.to_string().parse().unwrap()).collect();

    let df = DataFrame::new(vec![
        Column::Series(Series::new("timestamp".into(), timestamps)),
        Column::Series(Series::new("open".into(), opens)),
        Column::Series(Series::new("high".into(), highs)),
        Column::Series(Series::new("low".into(), lows)),
        Column::Series(Series::new("close".into(), closes)),
        Column::Series(Series::new("volume".into(), volumes)),
    ])
    .map_err(|e| CcxtError::ConfigError(format!("Failed to create DataFrame: {}", e)))?;

    let mut file = std::fs::File::create(path)
        .map_err(|e| CcxtError::ConfigError(format!("Failed to create file: {}", e)))?;

    ParquetWriter::new(&mut file)
        .finish(&mut df.clone())
        .map_err(|e| CcxtError::ConfigError(format!("Failed to write Parquet: {}", e)))?;

    Ok(())
}

/// Export trades data to Parquet format
fn export_trades_to_parquet(trades: &[Trade], path: &str) -> Result<()> {
    let ids: Vec<String> = trades.iter().map(|t| t.id.clone()).collect();
    let symbols: Vec<String> = trades.iter().map(|t| t.symbol.clone()).collect();
    let timestamps: Vec<i64> = trades.iter().map(|t| t.timestamp).collect();
    let sides: Vec<String> = trades.iter().map(|t| format!("{:?}", t.side)).collect();
    let prices: Vec<f64> = trades.iter().map(|t| t.price.to_string().parse().unwrap()).collect();
    let amounts: Vec<f64> = trades.iter().map(|t| t.amount.to_string().parse().unwrap()).collect();
    let costs: Vec<f64> = trades.iter().map(|t| t.cost.to_string().parse().unwrap()).collect();

    let df = DataFrame::new(vec![
        Column::Series(Series::new("id".into(), ids)),
        Column::Series(Series::new("symbol".into(), symbols)),
        Column::Series(Series::new("timestamp".into(), timestamps)),
        Column::Series(Series::new("side".into(), sides)),
        Column::Series(Series::new("price".into(), prices)),
        Column::Series(Series::new("amount".into(), amounts)),
        Column::Series(Series::new("cost".into(), costs)),
    ])
    .map_err(|e| CcxtError::ConfigError(format!("Failed to create DataFrame: {}", e)))?;

    let mut file = std::fs::File::create(path)
        .map_err(|e| CcxtError::ConfigError(format!("Failed to create file: {}", e)))?;

    ParquetWriter::new(&mut file)
        .finish(&mut df.clone())
        .map_err(|e| CcxtError::ConfigError(format!("Failed to write Parquet: {}", e)))?;

    Ok(())
}

/// Export tickers to Parquet format
fn export_tickers_to_parquet(tickers: &[Ticker], path: &str) -> Result<()> {
    let symbols: Vec<String> = tickers.iter().map(|t| t.symbol.clone()).collect();
    let timestamps: Vec<i64> = tickers.iter().map(|t| t.timestamp).collect();
    let lasts: Vec<Option<f64>> = tickers.iter().map(|t| t.last.map(|d| d.to_string().parse().unwrap())).collect();
    let bids: Vec<Option<f64>> = tickers.iter().map(|t| t.bid.map(|d| d.to_string().parse().unwrap())).collect();
    let asks: Vec<Option<f64>> = tickers.iter().map(|t| t.ask.map(|d| d.to_string().parse().unwrap())).collect();
    let highs: Vec<Option<f64>> = tickers.iter().map(|t| t.high.map(|d| d.to_string().parse().unwrap())).collect();
    let lows: Vec<Option<f64>> = tickers.iter().map(|t| t.low.map(|d| d.to_string().parse().unwrap())).collect();
    let volumes: Vec<Option<f64>> = tickers.iter().map(|t| t.base_volume.map(|d| d.to_string().parse().unwrap())).collect();
    let changes: Vec<Option<f64>> = tickers.iter().map(|t| t.percentage.map(|d| d.to_string().parse().unwrap())).collect();

    let df = DataFrame::new(vec![
        Column::Series(Series::new("symbol".into(), symbols)),
        Column::Series(Series::new("timestamp".into(), timestamps)),
        Column::Series(Series::new("last".into(), lasts)),
        Column::Series(Series::new("bid".into(), bids)),
        Column::Series(Series::new("ask".into(), asks)),
        Column::Series(Series::new("high".into(), highs)),
        Column::Series(Series::new("low".into(), lows)),
        Column::Series(Series::new("volume".into(), volumes)),
        Column::Series(Series::new("change_percent".into(), changes)),
    ])
    .map_err(|e| CcxtError::ConfigError(format!("Failed to create DataFrame: {}", e)))?;

    let mut file = std::fs::File::create(path)
        .map_err(|e| CcxtError::ConfigError(format!("Failed to create file: {}", e)))?;

    ParquetWriter::new(&mut file)
        .finish(&mut df.clone())
        .map_err(|e| CcxtError::ConfigError(format!("Failed to write Parquet: {}", e)))?;

    Ok(())
}

/// Export order book to Parquet format
fn export_orderbook_to_parquet(orderbook: &OrderBook, path: &str) -> Result<()> {
    // Export bids
    let bid_prices: Vec<f64> = orderbook.bids.iter().map(|(p, _)| p.to_string().parse().unwrap()).collect();
    let bid_amounts: Vec<f64> = orderbook.bids.iter().map(|(_, a)| a.to_string().parse().unwrap()).collect();
    let bid_sides: Vec<String> = vec!["bid".to_string(); orderbook.bids.len()];

    // Export asks
    let ask_prices: Vec<f64> = orderbook.asks.iter().map(|(p, _)| p.to_string().parse().unwrap()).collect();
    let ask_amounts: Vec<f64> = orderbook.asks.iter().map(|(_, a)| a.to_string().parse().unwrap()).collect();
    let ask_sides: Vec<String> = vec!["ask".to_string(); orderbook.asks.len()];

    // Combine bids and asks
    let mut prices = bid_prices;
    prices.extend(ask_prices);
    let mut amounts = bid_amounts;
    amounts.extend(ask_amounts);
    let mut sides = bid_sides;
    sides.extend(ask_sides);

    let df = DataFrame::new(vec![
        Column::Series(Series::new("side".into(), sides)),
        Column::Series(Series::new("price".into(), prices)),
        Column::Series(Series::new("amount".into(), amounts)),
    ])
    .map_err(|e| CcxtError::ConfigError(format!("Failed to create DataFrame: {}", e)))?;

    let mut file = std::fs::File::create(path)
        .map_err(|e| CcxtError::ConfigError(format!("Failed to create file: {}", e)))?;

    ParquetWriter::new(&mut file)
        .finish(&mut df.clone())
        .map_err(|e| CcxtError::ConfigError(format!("Failed to write Parquet: {}", e)))?;

    Ok(())
}
