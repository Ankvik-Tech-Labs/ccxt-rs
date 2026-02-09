//! WebSocket Trading Example
//!
//! Demonstrates combining REST and WebSocket APIs:
//! 1. Subscribe to ticker stream (public) + order stream (private)
//! 2. Place a limit order via REST
//! 3. Observe order update via WebSocket
//! 4. Cancel order via REST
//! 5. Observe cancellation via WebSocket
//!
//! # Running
//! ```bash
//! BINANCE_API_KEY=key BINANCE_SECRET=secret \
//!   cargo run --example websocket_trading --features binance
//! ```

use ccxt::base::ws::{ExchangeWs, WsConfig};
use ccxt::binance::ws::BinanceWs;
use ccxt::binance::Binance;
use ccxt::prelude::*;
use rust_decimal_macros::dec;
use std::time::Duration;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("ccxt=info")
        .init();

    let api_key = std::env::var("BINANCE_API_KEY")
        .expect("Set BINANCE_API_KEY env var");
    let secret = std::env::var("BINANCE_SECRET")
        .expect("Set BINANCE_SECRET env var");

    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);

    println!("=== WebSocket Trading ===");
    println!("Mode: {}", if sandbox { "SANDBOX" } else { "LIVE" });
    println!();

    // --- REST client for trading ---
    let exchange = Binance::builder()
        .api_key(&api_key)
        .secret(&secret)
        .sandbox(sandbox)
        .build()?;

    // --- WebSocket client for streaming ---
    let config = WsConfig::default();
    let ws = BinanceWs::new(sandbox, config)
        .with_credentials(api_key.clone(), secret.clone());

    // --- Subscribe to public ticker stream ---
    println!("Subscribing to BTC/USDT ticker...");
    let mut ticker_stream = ws.watch_ticker("BTC/USDT").await?;

    // Wait for first ticker to confirm connection
    println!("Waiting for first ticker update...");
    if let Some(ticker) = tokio::time::timeout(Duration::from_secs(10), ticker_stream.next()).await? {
        println!("Connected! BTC/USDT last: ${}", ticker.last.unwrap_or_default());
    } else {
        println!("No ticker received within 10s, continuing...");
    }
    println!();

    // --- Place a limit order via REST ---
    let ticker = exchange.fetch_ticker("BTC/USDT").await?;
    let last_price = ticker.last.unwrap_or(dec!(50000));
    let order_price = (last_price * dec!(0.80)).round_dp(2);

    println!("Placing limit buy: 0.001 BTC @ ${}", order_price);

    let order = match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Buy,
            dec!(0.001),
            Some(order_price),
            None,
        )
        .await
    {
        Ok(o) => {
            println!("Order placed: id={}, status={:?}", o.id, o.status);
            o
        }
        Err(CcxtError::InsufficientFunds(msg)) => {
            println!("Insufficient funds: {}", msg);
            ws.close().await?;
            return Ok(());
        }
        Err(e) => {
            ws.close().await?;
            return Err(e.into());
        }
    };
    println!();

    // --- Listen for a few more ticker updates while order is open ---
    println!("Listening for ticker updates (3 seconds)...");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut update_count = 0;

    loop {
        tokio::select! {
            ticker = ticker_stream.next() => {
                if let Some(t) = ticker {
                    update_count += 1;
                    if update_count <= 3 {
                        println!("  Ticker: ${}", t.last.unwrap_or_default());
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }
    println!("Received {} ticker updates while order was open", update_count);
    println!();

    // --- Cancel the order via REST ---
    println!("Cancelling order {}...", order.id);
    match exchange.cancel_order(&order.id, Some("BTC/USDT")).await {
        Ok(cancelled) => {
            println!("Order cancelled: status={:?}", cancelled.status);
        }
        Err(e) => {
            println!("Cancel error: {}", e);
        }
    }
    println!();

    // --- Cleanup ---
    println!("Closing WebSocket connections...");
    ws.close().await?;

    println!("=== Done! ===");
    Ok(())
}
