//! Multi-Exchange Trading Example
//!
//! Demonstrates concurrent trading operations across Binance, Bybit, and OKX:
//! 1. Concurrent balance fetch across 3 exchanges
//! 2. Find best price (lowest ask for buying)
//! 3. Place identical limit orders concurrently
//! 4. Cancel all concurrently
//! 5. Summary table with execution times
//!
//! # Running
//! ```bash
//! BINANCE_API_KEY=... BINANCE_SECRET=... \
//! BYBIT_API_KEY=... BYBIT_SECRET=... \
//! OKX_API_KEY=... OKX_SECRET=... OKX_PASSPHRASE=... \
//!   cargo run --example multi_exchange_trading --features "binance,bybit,okx"
//! ```

use ccxt::binance::Binance;
use ccxt::bybit::Bybit;
use ccxt::okx::Okx;
use ccxt::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Instant;

const SYMBOL: &str = "BTC/USDT";
const ORDER_AMOUNT: Decimal = dec!(0.001);

struct ExchangeResult {
    name: String,
    usdt_balance: Option<Decimal>,
    ask_price: Option<Decimal>,
    order_id: Option<String>,
    order_status: Option<OrderStatus>,
    cancel_ok: bool,
    duration_ms: u128,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("=== Multi-Exchange Trading ===");
    println!("Symbol: {}", SYMBOL);
    println!();

    // --- Build Exchanges (skip those without credentials) ---
    let binance = build_binance();
    let bybit = build_bybit();
    let okx = build_okx();

    let has_binance = binance.is_some();
    let has_bybit = bybit.is_some();
    let has_okx = okx.is_some();

    println!("Exchanges configured:");
    println!("  Binance: {}", if has_binance { "YES" } else { "SKIP (no credentials)" });
    println!("  Bybit:   {}", if has_bybit { "YES" } else { "SKIP (no credentials)" });
    println!("  OKX:     {}", if has_okx { "YES" } else { "SKIP (no credentials)" });
    println!();

    if !has_binance && !has_bybit && !has_okx {
        println!("No exchange credentials found. Set env vars and retry.");
        return Ok(());
    }

    // --- Step 1: Concurrent Balance Fetch ---
    println!("--- Step 1: Concurrent Balance Fetch ---");
    let start = Instant::now();

    let (bal_binance, bal_bybit, bal_okx) = tokio::join!(
        async {
            match &binance {
                Some(e) => e.fetch_balance().await.ok(),
                None => None,
            }
        },
        async {
            match &bybit {
                Some(e) => e.fetch_balance().await.ok(),
                None => None,
            }
        },
        async {
            match &okx {
                Some(e) => e.fetch_balance().await.ok(),
                None => None,
            }
        },
    );

    let balance_duration = start.elapsed().as_millis();
    println!("All balances fetched in {}ms (concurrent)", balance_duration);

    fn get_usdt(bal: &Option<Balances>) -> Option<Decimal> {
        bal.as_ref()
            .and_then(|b| b.balances.get("USDT"))
            .map(|b| b.free)
    }

    println!("  Binance USDT: {:?}", get_usdt(&bal_binance));
    println!("  Bybit USDT:   {:?}", get_usdt(&bal_bybit));
    println!("  OKX USDT:     {:?}", get_usdt(&bal_okx));
    println!();

    // --- Step 2: Concurrent Ticker Fetch → Find Best Price ---
    println!("--- Step 2: Find Best Price (Lowest Ask) ---");
    let start = Instant::now();

    let (ticker_binance, ticker_bybit, ticker_okx) = tokio::join!(
        async {
            match &binance {
                Some(e) => e.fetch_ticker(SYMBOL).await.ok(),
                None => None,
            }
        },
        async {
            match &bybit {
                Some(e) => e.fetch_ticker(SYMBOL).await.ok(),
                None => None,
            }
        },
        async {
            match &okx {
                Some(e) => e.fetch_ticker(SYMBOL).await.ok(),
                None => None,
            }
        },
    );

    let ticker_duration = start.elapsed().as_millis();
    println!("All tickers fetched in {}ms (concurrent)", ticker_duration);

    let asks = [
        ("Binance", ticker_binance.as_ref().and_then(|t| t.ask)),
        ("Bybit", ticker_bybit.as_ref().and_then(|t| t.ask)),
        ("OKX", ticker_okx.as_ref().and_then(|t| t.ask)),
    ];

    for (name, ask) in &asks {
        println!("  {} ask: {:?}", name, ask);
    }

    let best = asks
        .iter()
        .filter_map(|(name, ask)| ask.map(|a| (*name, a)))
        .min_by_key(|(_, a)| *a);

    if let Some((name, price)) = best {
        println!("Best ask (lowest): {} @ ${}", name, price);
    }
    println!();

    // --- Step 3: Place Limit Orders Concurrently ---
    println!("--- Step 3: Place Limit Orders (20% below market) ---");
    let reference_price = asks
        .iter()
        .filter_map(|(_, ask)| *ask)
        .min()
        .unwrap_or(dec!(50000));
    let order_price = (reference_price * dec!(0.80)).round_dp(2);
    println!("Order: BUY {} {} @ ${}", ORDER_AMOUNT, SYMBOL, order_price);

    let start = Instant::now();

    let (ord_binance, ord_bybit, ord_okx) = tokio::join!(
        async {
            match &binance {
                Some(e) => e
                    .create_order(SYMBOL, OrderType::Limit, OrderSide::Buy, ORDER_AMOUNT, Some(order_price), None)
                    .await
                    .ok(),
                None => None,
            }
        },
        async {
            match &bybit {
                Some(e) => e
                    .create_order(SYMBOL, OrderType::Limit, OrderSide::Buy, ORDER_AMOUNT, Some(order_price), None)
                    .await
                    .ok(),
                None => None,
            }
        },
        async {
            match &okx {
                Some(e) => e
                    .create_order(SYMBOL, OrderType::Limit, OrderSide::Buy, ORDER_AMOUNT, Some(order_price), None)
                    .await
                    .ok(),
                None => None,
            }
        },
    );

    let order_duration = start.elapsed().as_millis();
    println!("All orders placed in {}ms (concurrent)", order_duration);

    fn show_order(name: &str, order: &Option<Order>) {
        match order {
            Some(o) => println!("  {}: id={}, status={:?}", name, o.id, o.status),
            None => println!("  {}: FAILED or SKIPPED", name),
        }
    }

    show_order("Binance", &ord_binance);
    show_order("Bybit", &ord_bybit);
    show_order("OKX", &ord_okx);
    println!();

    // --- Step 4: Cancel All Concurrently ---
    println!("--- Step 4: Cancel All Orders ---");
    let start = Instant::now();

    let (cancel_binance, cancel_bybit, cancel_okx) = tokio::join!(
        async {
            match (&binance, &ord_binance) {
                (Some(e), Some(o)) => e.cancel_order(&o.id, Some(SYMBOL)).await.is_ok(),
                _ => false,
            }
        },
        async {
            match (&bybit, &ord_bybit) {
                (Some(e), Some(o)) => e.cancel_order(&o.id, Some(SYMBOL)).await.is_ok(),
                _ => false,
            }
        },
        async {
            match (&okx, &ord_okx) {
                (Some(e), Some(o)) => e.cancel_order(&o.id, Some(SYMBOL)).await.is_ok(),
                _ => false,
            }
        },
    );

    let cancel_duration = start.elapsed().as_millis();
    println!("All cancels done in {}ms (concurrent)", cancel_duration);
    println!("  Binance: {}", if cancel_binance { "OK" } else { "SKIP/FAIL" });
    println!("  Bybit:   {}", if cancel_bybit { "OK" } else { "SKIP/FAIL" });
    println!("  OKX:     {}", if cancel_okx { "OK" } else { "SKIP/FAIL" });
    println!();

    // --- Summary ---
    println!("=== Summary ===");
    println!("┌──────────┬──────────────┬──────────┬──────────┬──────────┐");
    println!("│ Exchange │ USDT Balance │ Ask      │ Order    │ Cancel   │");
    println!("├──────────┼──────────────┼──────────┼──────────┼──────────┤");
    print_row("Binance", get_usdt(&bal_binance), asks[0].1, &ord_binance, cancel_binance);
    print_row("Bybit", get_usdt(&bal_bybit), asks[1].1, &ord_bybit, cancel_bybit);
    print_row("OKX", get_usdt(&bal_okx), asks[2].1, &ord_okx, cancel_okx);
    println!("└──────────┴──────────────┴──────────┴──────────┴──────────┘");
    println!();
    println!("Timing:");
    println!("  Balance fetch: {}ms", balance_duration);
    println!("  Ticker fetch:  {}ms", ticker_duration);
    println!("  Order place:   {}ms", order_duration);
    println!("  Order cancel:  {}ms", cancel_duration);
    println!("  Total:         {}ms", balance_duration + ticker_duration + order_duration + cancel_duration);

    println!("\n=== Done! ===");
    Ok(())
}

fn print_row(
    name: &str,
    balance: Option<Decimal>,
    ask: Option<Decimal>,
    order: &Option<Order>,
    cancelled: bool,
) {
    let bal_str = balance
        .map(|b| format!("{:.2}", b))
        .unwrap_or_else(|| "N/A".to_string());
    let ask_str = ask
        .map(|a| format!("{:.2}", a))
        .unwrap_or_else(|| "N/A".to_string());
    let ord_str = order
        .as_ref()
        .map(|o| format!("{:?}", o.status))
        .unwrap_or_else(|| "N/A".to_string());
    let cancel_str = if order.is_some() {
        if cancelled { "OK" } else { "FAIL" }
    } else {
        "N/A"
    };

    println!(
        "│ {:<8} │ {:>12} │ {:>8} │ {:>8} │ {:>8} │",
        name, bal_str, ask_str, ord_str, cancel_str
    );
}

fn build_binance() -> Option<Binance> {
    let api_key = std::env::var("BINANCE_API_KEY").ok()?;
    let secret = std::env::var("BINANCE_SECRET").ok()?;
    let sandbox = std::env::var("BINANCE_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);
    Binance::builder()
        .api_key(api_key)
        .secret(secret)
        .sandbox(sandbox)
        .build()
        .ok()
}

fn build_bybit() -> Option<Bybit> {
    let api_key = std::env::var("BYBIT_API_KEY").ok()?;
    let secret = std::env::var("BYBIT_SECRET").ok()?;
    let sandbox = std::env::var("BYBIT_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);
    Bybit::builder()
        .api_key(api_key)
        .secret(secret)
        .sandbox(sandbox)
        .build()
        .ok()
}

fn build_okx() -> Option<Okx> {
    let api_key = std::env::var("OKX_API_KEY").ok()?;
    let secret = std::env::var("OKX_SECRET").ok()?;
    let passphrase = std::env::var("OKX_PASSPHRASE").ok()?;
    let sandbox = std::env::var("OKX_SANDBOX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);
    Okx::builder()
        .api_key(api_key)
        .secret(secret)
        .passphrase(passphrase)
        .sandbox(sandbox)
        .build()
        .ok()
}
