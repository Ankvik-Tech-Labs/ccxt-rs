//! WebSocket Ticker Example
//!
//! Subscribe to real-time BTC/USDT ticker updates from Binance.
//! Prints price updates as they arrive. Graceful shutdown on Ctrl+C.
//!
//! # Running
//! ```bash
//! cargo run --example websocket_ticker --features binance
//! ```

use ccxt::base::ws::{ExchangeWs, WsConfig};
use ccxt::binance::ws::BinanceWs;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter("ccxt=info")
        .init();

    println!("=== WebSocket Ticker Stream ===");
    println!("Subscribing to BTC/USDT ticker from Binance...");
    println!("Press Ctrl+C to stop\n");

    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(false); // Default to mainnet for public data

    let config = WsConfig::default();
    let ws = BinanceWs::new(sandbox, config);

    let mut ticker_stream = ws.watch_ticker("BTC/USDT").await?;

    let mut count = 0u64;

    // Set up Ctrl+C handler
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx.send(());
    });

    loop {
        tokio::select! {
            ticker = ticker_stream.next() => {
                match ticker {
                    Some(t) => {
                        count += 1;
                        println!(
                            "[{}] {} | Last: ${} | Bid: ${} | Ask: ${} | Vol: {}",
                            count,
                            t.symbol,
                            t.last.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
                            t.bid.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
                            t.ask.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
                            t.base_volume.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
                        );
                    }
                    None => {
                        println!("Stream ended");
                        break;
                    }
                }
            }
            _ = &mut shutdown_rx => {
                println!("\nShutting down...");
                break;
            }
        }
    }

    ws.close().await?;
    println!("Received {} ticker updates", count);
    println!("=== Done! ===");
    Ok(())
}
