//! Cross-Validation Runner
//!
//! CLI tool that instantiates a ccxt-rs exchange, calls a method, and prints
//! JSON to stdout for comparison with CCXT Python output.
//!
//! Usage:
//!   cargo run --all-features --example cross_validate_runner -- \
//!     binance fetch_ticker BTC/USDT
//!
//!   cargo run --all-features --example cross_validate_runner -- \
//!     okx fetch_order_book BTC/USDT --limit 5

use ccxt::prelude::*;
use serde_json;
use std::env;

#[cfg(feature = "binance")]
use ccxt::binance::Binance;
#[cfg(feature = "bybit")]
use ccxt::bybit::Bybit;
#[cfg(feature = "okx")]
use ccxt::okx::Okx;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: cross_validate_runner <exchange> <method> [args...] [--limit N]");
        eprintln!("  exchange: binance, bybit, okx");
        eprintln!("  method: fetch_ticker, fetch_order_book, fetch_trades, fetch_ohlcv, fetch_markets");
        std::process::exit(1);
    }

    let exchange_id = &args[1];
    let method = &args[2];
    let rest_args: Vec<&str> = args[3..].iter().map(|s| s.as_str()).collect();

    // Parse optional --limit flag
    let limit = rest_args
        .iter()
        .position(|&a| a == "--limit")
        .and_then(|i| rest_args.get(i + 1))
        .and_then(|s| s.parse::<u32>().ok());

    // Get positional args (exclude --limit and its value)
    let positional: Vec<&str> = rest_args
        .iter()
        .enumerate()
        .filter(|(i, &a)| a != "--limit" && !rest_args.get(i.wrapping_sub(1)).is_some_and(|&prev| prev == "--limit"))
        .map(|(_, &a)| a)
        .collect();

    let symbol = positional.first().copied().unwrap_or("BTC/USDT");

    match exchange_id.as_str() {
        #[cfg(feature = "binance")]
        "binance" => {
            let exchange = Binance::builder().build()?;
            run_method(&exchange, method, symbol, limit, &positional).await?;
        }
        #[cfg(feature = "bybit")]
        "bybit" => {
            let exchange = Bybit::builder().build()?;
            run_method(&exchange, method, symbol, limit, &positional).await?;
        }
        #[cfg(feature = "okx")]
        "okx" => {
            let exchange = Okx::builder().build()?;
            run_method(&exchange, method, symbol, limit, &positional).await?;
        }
        _ => {
            eprintln!("Unsupported exchange: {}", exchange_id);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run_method(
    exchange: &dyn Exchange,
    method: &str,
    symbol: &str,
    limit: Option<u32>,
    positional: &[&str],
) -> Result<()> {
    match method {
        "fetch_ticker" => {
            let ticker = exchange.fetch_ticker(symbol).await?;
            println!("{}", serde_json::to_string_pretty(&ticker).unwrap());
        }
        "fetch_tickers" => {
            let symbols: Vec<&str> = if positional.len() > 1 {
                positional.to_vec()
            } else {
                vec!["BTC/USDT", "ETH/USDT"]
            };
            let tickers = exchange.fetch_tickers(Some(&symbols)).await?;
            println!("{}", serde_json::to_string_pretty(&tickers).unwrap());
        }
        "fetch_order_book" => {
            let ob = exchange.fetch_order_book(symbol, limit).await?;
            println!("{}", serde_json::to_string_pretty(&ob).unwrap());
        }
        "fetch_trades" => {
            let trades = exchange.fetch_trades(symbol, None, limit).await?;
            println!("{}", serde_json::to_string_pretty(&trades).unwrap());
        }
        "fetch_ohlcv" => {
            let timeframe_str = positional.get(1).copied().unwrap_or("1h");
            let timeframe = match timeframe_str {
                "1m" => Timeframe::OneMinute,
                "5m" => Timeframe::FiveMinutes,
                "15m" => Timeframe::FifteenMinutes,
                "1h" => Timeframe::OneHour,
                "4h" => Timeframe::FourHours,
                "1d" => Timeframe::OneDay,
                _ => Timeframe::OneHour,
            };
            let ohlcv = exchange.fetch_ohlcv(symbol, timeframe, None, limit).await?;
            println!("{}", serde_json::to_string_pretty(&ohlcv).unwrap());
        }
        "fetch_markets" => {
            let markets = exchange.fetch_markets().await?;
            // Only output first 5 to keep output manageable
            let subset: Vec<_> = markets.into_iter().take(5).collect();
            println!("{}", serde_json::to_string_pretty(&subset).unwrap());
        }
        _ => {
            eprintln!("Unsupported method: {}", method);
            std::process::exit(1);
        }
    }
    Ok(())
}
