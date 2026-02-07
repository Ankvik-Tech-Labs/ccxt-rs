# ccxt-rs Examples

This directory contains practical examples demonstrating how to use ccxt-rs for cryptocurrency trading and market data retrieval.

## 📚 Quick Start

All examples require the `binance` feature flag:

```bash
cargo run --example <example_name> --features binance
```

---

## 🎯 Examples by Category

### 🟢 Beginner Examples

| Example | Description | Run Command |
|---------|-------------|-------------|
| **simple_ohlcv.rs** | Basic candlestick data fetching for BTC/USDT | `cargo run --example simple_ohlcv --features binance` |
| **binance_public_api.rs** | All public API methods (ticker, orderbook, trades, etc.) | `cargo run --example binance_public_api --features binance` |
| **verify_binance_data.rs** | Simple data verification example | `cargo run --example verify_binance_data` |

### 🔵 Intermediate Examples

| Example | Description | Run Command |
|---------|-------------|-------------|
| **fetch_ohlcv.rs** | **Comprehensive OHLCV tutorial** with 6 different patterns | `cargo run --example fetch_ohlcv --features binance` |
| **ccxt_style_usage.rs** | Shows CCXT Python API compatibility | `cargo run --example ccxt_style_usage --features binance` |

### 🟣 Advanced Examples

| Example | Description | Run Command |
|---------|-------------|-------------|
| **binance_export_parquet.rs** | Export market data to Parquet files for analysis | `cargo run --example binance_export_parquet --features binance` |
| **read_parquet.rs** | Read and analyze exported Parquet files | `cargo run --example read_parquet` |

---

## 📖 Detailed Example Descriptions

### 1. simple_ohlcv.rs — Basic OHLCV Fetching
**Perfect for:** First-time users, quick prototyping

**What it demonstrates:**
- Creating a Binance exchange instance
- Fetching hourly candlestick data for BTC/USDT
- Printing candles in a formatted table
- Calculating basic statistics (price change, high/low, volume)

**Output:**
```
Timestamp                    Open         High          Low        Close       Volume
--------------------------------------------------------------------------------------------
2026-02-07 04:00     $70918.99 $71690.07 $70604.55 $70768.13      1760.77
...

=== Statistics ===
Starting price: $70768.13
Ending price:   $68983.76
Price change:   $-1784.37 (-2.52%)
```

**When to use this:** When you just need to quickly fetch and display candlestick data.

---

### 2. fetch_ohlcv.rs — Comprehensive OHLCV Tutorial
**Perfect for:** Learning all OHLCV patterns, production applications

**What it demonstrates:**
1. **Recent candles** — Fetch last N candles
2. **Date range queries** — Fetch candles for specific time period (last 24 hours)
3. **Multiple timeframes** — Compare 1m, 5m, 15m, 1h, 4h, 1d
4. **Large datasets** — Fetch 100 days of historical data
5. **Multiple symbols** — Fetch OHLCV for BTC, ETH, BNB, SOL, XRP
6. **Pagination** — Handle exchange limits by fetching in batches

**Output:**
```
1. Fetching last 10 candles for BTC/USDT (1 hour timeframe)...
   Fetched 10 candles

2. Fetching BTC/USDT 1h candles for last 24 hours...
   Fetched 24 candles in the last 24 hours

3. Comparing different timeframes (last 5 candles each)...
   1 MINUTE        | Close: $68989.74 | Volume: 2.27 BTC
   5 MINUTES       | Close: $68989.74 | Volume: 30.67 BTC
   ...

4. Fetching large dataset (last 100 daily candles)...
   Highest: $111250.01 on 2025-11-02
   Lowest:  $60000.00 on 2026-02-06

5. Fetching OHLCV for multiple symbols...
     BTC/USDT | $68989.63 | -0.05%
     ETH/USDT | $2036.07  | -0.26%
     ...

6. Demonstrating pagination (fetching 200 candles in batches)...
```

**When to use this:** When building a trading bot, backtesting system, or market analysis tool.

---

### 3. binance_public_api.rs — Public API Reference
**Perfect for:** Understanding all public endpoints

**What it demonstrates:**
- `fetch_ticker()` — Single ticker
- `fetch_tickers()` — Multiple tickers
- `fetch_order_book()` — Order book depth
- `fetch_trades()` — Recent trades
- `fetch_ohlcv()` — Candlestick data
- `fetch_markets()` — All trading pairs

**Output:**
```
1. Fetching BTC/USDT ticker...
   Last: $69091.41
   Bid: $69091.41
   Ask: $69091.42
   24h Change: 3.607%

2. Fetching BTC/USDT order book (top 5)...
   Best Bid: $69091.41 (2.62 BTC)
   Best Ask: $69091.42 (0.001 BTC)
   Spread: $0.01

3. Fetching recent BTC/USDT trades (5)...
   1. Buy 0.00008 BTC @ $69091.41
   ...
```

**When to use this:** When learning the public API or building a market data dashboard.

---

### 4. ccxt_style_usage.rs — CCXT API Compatibility Demo
**Perfect for:** Python CCXT users migrating to Rust

**What it demonstrates:**
- Exact same API surface as CCXT Python
- All convenience methods: `create_market_buy_order()`, `create_limit_sell_order()`, etc.
- Custom parameters passing
- Complete public + private API coverage

**Output:**
```
=== CCXT API Compatibility Summary ===

✓ Exchange construction with config
✓ load_markets() - Load and cache markets
✓ fetch_ticker(symbol) - Get single ticker
✓ create_market_buy_order(symbol, amount) - Convenience method
✓ create_limit_sell_order(symbol, amount, price) - Convenience method
...
All CCXT methods are supported with identical signatures! 🎉
```

**When to use this:** When porting existing CCXT code from Python/JS to Rust.

---

### 5. binance_export_parquet.rs — Data Export for Analysis
**Perfect for:** Backtesting, data science, analytics

**What it demonstrates:**
- Exporting OHLCV data to Parquet format
- Exporting trades history to Parquet
- Exporting tickers to Parquet
- Exporting order book snapshots to Parquet
- Using Polars DataFrame for data manipulation

**Output:**
```
1. Fetching BTC/USDT 1h OHLCV data (last 100 candles)...
   ✓ Exported 100 candles to data/binance/btc_usdt_1h.parquet

2. Fetching BTC/USDT recent trades...
   ✓ Exported 1000 trades to data/binance/btc_usdt_trades.parquet

3. Fetching tickers for multiple pairs...
   ✓ Exported 4 tickers to data/binance/tickers_snapshot.parquet

4. Fetching BTC/USDT order book...
   ✓ Exported order book to data/binance/btc_usdt_orderbook.parquet
```

**Files created:**
- `data/binance/btc_usdt_1h.parquet` — OHLCV candles
- `data/binance/btc_usdt_trades.parquet` — Trade history
- `data/binance/tickers_snapshot.parquet` — Ticker data
- `data/binance/btc_usdt_orderbook.parquet` — Order book snapshot

**When to use this:** When collecting data for backtesting, machine learning, or external analysis tools (Python pandas, R, etc.).

---

### 6. read_parquet.rs — Parquet Data Analysis
**Perfect for:** Analyzing exported data

**What it demonstrates:**
- Reading Parquet files with Polars
- Lazy evaluation for performance
- Statistical aggregations (min, max, mean, sum)
- Working with exported market data

**Output:**
```
1. BTC/USDT 1h OHLCV Data (first 5 candles):
┌────────────┬──────────┬──────────┬──────────┬──────────┬─────────┐
│ timestamp  │ open     │ high     │ low      │ close    │ volume  │
├────────────┼──────────┼──────────┼──────────┼──────────┼─────────┤
│ ...        │ ...      │ ...      │ ...      │ ...      │ ...     │
└────────────┴──────────┴──────────┴──────────┴──────────┴─────────┘

OHLCV Statistics:
┌───────────┬───────────┬────────────┬──────────────┐
│ min_close │ max_close │ avg_close  │ total_volume │
│ 62909.86  │ 78330.26  │ 71112.3932 │ 298045.72547 │
└───────────┴───────────┴────────────┴──────────────┘
```

**When to use this:** When analyzing previously exported data or integrating with data analysis workflows.

---

## 🔑 Common Patterns

### Pattern 1: Fetch Recent Data
```rust
let candles = exchange.fetch_ohlcv(
    "BTC/USDT",
    Timeframe::OneHour,
    None,           // None = recent data
    Some(100),      // Last 100 candles
).await?;
```

### Pattern 2: Fetch Historical Data (Date Range)
```rust
use chrono::Utc;

let now = Utc::now().timestamp_millis();
let one_week_ago = now - (7 * 24 * 60 * 60 * 1000);

let candles = exchange.fetch_ohlcv(
    "BTC/USDT",
    Timeframe::OneDay,
    Some(one_week_ago),  // Start date
    None,                // No limit (fetch all until now)
).await?;
```

### Pattern 3: Pagination (Fetch More Than Limit)
```rust
let mut all_candles = Vec::new();
let mut current_since = start_timestamp;

loop {
    let batch = exchange.fetch_ohlcv(
        "BTC/USDT",
        Timeframe::OneHour,
        Some(current_since),
        Some(500),  // Max per request
    ).await?;

    if batch.is_empty() {
        break;
    }

    current_since = batch.last().unwrap().timestamp + (60 * 60 * 1000);
    all_candles.extend(batch);
}
```

### Pattern 4: Multiple Symbols
```rust
let symbols = vec!["BTC/USDT", "ETH/USDT", "BNB/USDT"];

for symbol in symbols {
    let candles = exchange.fetch_ohlcv(
        symbol,
        Timeframe::OneHour,
        None,
        Some(10),
    ).await?;

    println!("{}: {} candles", symbol, candles.len());
}
```

---

## 🎓 Learning Path

**Beginner:**
1. Start with `simple_ohlcv.rs` — Understand basic fetching
2. Try `binance_public_api.rs` — Learn all public endpoints
3. Read `verify_binance_data.rs` — See data validation patterns

**Intermediate:**
1. Study `fetch_ohlcv.rs` — Learn all 6 OHLCV patterns
2. Review `ccxt_style_usage.rs` — Understand CCXT compatibility
3. Experiment with different timeframes and symbols

**Advanced:**
1. Explore `binance_export_parquet.rs` — Data pipeline patterns
2. Use `read_parquet.rs` — Analyze exported data
3. Combine patterns for custom trading bots

---

## ⚙️ Requirements

All examples require:
- Rust 1.70+
- `tokio` runtime (async/await)
- `binance` feature flag

Some examples have additional dependencies:
- **Parquet examples** — `polars` crate (included in dev-dependencies)
- **Date/time operations** — `chrono` crate (included in dependencies)

---

## 🔗 Resources

- **[API_REFERENCE.md](../API_REFERENCE.md)** — Complete API reference
- **[CCXT_API_MAPPING.md](../CCXT_API_MAPPING.md)** — Python ↔ Rust mapping
- **[CCXT_COMPATIBILITY.md](../CCXT_COMPATIBILITY.md)** — Compatibility status

---

## 💡 Tips

1. **Start simple** — Use `simple_ohlcv.rs` for quick testing
2. **Use the comprehensive example** — `fetch_ohlcv.rs` covers 90% of use cases
3. **Export for analysis** — Use Parquet examples for backtesting/ML workflows
4. **Handle errors properly** — All examples use `Result<T>` with `?` operator
5. **Use Decimal for money** — Never use `f64` for financial calculations

---

## 🐛 Troubleshooting

**Error: "unresolved import"**
```bash
# Make sure you have the binance feature enabled:
cargo run --example <name> --features binance
```

**Error: Rate limit exceeded**
```rust
// Add delays between requests:
tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
```

**Error: Symbol not found**
```rust
// Load markets first to validate symbols:
exchange.load_markets().await?;
```

---

**Happy Trading! 🚀**
