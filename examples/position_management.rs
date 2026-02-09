//! Position Management Example
//!
//! Demonstrates derivatives/futures position lifecycle on Binance:
//! 1. Set margin mode (cross or isolated)
//! 2. Set leverage
//! 3. Open a long position via market order
//! 4. Fetch positions — display entry price, PnL, leverage, liquidation price
//! 5. Close position with reduce-only market order
//! 6. Verify position is closed
//!
//! # Safety
//! - Defaults to sandbox mode (Binance futures testnet)
//! - Uses tiny position sizes (0.001 BTC)
//!
//! # Running
//! ```bash
//! BINANCE_API_KEY=your_key BINANCE_SECRET=your_secret \
//!   cargo run --example position_management --features binance
//! ```

use ccxt::binance::Binance;
use ccxt::prelude::*;
use rust_decimal_macros::dec;
use std::collections::HashMap;

const SYMBOL: &str = "BTC/USDT";
const POSITION_SIZE: rust_decimal::Decimal = dec!(0.001);
const LEVERAGE: u32 = 10;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("BINANCE_API_KEY")
        .expect("Set BINANCE_API_KEY env var");
    let secret = std::env::var("BINANCE_SECRET")
        .expect("Set BINANCE_SECRET env var");

    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);

    println!("=== Position Management ===");
    println!("Mode:     {}", if sandbox { "SANDBOX" } else { "LIVE" });
    println!("Symbol:   {}", SYMBOL);
    println!("Size:     {} BTC", POSITION_SIZE);
    println!("Leverage: {}x", LEVERAGE);
    println!();

    let exchange = Binance::builder()
        .api_key(api_key)
        .secret(secret)
        .sandbox(sandbox)
        .build()?;

    // --- Step 1: Set Margin Mode ---
    println!("--- Step 1: Set Margin Mode (Cross) ---");
    match exchange.set_margin_mode(MarginMode::Cross, SYMBOL).await {
        Ok(()) => println!("Margin mode set to Cross"),
        Err(CcxtError::MarginModeAlreadySet(msg)) => {
            println!("Margin mode already Cross: {}", msg);
        }
        Err(e) => {
            println!("Warning: Could not set margin mode: {}", e);
            println!("Continuing anyway...");
        }
    }
    println!();

    // --- Step 2: Set Leverage ---
    println!("--- Step 2: Set Leverage ({}x) ---", LEVERAGE);
    match exchange.set_leverage(LEVERAGE, SYMBOL).await {
        Ok(()) => println!("Leverage set to {}x", LEVERAGE),
        Err(e) => {
            println!("Warning: Could not set leverage: {}", e);
            println!("Continuing with current leverage...");
        }
    }
    println!();

    // --- Step 3: Open Long Position ---
    println!("--- Step 3: Open Long Position ---");
    println!("Opening market buy for {} {}", POSITION_SIZE, SYMBOL);

    let open_order = match exchange
        .create_order(
            SYMBOL,
            OrderType::Market,
            OrderSide::Buy,
            POSITION_SIZE,
            None,
            None,
        )
        .await
    {
        Ok(o) => {
            println!("Market order filled!");
            println!("  ID:      {}", o.id);
            println!("  Status:  {:?}", o.status);
            println!("  Filled:  {:?}", o.filled);
            println!("  Average: {:?}", o.average);
            println!("  Cost:    {:?}", o.cost);
            o
        }
        Err(CcxtError::InsufficientFunds(msg)) => {
            println!("Insufficient funds: {}", msg);
            println!("Tip: Fund your Binance futures testnet account");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    println!();

    // --- Step 4: Fetch Positions ---
    println!("--- Step 4: Fetch Positions ---");
    // Short delay for position to register
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let positions = exchange.fetch_positions(None).await?;
    println!("Active positions: {}", positions.len());
    for pos in &positions {
        println!("  Symbol:           {}", pos.symbol);
        println!("  Side:             {:?}", pos.side);
        println!("  Contracts:        {}", pos.contracts);
        println!("  Entry Price:      {:?}", pos.entry_price);
        println!("  Mark Price:       {:?}", pos.mark_price);
        println!("  Leverage:         {:?}", pos.leverage);
        println!("  Unrealized PnL:   {:?}", pos.unrealized_pnl);
        println!("  Liquidation:      {:?}", pos.liquidation_price);
        println!("  Margin Mode:      {:?}", pos.margin_mode);
        println!("  Notional:         {:?}", pos.notional);
        println!("  ---");
    }

    // Check if our position exists
    let our_position = positions.iter().find(|p| p.symbol.contains("BTC"));
    if our_position.is_none() {
        println!("Warning: Position not found in fetch_positions response");
        println!("It may have been immediately liquidated or the symbol format differs");
    }
    println!();

    // --- Step 5: Close Position ---
    println!("--- Step 5: Close Position (Reduce-Only Market Sell) ---");
    let mut close_params = HashMap::new();
    close_params.insert(
        "reduceOnly".to_string(),
        serde_json::Value::String("true".to_string()),
    );

    match exchange
        .create_order(
            SYMBOL,
            OrderType::Market,
            OrderSide::Sell,
            POSITION_SIZE,
            None,
            Some(&close_params),
        )
        .await
    {
        Ok(close_order) => {
            println!("Position closed!");
            println!("  ID:      {}", close_order.id);
            println!("  Status:  {:?}", close_order.status);
            println!("  Filled:  {:?}", close_order.filled);
            println!("  Average: {:?}", close_order.average);
        }
        Err(CcxtError::InvalidOrder(msg)) if msg.contains("reduce") => {
            println!("No position to reduce: {}", msg);
        }
        Err(e) => {
            println!("Error closing position: {}", e);
        }
    }
    println!();

    // --- Step 6: Verify Closed ---
    println!("--- Step 6: Verify Position Closed ---");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let final_positions = exchange.fetch_positions(None).await?;
    let btc_position = final_positions.iter().find(|p| p.symbol.contains("BTC"));
    match btc_position {
        Some(pos) => {
            println!("BTC position still exists:");
            println!("  Contracts: {}", pos.contracts);
            println!("  Side: {:?}", pos.side);
        }
        None => {
            println!("BTC position fully closed (no active positions)");
        }
    }

    println!("\n=== Done! ===");
    Ok(())
}
