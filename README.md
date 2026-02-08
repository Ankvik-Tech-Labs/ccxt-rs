# ccxt-rs: Unified Rust API for Cryptocurrency Exchange Trading

[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg?maxAge=3600)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**ccxt-rs** is a Rust implementation of [CCXT](https://github.com/ccxt/ccxt), providing a unified, type-safe async API for trading across centralized exchanges (CEX) and decentralized exchanges (DEX).

> 20,000+ LOC | 5 Exchanges | 100+ Unified Methods | 120+ Feature Flags | WebSocket Real-Time Streams

## Features

- **Unified API** - Same `Exchange` trait for both CEX (Binance, Bybit, OKX) and DEX (Uniswap, Hyperliquid)
- **Type Safety** - `rust_decimal::Decimal` for all financial calculations (never `f64` for money)
- **Async/Await** - Built on `tokio` for high-performance concurrent operations
- **WebSocket Streaming** - Real-time ticker, order book, trades, and OHLCV via `ExchangeWs` trait
- **Rate Limiting** - Built-in per-exchange rate limiting with `governor`
- **Feature Flags** - Compile only the exchanges you need
- **Full Private API** - Order management, deposits/withdrawals, positions, leverage on all 3 CEX exchanges
- **Multi-Chain DEX** - EVM support via `alloy-rs` for Ethereum, BSC, Arbitrum, Polygon
- **EIP-712 Signing** - Native L1 action signing for Hyperliquid perp DEX

## Supported Exchanges

| Exchange | Type | Auth | Public API | Private API | WebSocket | Markets |
|----------|------|------|:----------:|:-----------:|:---------:|---------|
| **Binance** | CEX | HMAC-SHA256 | Full | Full (~23 methods) | Full | Spot, Futures, Margin |
| **Bybit** | CEX | HMAC-SHA256 | Full | Full (~20 methods) | Full | Spot, Futures (v5 unified) |
| **OKX** | CEX | HMAC-SHA256-Base64 | Full | Full (~22 methods) | Full | Spot, Futures, Perpetual |
| **Hyperliquid** | DEX | EIP-712 | Full | Full | Full | Perpetual |
| **Uniswap V3** | DEX | - | Full | Read-only | - | Ethereum, Arbitrum, Polygon |

### Per-Exchange Capabilities

<details>
<summary>Binance (Spot + Futures)</summary>

- Markets, Tickers, Order Book, OHLCV, Trades
- Create/Cancel/Edit Orders, Fetch Orders (open, closed, all)
- Fetch Balance, My Trades, Trading Fees
- Deposit Address, Deposits, Withdrawals, Withdraw, Transfer
- Positions, Funding Rate, Set Leverage, Set Margin Mode
- Currencies, Cancel All Orders
- **WebSocket**: Ticker, Order Book, Trades, OHLCV (public); Orders, Balance, Positions (private)

</details>

<details>
<summary>Bybit (v5 Unified API)</summary>

- Markets, Tickers, Order Book, OHLCV, Trades
- Create/Cancel/Edit Orders, Fetch Orders (open, closed, all)
- Fetch Balance, My Trades, Trading Fees
- Deposit Address, Deposits, Withdrawals, Withdraw, Transfer
- Positions, Funding Rate, Set Leverage, Set Margin Mode
- Currencies, Cancel All Orders
- **WebSocket**: Ticker, Order Book, Trades, OHLCV (public); Orders, Balance, Positions (private)

</details>

<details>
<summary>OKX (v5 API)</summary>

- Markets, Tickers, Order Book, OHLCV, Trades
- Create/Cancel/Edit Orders, Fetch Orders (open, closed, all)
- Fetch Balance, My Trades, Trading Fees
- Deposit Address, Deposits, Withdrawals, Withdraw, Transfer
- Positions, Funding Rate, Set Leverage, Set Margin Mode
- Currencies, Cancel All Orders
- **WebSocket**: Ticker, Order Book, Trades, OHLCV (public); Orders, Balance, Positions (private)

</details>

<details>
<summary>Hyperliquid (Perp DEX)</summary>

- Markets, Tickers, Order Book, OHLCV, Trades
- Create/Cancel Orders, Fetch Open Orders
- Fetch Balance, Positions, Funding Rate
- Set Leverage, Set Margin Mode
- EIP-712 signed L1 actions
- **WebSocket**: Ticker, Order Book, Trades (public); Orders, Positions (private via user address)

</details>

<details>
<summary>Uniswap V3 (DEX - Read Only)</summary>

- Pool discovery via The Graph subgraph
- Tickers (pool prices), Order Book (liquidity depth)
- OHLCV (hourly/daily candles), Recent Trades (swaps)
- Multi-chain: Ethereum, Polygon, Arbitrum

</details>

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ccxt = { version = "0.0.1", features = ["binance"] }
tokio = { version = "1", features = ["full"] }
rust_decimal = "1.37"
```

### Public API (No credentials needed)

```rust
use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let binance = ccxt::binance::Binance::builder()
        .build()?;

    // Fetch ticker
    let ticker = binance.fetch_ticker("BTC/USDT").await?;
    println!("BTC/USDT: {:?}", ticker.last);

    // Fetch order book
    let book = binance.fetch_order_book("BTC/USDT", Some(5)).await?;
    println!("Best bid: {} @ {}", book.bids[0].1, book.bids[0].0);

    // Fetch OHLCV candles
    let candles = binance.fetch_ohlcv("BTC/USDT", Timeframe::OneHour, None, Some(10)).await?;
    for c in &candles {
        println!("O:{} H:{} L:{} C:{} V:{}", c.open, c.high, c.low, c.close, c.volume);
    }

    Ok(())
}
```

### Private API (Trading)

```rust
use ccxt::prelude::*;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<()> {
    let binance = ccxt::binance::Binance::builder()
        .api_key("your-api-key")
        .secret("your-secret")
        .build()?;

    // Check balance
    let balance = binance.fetch_balance().await?;
    println!("USDT: {:?}", balance.free.get("USDT"));

    // Place a limit order
    let order = binance.create_limit_buy_order(
        "BTC/USDT", dec!(0.001), dec!(50000), None
    ).await?;
    println!("Order: {} {:?}", order.id, order.status);

    // Cancel it
    let canceled = binance.cancel_order(&order.id, Some("BTC/USDT")).await?;
    println!("Canceled: {:?}", canceled.status);

    Ok(())
}
```

### WebSocket Real-Time Streaming

```rust
use ccxt::base::ws::{ExchangeWs, WsConfig};
use ccxt::binance::ws::BinanceWs;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WsConfig::default();
    let ws = BinanceWs::new(false, config);

    // Subscribe to real-time ticker updates
    let mut stream = ws.watch_ticker("BTC/USDT").await?;

    // Receive updates as they arrive
    while let Some(ticker) = stream.next().await {
        println!("BTC/USDT: ${}", ticker.last.unwrap_or_default());
    }

    ws.close().await?;
    Ok(())
}
```

### Multi-Exchange Comparison

```rust
use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let binance = ccxt::binance::Binance::builder().build()?;
    let bybit = ccxt::bybit::Bybit::builder().build()?;
    let okx = ccxt::okx::Okx::builder().build()?;

    // Same API across all exchanges
    let exchanges: Vec<&dyn Exchange> = vec![&binance, &bybit, &okx];

    for ex in exchanges {
        let ticker = ex.fetch_ticker("BTC/USDT").await?;
        println!("{}: {:?}", ex.name(), ticker.last);
    }

    Ok(())
}
```

### Hyperliquid (Perp DEX)

```rust
use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let hl = Hyperliquid::builder()
        .private_key("0x...")  // EVM private key for signing
        .build()?;

    // Same unified API as CEX
    let ticker = hl.fetch_ticker("BTC/USD:USDC").await?;
    let positions = hl.fetch_positions(None).await?;
    let balance = hl.fetch_balance().await?;

    Ok(())
}
```

## Installation

### Feature Flags

```toml
# Individual exchanges
ccxt = { version = "0.0.1", features = ["binance"] }
ccxt = { version = "0.0.1", features = ["bybit"] }
ccxt = { version = "0.0.1", features = ["okx"] }
ccxt = { version = "0.0.1", features = ["hyperliquid"] }
ccxt = { version = "0.0.1", features = ["uniswap"] }

# Groups
ccxt = { version = "0.0.1", features = ["all-cex"] }    # binance + bybit + okx
ccxt = { version = "0.0.1", features = ["all-dex"] }    # uniswap + pancakeswap + hyperliquid
ccxt = { version = "0.0.1", features = ["all"] }        # everything
```

## WebSocket API

All exchanges with WebSocket support implement the `ExchangeWs` trait. Streams return `WsStream<T>` wrappers over `tokio::sync::broadcast` channels, supporting multiple consumers per stream.

### Public Streams (No credentials)

| Method | Description | Binance | Bybit | OKX | Hyperliquid |
|--------|-------------|:-------:|:-----:|:---:|:-----------:|
| `watch_ticker(symbol)` | Real-time price ticker | Y | Y | Y | Y |
| `watch_order_book(symbol, limit)` | Order book updates | Y | Y | Y | Y |
| `watch_trades(symbol)` | Trade stream | Y | Y | Y | Y |
| `watch_ohlcv(symbol, timeframe)` | Candlestick updates | Y | Y | Y | - |

### Private Streams (Requires credentials)

| Method | Description | Binance | Bybit | OKX | Hyperliquid |
|--------|-------------|:-------:|:-----:|:---:|:-----------:|
| `watch_orders(symbol)` | Order updates | Y | Y | Y | Y |
| `watch_balance()` | Balance changes | Y | Y | Y | - |
| `watch_positions(symbols)` | Position updates | Y | Y | Y | Y |
| `watch_my_trades(symbol)` | Your trade fills | Y | Y | Y | Y |

### Connection Features

- **Automatic reconnection** with exponential backoff (1s -> 2s -> 4s -> max 30s)
- **Subscription replay** on reconnect (all active subscriptions re-sent)
- **Lazy connections** (WebSocket connects only on first `watch_*` call)
- **Configurable** ping interval, pong timeout, channel capacity via `WsConfig`

## Unified REST API Reference

All exchanges implement the `Exchange` trait. Methods return `Err(CcxtError::NotSupported)` if the exchange doesn't support a given operation.

### Market Data (Public)

| Method | Description | Binance | Bybit | OKX | Hyperliquid | Uniswap |
|--------|-------------|:-------:|:-----:|:---:|:-----------:|:-------:|
| `fetch_markets()` | List all trading pairs | Y | Y | Y | Y | Y |
| `fetch_currencies()` | List all currencies | Y | Y | Y | - | - |
| `fetch_ticker(symbol)` | Get price ticker | Y | Y | Y | Y | Y |
| `fetch_tickers(symbols)` | Get multiple tickers | Y | Y | Y | Y | - |
| `fetch_order_book(symbol, limit)` | Get order book | Y | Y | Y | Y | Y |
| `fetch_ohlcv(symbol, tf, since, limit)` | Get candlesticks | Y | Y | Y | Y | Y |
| `fetch_trades(symbol, since, limit)` | Get recent trades | Y | Y | Y | Y | Y |

### Trading (Private)

| Method | Description | Binance | Bybit | OKX |
|--------|-------------|:-------:|:-----:|:---:|
| `create_order(symbol, type, side, amount, price, params)` | Place an order | Y | Y | Y |
| `cancel_order(id, symbol)` | Cancel an order | Y | Y | Y |
| `edit_order(id, symbol, type, side, amount, price)` | Modify an order | Y | Y | Y |
| `cancel_all_orders(symbol)` | Cancel all orders | Y | Y | Y |
| `fetch_order(id, symbol)` | Get order details | Y | Y | Y |
| `fetch_orders(symbol, since, limit)` | Get order history | Y | Y | Y |
| `fetch_open_orders(symbol, since, limit)` | Get open orders | Y | Y | Y |
| `fetch_closed_orders(symbol, since, limit)` | Get closed orders | Y | Y | Y |
| `fetch_my_trades(symbol, since, limit)` | Get your trades | Y | Y | Y |

### Convenience Order Methods

```rust
// These delegate to create_order() with the right type/side:
exchange.create_market_buy_order("BTC/USDT", amount, None).await?;
exchange.create_market_sell_order("BTC/USDT", amount, None).await?;
exchange.create_limit_buy_order("BTC/USDT", amount, price, None).await?;
exchange.create_limit_sell_order("BTC/USDT", amount, price, None).await?;
```

### Account & Transfers (Private)

| Method | Description | Binance | Bybit | OKX |
|--------|-------------|:-------:|:-----:|:---:|
| `fetch_balance()` | Get balances | Y | Y | Y |
| `fetch_deposit_address(code)` | Get deposit address | Y | Y | Y |
| `fetch_deposits(code, since, limit)` | Deposit history | Y | Y | Y |
| `fetch_withdrawals(code, since, limit)` | Withdrawal history | Y | Y | Y |
| `withdraw(code, amount, address, tag)` | Withdraw funds | Y | Y | Y |
| `transfer(code, amount, from, to)` | Internal transfer | Y | Y | Y |
| `fetch_trading_fees()` | Get trading fees | Y | Y | Y |

### Derivatives (Private)

| Method | Description | Binance | Bybit | OKX |
|--------|-------------|:-------:|:-----:|:---:|
| `fetch_positions(symbols)` | Open positions | Y | Y | Y |
| `fetch_funding_rate(symbol)` | Current funding rate | Y | Y | Y |
| `set_leverage(leverage, symbol)` | Set leverage | Y | Y | Y |
| `set_margin_mode(mode, symbol)` | Set margin mode | Y | Y | Y |

### Additional Trait Methods (Default: NotSupported)

The `Exchange` trait defines 100+ methods including batch operations, advanced order types, margin borrowing, options/greeks, liquidations, ledger, and conversions. Exchanges implement what they support; the rest return `NotSupported`.

## Unified Types

All exchanges return the same type-safe Rust structs using `rust_decimal::Decimal`:

| Type | Description | Key Fields |
|------|-------------|------------|
| `Market` | Trading pair info | symbol, base, quote, limits, precision |
| `Ticker` | Price snapshot | symbol, last, bid, ask, volume, percentage |
| `OrderBook` | Depth of market | symbol, bids, asks, timestamp |
| `OHLCV` | Candlestick | timestamp, open, high, low, close, volume |
| `Trade` | Executed trade | id, symbol, side, price, amount, timestamp |
| `Order` | Order info | id, symbol, type, side, price, amount, status |
| `Balances` | Account balances | free, used, total (HashMap per currency) |
| `Position` | Open position | symbol, side, contracts, entry_price, pnl |
| `FundingRate` | Perp funding | symbol, funding_rate, next_funding_time |
| `Currency` | Currency info | code, name, networks, precision, limits |
| `Deposit` / `Withdrawal` | Transfer record | id, currency, amount, status, address |
| `Transfer` | Internal transfer | id, currency, amount, from/to account |
| `TradingFees` | Fee structure | maker, taker (Decimal) |

Plus 15 additional types: `LedgerEntry`, `Greeks`, `OptionContract`, `OpenInterest`, `Liquidation`, `LongShortRatio`, `Leverage`, `MarginModification`, `Conversion`, `FundingHistory`, `BorrowRate`, `LastPrice`, `DepositWithdrawFee`, `Account`, `LeverageTier`.

## Error Handling

ccxt-rs provides a rich error hierarchy matching CCXT's error classes:

```rust
use ccxt::prelude::*;

match exchange.create_order(/*...*/).await {
    Ok(order) => println!("Order placed: {}", order.id),
    Err(CcxtError::InsufficientFunds(msg)) => eprintln!("Not enough funds: {}", msg),
    Err(CcxtError::InvalidOrder(msg)) => eprintln!("Bad order params: {}", msg),
    Err(CcxtError::RateLimitExceeded(_)) => eprintln!("Slow down!"),
    Err(CcxtError::AuthenticationError(_)) => eprintln!("Check your API keys"),
    Err(CcxtError::WsConnectionError(_)) => eprintln!("WebSocket connection failed"),
    Err(e) if e.is_retryable() => eprintln!("Transient error, retry: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

38 error variants covering: authentication, account, order, request, network, response, WebSocket, and internal errors.

## Architecture

### Project Structure

```
ccxt-rs/
├── src/
│   ├── lib.rs                    # Crate root, prelude, feature gates
│   ├── base/                     # Core infrastructure
│   │   ├── exchange.rs           # Exchange trait (100+ methods, 120+ feature flags)
│   │   ├── errors.rs             # 38-variant error hierarchy
│   │   ├── http_client.rs        # Rate-limited HTTP client
│   │   ├── signer.rs             # HMAC-SHA256/512, timestamps
│   │   ├── rate_limiter.rs       # Token-bucket rate limiter
│   │   ├── decimal.rs            # Decimal parsing utilities
│   │   ├── precise.rs            # Precise number formatting
│   │   ├── ws.rs                 # ExchangeWs trait, WsStream, WsConfig
│   │   └── ws_connection.rs      # WsConnectionManager (reconnect, ping/pong)
│   ├── types/                    # 30 unified type files
│   │   ├── ticker.rs, orderbook.rs, ohlcv.rs, trade.rs, order.rs
│   │   ├── balance.rs, market.rs, currency.rs, position.rs
│   │   ├── funding.rs, deposit.rs, transfer.rs, fee.rs
│   │   ├── ledger.rs, greeks.rs, option.rs, open_interest.rs
│   │   ├── liquidation.rs, long_short.rs, leverage.rs
│   │   ├── margin_mod.rs, conversion.rs, funding_history.rs
│   │   ├── borrow.rs, last_price.rs, deposit_withdraw_fee.rs
│   │   ├── account.rs, leverage_tier.rs, common.rs
│   │   └── mod.rs
│   ├── binance/                  # Binance (HMAC-SHA256, spot + futures)
│   │   ├── mod.rs, parsers.rs, types.rs, ws.rs
│   ├── bybit/                    # Bybit (HMAC-SHA256, v5 unified)
│   │   ├── mod.rs, parsers.rs, ws.rs
│   ├── okx/                      # OKX (HMAC-SHA256-Base64 + passphrase)
│   │   ├── mod.rs, parsers.rs, ws.rs
│   ├── hyperliquid/              # Hyperliquid (EIP-712 signing)
│   │   ├── mod.rs, exchange.rs, parsers.rs, signer.rs, client.rs
│   │   ├── constants.rs, types.rs, ws.rs
│   ├── uniswap/                  # Uniswap V3 (subgraph + on-chain)
│   │   ├── exchange.rs, parsers.rs, pools.rs, constants.rs
│   ├── dex/                      # Shared DEX utilities
│   │   ├── provider.rs, wallet.rs, erc20.rs, subgraph.rs
│   └── pancakeswap.rs            # Stub
├── examples/                     # 16 runnable examples
├── tests/                        # Unit, integration, and sandbox tests
└── Cargo.toml
```

### Design Principles

1. **CCXT Compatibility** - Method names and signatures match CCXT Python/JS exactly
2. **Unified Types** - All exchanges return the same `Ticker`, `Order`, `Balance`, etc.
3. **Decimal Precision** - `rust_decimal::Decimal` everywhere, never `f64` for money
4. **Feature-Gated** - Only compile the exchanges you need
5. **Builder Pattern** - Exchange instances created via `Exchange::builder().build()`
6. **Default NotSupported** - New trait methods default to `NotSupported` so implementations compile without changes

## Examples

The `examples/` directory contains 16 runnable examples:

```bash
# --- Public API (no credentials needed) ---
cargo run --example binance_public_api --features binance
cargo run --example bybit_public_api --features bybit
cargo run --example okx_public_api --features okx
cargo run --example multi_exchange_comparison --features binance,bybit,okx

# --- OHLCV Data ---
cargo run --example fetch_ohlcv --features binance
cargo run --example simple_ohlcv --features binance

# --- CCXT-compatible patterns ---
cargo run --example ccxt_style_usage --features binance

# --- Data Export ---
cargo run --example binance_export_parquet --features binance
cargo run --example read_parquet
cargo run --example verify_binance_data

# --- WebSocket Streaming ---
cargo run --example websocket_ticker --features binance
cargo run --example websocket_trading --features binance

# --- Live Trading (SANDBOX MODE - requires credentials) ---
BINANCE_API_KEY=key BINANCE_SECRET=secret \
  cargo run --example live_trading_basics --features binance

BINANCE_API_KEY=key BINANCE_SECRET=secret \
  cargo run --example position_management --features binance

BINANCE_API_KEY=key BINANCE_SECRET=secret \
  cargo run --example advanced_orders --features binance

BINANCE_API_KEY=key BINANCE_SECRET=secret \
BYBIT_API_KEY=key BYBIT_SECRET=secret \
OKX_API_KEY=key OKX_SECRET=secret OKX_PASSPHRASE=pass \
  cargo run --example multi_exchange_trading --features binance,bybit,okx
```

## Testing

```bash
# Run all unit tests + doctests
cargo test --all-features

# Test specific exchange
cargo test --features binance
cargo test --features okx

# Run WebSocket integration tests (requires network)
cargo test --all-features -- --ignored ws_ --test-threads=1

# Run sandbox trading tests (requires credentials)
BINANCE_SANDBOX_API_KEY=key BINANCE_SANDBOX_SECRET=secret \
  cargo test --all-features -- --ignored sandbox --test-threads=1

# Run order type tests (requires sandbox credentials)
BINANCE_SANDBOX_API_KEY=key BINANCE_SANDBOX_SECRET=secret \
  cargo test --all-features -- --ignored order_type --test-threads=1

# Run position tests (requires futures sandbox credentials)
BINANCE_SANDBOX_API_KEY=key BINANCE_SANDBOX_SECRET=secret \
  cargo test --all-features -- --ignored position --test-threads=1

# Run error scenario tests
BINANCE_SANDBOX_API_KEY=key BINANCE_SANDBOX_SECRET=secret \
  cargo test --all-features -- --ignored error_scenario --test-threads=1

# Check for warnings and clippy lints
cargo clippy --all-features

# Build documentation
cargo doc --all-features --no-deps --open
```

### Test Categories

| Category | Description | Requires |
|----------|-------------|----------|
| Unit tests | Parser tests, type validation, helpers | Nothing |
| Doctests | In-code examples | Nothing |
| Sandbox tests | Order lifecycle on testnet | Sandbox API credentials |
| Order type tests | Market, limit, stop-loss, post-only | Sandbox API credentials |
| Position tests | Open/close, leverage, margin mode | Futures sandbox credentials |
| Error scenario tests | Bad symbol, auth, insufficient funds | Sandbox API credentials |
| WS integration tests | Live WebSocket stream validation | Network access |

## Technology Stack

| Dependency | Purpose | Version |
|-----------|---------|---------|
| `tokio` | Async runtime | 1.43+ |
| `reqwest` | HTTP client (CEX) | 0.12+ |
| `serde` / `serde_json` | JSON serialization | 1.x |
| `rust_decimal` | Precise financial math | 1.37+ |
| `tokio-tungstenite` | WebSocket connections | 0.24+ |
| `futures-util` | Stream utilities | 0.3+ |
| `alloy` | EVM blockchain interaction (DEX) | 0.8+ |
| `uniswap-v3-sdk` | Uniswap V3 math | 6.0+ |
| `governor` | Token-bucket rate limiting | 0.8+ |
| `hmac` / `sha2` | HMAC signing | 0.12+ / 0.10+ |
| `rmp-serde` | MessagePack (Hyperliquid) | 1.3+ |
| `chrono` | Timestamps | 0.4+ |
| `async-trait` | Async trait support | 0.1+ |
| `thiserror` | Derive Error | 2.x |
| `tracing` | Structured logging | 0.1+ |

## Project Status

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Foundation: core trait, types, errors, HTTP client, DEX infra | Complete |
| Phase 2 | Expanded trait (100+ methods), 30 types, 38 errors, 120 feature flags | Complete |
| Phase 3 | Full private APIs for Binance, Bybit, OKX | Complete |
| Phase 3.5 | Production testing framework (sandbox, validators) | Complete |
| Phase 4 | Live trading examples & comprehensive private API tests | Complete |
| Phase 5 | WebSocket real-time streaming for all exchanges | Complete |
| Phase 6 | New CEX exchanges: Kraken, Coinbase, KuCoin, Gate.io, Bitget | Planned |
| Phase 7 | Perp DEX: dYdX, GMX, Jupiter | Planned |
| Phase 8 | Implicit API method generation | Planned |
| Phase 9 | Advanced features, CI/CD, crates.io publish | Planned |

## Contributing

Contributions are welcome! This project follows CCXT conventions for consistency with the broader ecosystem.

To add a new exchange:

1. Create `src/{exchange}/mod.rs` with builder pattern and `Exchange` trait impl
2. Create `src/{exchange}/parsers.rs` for response parsing to unified types
3. Create `src/{exchange}/ws.rs` implementing `ExchangeWs` trait for WebSocket support
4. Add feature flag in `Cargo.toml`
5. Add module declaration in `src/lib.rs`
6. Implement public API methods first, then private API with signing

See the existing Binance or OKX implementations as reference.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [CCXT](https://github.com/ccxt/ccxt) - The original multi-exchange trading library
- [shuhuiluo](https://github.com/shuhuiluo) - Uniswap SDK Rust implementations
- [alloy-rs](https://github.com/alloy-rs) - Modern Ethereum library

## Links

- [CCXT Documentation](https://docs.ccxt.com/)
- [CCXT GitHub](https://github.com/ccxt/ccxt)
- [Rust Decimal](https://docs.rs/rust_decimal)

---

**Status**: Phases 1-5 Complete | Active Development
