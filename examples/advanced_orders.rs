//! Advanced Orders Example
//!
//! Demonstrates advanced order types using the `params` HashMap approach:
//! 1. Stop-Loss Limit order (trigger price + limit price)
//! 2. Take-Profit Limit order
//! 3. Post-Only order (maker only, rejected if would take)
//! 4. Reduce-Only order (only reduces an existing position)
//!
//! Each order: create → verify fields → cancel → cleanup.
//!
//! # Exchanges
//! Uses Binance for most examples; notes Bybit differences where applicable.
//!
//! # Running
//! ```bash
//! BINANCE_API_KEY=key BINANCE_SECRET=secret \
//!   cargo run --example advanced_orders --features "binance,bybit"
//! ```

use ccxt::binance::Binance;
use ccxt::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("BINANCE_API_KEY")
        .expect("Set BINANCE_API_KEY env var");
    let secret = std::env::var("BINANCE_SECRET")
        .expect("Set BINANCE_SECRET env var");

    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);

    println!("=== Advanced Orders ===");
    println!("Mode: {}", if sandbox { "SANDBOX" } else { "LIVE" });
    println!();

    let exchange = Binance::builder()
        .api_key(api_key)
        .secret(secret)
        .sandbox(sandbox)
        .build()?;

    // Print feature support matrix
    println!("--- Feature Support Matrix (Binance) ---");
    let features = exchange.has();
    println!("create_stop_order:       {}", features.create_stop_order);
    println!("create_stop_limit_order: {}", features.create_stop_limit_order);
    println!("create_post_only_order:  {}", features.create_post_only_order);
    println!("create_reduce_only_order:{}", features.create_reduce_only_order);
    println!("edit_order:              {}", features.edit_order);
    println!();

    // Get current price for reference
    let ticker = exchange.fetch_ticker("BTC/USDT").await?;
    let last_price = ticker.last.unwrap_or(dec!(50000));
    println!("Current BTC/USDT: ${}", last_price);
    println!();

    // =========================================================================
    // 1. Stop-Loss Limit Order
    // =========================================================================
    println!("--- 1. Stop-Loss Limit Order ---");
    println!("A stop-loss triggers when price drops to stopPrice,");
    println!("then places a limit order at the specified price.");

    // Stop trigger 15% below market, limit price 16% below
    let stop_trigger = (last_price * dec!(0.85)).round_dp(2);
    let stop_limit_price = (last_price * dec!(0.84)).round_dp(2);

    let mut stop_params = HashMap::new();
    stop_params.insert(
        "type".to_string(),
        serde_json::Value::String("STOP_LOSS_LIMIT".to_string()),
    );
    stop_params.insert(
        "stopPrice".to_string(),
        serde_json::Value::String(stop_trigger.to_string()),
    );
    stop_params.insert(
        "timeInForce".to_string(),
        serde_json::Value::String("GTC".to_string()),
    );

    println!("Placing: SELL 0.001 BTC, stop={}, limit={}", stop_trigger, stop_limit_price);

    match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Sell,
            dec!(0.001),
            Some(stop_limit_price),
            Some(&stop_params),
        )
        .await
    {
        Ok(order) => {
            println!("Stop-loss order created: id={}", order.id);
            println!("  Status:     {:?}", order.status);
            println!("  Stop Price: {:?}", order.stop_price);
            println!("  Price:      {:?}", order.price);

            // Cancel it
            match exchange.cancel_order(&order.id, Some("BTC/USDT")).await {
                Ok(_) => println!("  Cancelled successfully"),
                Err(e) => println!("  Cancel error: {}", e),
            }
        }
        Err(CcxtError::InvalidOrder(msg)) => {
            println!("Invalid order (exchange may require open position): {}", msg);
        }
        Err(CcxtError::InsufficientFunds(msg)) => {
            println!("Insufficient funds: {}", msg);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    println!();

    // =========================================================================
    // 2. Take-Profit Limit Order
    // =========================================================================
    println!("--- 2. Take-Profit Limit Order ---");
    println!("A take-profit triggers when price rises to stopPrice,");
    println!("then places a limit order to lock in profits.");

    let tp_trigger = (last_price * dec!(1.15)).round_dp(2);
    let tp_limit_price = (last_price * dec!(1.14)).round_dp(2);

    let mut tp_params = HashMap::new();
    tp_params.insert(
        "type".to_string(),
        serde_json::Value::String("TAKE_PROFIT_LIMIT".to_string()),
    );
    tp_params.insert(
        "stopPrice".to_string(),
        serde_json::Value::String(tp_trigger.to_string()),
    );
    tp_params.insert(
        "timeInForce".to_string(),
        serde_json::Value::String("GTC".to_string()),
    );

    println!("Placing: SELL 0.001 BTC, trigger={}, limit={}", tp_trigger, tp_limit_price);

    match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Sell,
            dec!(0.001),
            Some(tp_limit_price),
            Some(&tp_params),
        )
        .await
    {
        Ok(order) => {
            println!("Take-profit order created: id={}", order.id);
            println!("  Status:     {:?}", order.status);
            println!("  Stop Price: {:?}", order.stop_price);

            match exchange.cancel_order(&order.id, Some("BTC/USDT")).await {
                Ok(_) => println!("  Cancelled successfully"),
                Err(e) => println!("  Cancel error: {}", e),
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();

    // =========================================================================
    // 3. Post-Only (Maker) Order
    // =========================================================================
    println!("--- 3. Post-Only (Maker) Order ---");
    println!("Post-only orders are rejected if they would immediately match.");
    println!("This guarantees you pay the maker fee, not the taker fee.");

    // Place a buy well below market — should succeed as maker
    let post_only_price = (last_price * dec!(0.80)).round_dp(2);

    let mut post_params = HashMap::new();
    post_params.insert(
        "timeInForce".to_string(),
        serde_json::Value::String("GTX".to_string()), // GTX = post-only on Binance
    );

    println!("Placing: BUY 0.001 BTC @ {} (post-only/GTX)", post_only_price);

    match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Buy,
            dec!(0.001),
            Some(post_only_price),
            Some(&post_params),
        )
        .await
    {
        Ok(order) => {
            println!("Post-only order created: id={}", order.id);
            println!("  Status:    {:?}", order.status);
            println!("  Post-Only: {:?}", order.post_only);

            match exchange.cancel_order(&order.id, Some("BTC/USDT")).await {
                Ok(_) => println!("  Cancelled successfully"),
                Err(e) => println!("  Cancel error: {}", e),
            }
        }
        Err(CcxtError::OrderImmediatelyFillable(msg)) => {
            println!("Order rejected (would fill immediately): {}", msg);
            println!("This is expected if price is at or above market!");
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();

    // Now test with a price that WOULD fill (to show rejection)
    println!("Testing post-only at market price (expect rejection):");
    let aggressive_price = (last_price * dec!(1.05)).round_dp(2);
    println!("Placing: BUY 0.001 BTC @ {} (post-only, should reject)", aggressive_price);

    match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Buy,
            dec!(0.001),
            Some(aggressive_price),
            Some(&post_params),
        )
        .await
    {
        Ok(order) => {
            println!("Unexpected success! Order: {}", order.id);
            let _ = exchange.cancel_order(&order.id, Some("BTC/USDT")).await;
        }
        Err(CcxtError::OrderImmediatelyFillable(msg)) => {
            println!("Correctly rejected: {}", msg);
        }
        Err(e) => println!("Other error (may also indicate rejection): {}", e),
    }
    println!();

    // =========================================================================
    // 4. Reduce-Only Order (futures)
    // =========================================================================
    println!("--- 4. Reduce-Only Order ---");
    println!("Reduce-only orders can only decrease an existing position.");
    println!("If no position exists, the order is rejected.");
    println!();

    let mut reduce_params = HashMap::new();
    reduce_params.insert(
        "reduceOnly".to_string(),
        serde_json::Value::String("true".to_string()),
    );

    println!("Attempting reduce-only sell without a position (expect rejection):");
    match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Market,
            OrderSide::Sell,
            dec!(0.001),
            None,
            Some(&reduce_params),
        )
        .await
    {
        Ok(order) => {
            println!("Order created (exchange may allow on spot): id={}", order.id);
        }
        Err(CcxtError::InvalidOrder(msg)) => {
            println!("Correctly rejected (no position to reduce): {}", msg);
        }
        Err(e) => println!("Error: {}", e),
    }

    // =========================================================================
    // Summary
    // =========================================================================
    println!();
    println!("=== Order Type Summary ===");
    println!("┌─────────────────────────┬───────────────────────────────────┐");
    println!("│ Order Type              │ Params Key                        │");
    println!("├─────────────────────────┼───────────────────────────────────┤");
    println!("│ Stop-Loss Limit         │ type=STOP_LOSS_LIMIT, stopPrice  │");
    println!("│ Take-Profit Limit       │ type=TAKE_PROFIT_LIMIT, stopPrice│");
    println!("│ Post-Only (Maker)       │ timeInForce=GTX                  │");
    println!("│ Reduce-Only             │ reduceOnly=true                  │");
    println!("│ IOC (Immediate/Cancel)  │ timeInForce=IOC                  │");
    println!("│ FOK (Fill or Kill)      │ timeInForce=FOK                  │");
    println!("└─────────────────────────┴───────────────────────────────────┘");

    println!("\n=== Done! ===");
    Ok(())
}
