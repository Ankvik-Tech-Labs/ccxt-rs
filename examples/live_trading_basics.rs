//! Live Trading Basics Example
//!
//! Demonstrates the fundamental trading lifecycle using Binance sandbox:
//! 1. Fetch balance → verify funds available
//! 2. Fetch ticker → get current market price
//! 3. Place limit buy order below market
//! 4. Fetch the order → verify it's open
//! 5. Place market sell order (if you have holdings)
//! 6. Cancel the limit order
//! 7. Verify final balance
//!
//! # Safety
//! - Defaults to sandbox mode (testnet) — no real funds at risk
//! - Uses small order sizes (0.001 BTC)
//!
//! # Running
//! ```bash
//! BINANCE_API_KEY=your_key BINANCE_SECRET=your_secret \
//!   cargo run --example live_trading_basics --features binance
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

    // Default to sandbox for safety
    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);

    println!("=== Live Trading Basics ===");
    println!("Mode: {}", if sandbox { "SANDBOX (testnet)" } else { "LIVE (real funds!)" });
    println!();

    let exchange = Binance::builder()
        .api_key(api_key)
        .secret(secret)
        .sandbox(sandbox)
        .build()?;

    // --- Step 1: Fetch Balance ---
    println!("--- Step 1: Fetch Balance ---");
    let balances = exchange.fetch_balance().await?;

    let usdt_balance = balances.balances.get("USDT");
    let btc_balance = balances.balances.get("BTC");

    if let Some(usdt) = usdt_balance {
        println!("USDT: free={}, used={}, total={}", usdt.free, usdt.used, usdt.total);
    } else {
        println!("USDT: no balance found");
    }
    if let Some(btc) = btc_balance {
        println!("BTC:  free={}, used={}, total={}", btc.free, btc.used, btc.total);
    }
    println!();

    // --- Step 2: Fetch Ticker ---
    println!("--- Step 2: Fetch Ticker (BTC/USDT) ---");
    let ticker = exchange.fetch_ticker("BTC/USDT").await?;
    let last_price = ticker.last.unwrap_or(dec!(0));
    println!("Last price:  ${}", last_price);
    println!("Bid:         ${}", ticker.bid.unwrap_or(dec!(0)));
    println!("Ask:         ${}", ticker.ask.unwrap_or(dec!(0)));
    println!("24h Volume:  {}", ticker.base_volume.unwrap_or(dec!(0)));
    println!();

    // --- Step 3: Place Limit Buy Order (below market) ---
    println!("--- Step 3: Place Limit Buy Order ---");
    // Place 20% below market to avoid fill
    let buy_price = (last_price * dec!(0.80)).round_dp(2);
    let amount = dec!(0.001); // Small amount

    println!("Placing limit buy: {} BTC @ ${}", amount, buy_price);

    let order = match exchange
        .create_order(
            "BTC/USDT",
            OrderType::Limit,
            OrderSide::Buy,
            amount,
            Some(buy_price),
            None,
        )
        .await
    {
        Ok(o) => {
            println!("Order created successfully!");
            println!("  ID:     {}", o.id);
            println!("  Status: {:?}", o.status);
            println!("  Side:   {:?}", o.side);
            println!("  Type:   {:?}", o.order_type);
            println!("  Price:  {:?}", o.price);
            println!("  Amount: {}", o.amount);
            println!();
            o
        }
        Err(CcxtError::InsufficientFunds(msg)) => {
            println!("Insufficient funds to place order: {}", msg);
            println!("Tip: Fund your sandbox account at the Binance testnet faucet");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // --- Step 4: Fetch the Order ---
    println!("--- Step 4: Fetch Order ---");
    let fetched = exchange.fetch_order(&order.id, Some("BTC/USDT")).await?;
    println!("Fetched order:");
    println!("  ID:        {}", fetched.id);
    println!("  Status:    {:?}", fetched.status);
    println!("  Filled:    {:?}", fetched.filled);
    println!("  Remaining: {:?}", fetched.remaining);
    println!();

    // --- Step 5: Check Open Orders ---
    println!("--- Step 5: Check Open Orders ---");
    let open_orders = exchange
        .fetch_open_orders(Some("BTC/USDT"), None, None)
        .await?;
    println!("Open orders for BTC/USDT: {}", open_orders.len());
    for o in &open_orders {
        println!("  {} {:?} {:?} {} @ {:?}", o.id, o.side, o.order_type, o.amount, o.price);
    }
    println!();

    // --- Step 6: Cancel the Order ---
    println!("--- Step 6: Cancel Order ---");
    match exchange.cancel_order(&order.id, Some("BTC/USDT")).await {
        Ok(cancelled) => {
            println!("Order cancelled successfully!");
            println!("  ID:     {}", cancelled.id);
            println!("  Status: {:?}", cancelled.status);
        }
        Err(CcxtError::OrderNotFound(msg)) => {
            println!("Order already gone (filled or expired): {}", msg);
        }
        Err(e) => return Err(e.into()),
    }
    println!();

    // --- Step 7: Verify Final Balance ---
    println!("--- Step 7: Final Balance ---");
    let final_balances = exchange.fetch_balance().await?;
    if let Some(usdt) = final_balances.balances.get("USDT") {
        println!("USDT: free={}, used={}, total={}", usdt.free, usdt.used, usdt.total);
    }
    println!();

    // --- Feature Support Matrix ---
    println!("--- Exchange Feature Support ---");
    let features = exchange.has();
    println!("create_order:       {}", features.create_order);
    println!("create_market_order:{}", features.create_market_order);
    println!("edit_order:         {}", features.edit_order);
    println!("cancel_order:       {}", features.cancel_order);
    println!("fetch_balance:      {}", features.fetch_balance);
    println!("fetch_positions:    {}", features.fetch_positions);
    println!("set_leverage:       {}", features.set_leverage);

    println!("\n=== Done! ===");
    Ok(())
}
