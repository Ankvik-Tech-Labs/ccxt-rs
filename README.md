# ccxt-rs: Unified Rust API for Cryptocurrency Exchange Trading

[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg?maxAge=3600)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**ccxt-rs** is a Rust implementation of [CCXT](https://github.com/ccxt/ccxt), providing a unified, type-safe async API for trading across centralized exchanges (CEX) and decentralized exchanges (DEX).

## Features

- **🔄 Unified API**: Same interface for both CEX (Binance, Bybit, OKX) and DEX (Uniswap, PancakeSwap)
- **💰 Type Safety**: `rust_decimal::Decimal` for all financial calculations (never `f64` for money)
- **⚡ Async/Await**: Built on `tokio` for high-performance concurrent operations
- **🚦 Rate Limiting**: Built-in per-exchange rate limiting with `governor`
- **📦 Feature Flags**: Compile only the exchanges you need
- **🔐 Security**: HMAC-SHA256/SHA512 authentication, wallet signing for DEX
- **🌐 Multi-Chain**: EVM support via `alloy-rs` for Ethereum, BSC, Arbitrum, Polygon, etc.

## Project Status

### ✅ Phase 1: Foundation (COMPLETED)

The core infrastructure is complete and compiles successfully:

- **Core Infrastructure**
  - ✅ Exchange trait with unified API
  - ✅ Error hierarchy matching CCXT conventions
  - ✅ HTTP client with rate limiting
  - ✅ HMAC signing utilities (SHA256, SHA512)
  - ✅ Decimal utilities for precise financial math
  - ✅ Timestamp and ISO 8601 helpers

- **Unified Types** (all complete)
  - ✅ Ticker, OrderBook, OHLCV, Trade
  - ✅ Order, Balance, Market, Currency
  - ✅ Position, FundingRate (derivatives)
  - ✅ Deposit, Withdrawal, Transfer
  - ✅ Fee structures

- **DEX Infrastructure**
  - ✅ Alloy provider setup
  - ✅ Wallet/signer utilities
  - ✅ ERC20 token helpers
  - ✅ GraphQL subgraph client

### 🚧 Phase 2-6: Exchange Implementations (PLANNED)

- **Phase 2**: Binance (CEX) - Spot, Futures, Margin
- **Phase 3**: Bybit + OKX (CEX)
- **Phase 4**: Uniswap (DEX) - V2, V3 support
- **Phase 5**: PancakeSwap (DEX)
- **Phase 6**: Polish, Examples, Tests, Documentation

## Supported Exchanges (Planned)

| Exchange | Type | Status | Markets |
|----------|------|--------|---------|
| **Binance** | CEX | Planned | Spot, Futures, Margin |
| **Bybit** | CEX | Planned | Spot, Futures, Options |
| **OKX** | CEX | Planned | Spot, Futures, Perpetual |
| **Uniswap** | DEX | Planned | V2, V3 (Ethereum, Arbitrum, Polygon) |
| **PancakeSwap** | DEX | Planned | V2, V3 (BSC) |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ccxt = { version = "0.1", features = ["binance", "uniswap"] }
```

### Feature Flags

- **Individual exchanges**: `binance`, `bybit`, `okx`, `uniswap`, `pancakeswap`
- **Aggregate features**: `all-cex`, `all-dex`, `all`

```toml
# Only Binance
ccxt = { version = "0.1", features = ["binance"] }

# All CEX exchanges
ccxt = { version = "0.1", features = ["all-cex"] }

# All exchanges
ccxt = { version = "0.1", features = ["all"] }
```

## Architecture

### Design Philosophy

Following CCXT's proven design:

1. **Single crate structure** — Not a workspace, matches Python CCXT's single package
2. **Flat exchange files** — Each exchange is one `.rs` file at `src/exchangename.rs`
3. **Unified types** — All exchanges return the same `Ticker`, `Order`, `Balance`, etc.
4. **Feature-gated compilation** — Only compile exchanges you use
5. **Decimal precision** — `rust_decimal::Decimal` everywhere, never `f64` for money

### Project Structure

```
ccxt-rs/
├── src/
│   ├── lib.rs                 # Main entry point
│   │
│   ├── base/                  # Core infrastructure
│   │   ├── exchange.rs        # Exchange trait
│   │   ├── errors.rs          # Error hierarchy
│   │   ├── http_client.rs     # Rate-limited HTTP client
│   │   ├── signer.rs          # HMAC/signing utilities
│   │   ├── rate_limiter.rs    # Rate limiting
│   │   ├── decimal.rs         # Decimal utilities
│   │   └── precise.rs         # Precise number formatting
│   │
│   ├── types/                 # Unified data structures
│   │   ├── ticker.rs
│   │   ├── orderbook.rs
│   │   ├── ohlcv.rs
│   │   ├── trade.rs
│   │   ├── order.rs
│   │   ├── balance.rs
│   │   ├── market.rs
│   │   ├── currency.rs
│   │   ├── position.rs
│   │   ├── funding.rs
│   │   ├── deposit.rs
│   │   ├── transfer.rs
│   │   └── fee.rs
│   │
│   ├── dex/                   # Shared DEX utilities
│   │   ├── provider.rs        # Alloy provider
│   │   ├── wallet.rs          # Wallet/signer
│   │   ├── erc20.rs           # ERC20 helpers
│   │   └── subgraph.rs        # GraphQL client
│   │
│   ├── binance.rs             # (Coming in Phase 2)
│   ├── bybit.rs               # (Coming in Phase 3)
│   ├── okx.rs                 # (Coming in Phase 3)
│   ├── uniswap.rs             # (Coming in Phase 4)
│   └── pancakeswap.rs         # (Coming in Phase 5)
│
└── tests/
    └── (Integration tests coming in Phase 6)
```

## Usage Examples (Planned)

### CEX Example (Binance)

```rust
use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create exchange instance
    let binance = ccxt::binance::Binance::builder()
        .api_key("your-api-key")
        .secret("your-secret")
        .sandbox(true)  // Use testnet
        .build()?;

    // Fetch ticker
    let ticker = binance.fetch_ticker("BTC/USDT").await?;
    println!("BTC/USDT: ${}", ticker.last.unwrap());

    // Fetch order book
    let orderbook = binance.fetch_order_book("BTC/USDT", Some(10)).await?;
    println!("Best bid: {}", orderbook.best_bid().unwrap().0);
    println!("Best ask: {}", orderbook.best_ask().unwrap().0);

    // Place order
    let order = binance.create_order(
        "BTC/USDT",
        OrderType::Limit,
        OrderSide::Buy,
        Decimal::from_str("0.001")?,
        Some(Decimal::from_str("50000")?),
        None,
    ).await?;

    println!("Order placed: {}", order.id);

    Ok(())
}
```

### DEX Example (Uniswap)

```rust
use ccxt::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create Uniswap instance
    let uniswap = ccxt::uniswap::Uniswap::builder()
        .private_key("0x...")
        .rpc_url("https://eth-mainnet.g.alchemy.com/v2/...")
        .chain_id(1)  // Ethereum mainnet
        .subgraph_api_key("...")
        .build()?;

    // Fetch ticker (pool price)
    let ticker = uniswap.fetch_ticker("WETH/USDC").await?;
    println!("WETH/USDC: ${}", ticker.last.unwrap());

    // Execute swap (DEX "order")
    let swap = uniswap.create_order(
        "WETH/USDC",
        OrderType::Market,
        OrderSide::Sell,
        Decimal::from_str("0.1")?,  // Sell 0.1 WETH
        None,
        None,
    ).await?;

    println!("Swap executed: {:?}", swap.id);

    Ok(())
}
```

## Technology Stack

| Dependency | Purpose | Version |
|-----------|---------|---------|
| `tokio` | Async runtime | 1.43+ |
| `reqwest` | HTTP client (CEX) | 0.12+ |
| `serde`, `serde_json` | JSON serialization | 1.x |
| `rust_decimal` | Precise financial math | 1.37+ |
| `alloy` | EVM blockchain interaction | 0.8+ |
| `uniswap-v3-sdk` | Uniswap V3 SDK | 6.0+ |
| `uniswap-v2-sdk` | Uniswap V2 SDK | 2.0+ |
| `governor` | Rate limiting | 0.8+ |
| `hmac`, `sha2` | HMAC signing | 0.12+, 0.10+ |
| `chrono` | Timestamps | 0.4+ |
| `async-trait` | Async trait support | 0.1+ |
| `thiserror` | Error types | 2.x |

## Testing

```bash
# Run all tests
cargo test --all-features

# Test specific exchange
cargo test --features binance

# Run with output
cargo test --all-features -- --nocapture
```

## Development

```bash
# Check compilation
cargo check --all-features

# Build with all exchanges
cargo build --all-features

# Run clippy
cargo clippy --all-features -- -D warnings

# Format code
cargo fmt
```

## Contributing

Contributions welcome! This project follows CCXT conventions for consistency with the broader CCXT ecosystem.

### Development Priorities

1. **Phase 2**: Implement Binance (reference CEX implementation)
2. **Phase 3**: Implement Bybit + OKX
3. **Phase 4**: Implement Uniswap (reference DEX implementation)
4. **Phase 5**: Implement PancakeSwap
5. **Phase 6**: Polish, examples, comprehensive tests

## Roadmap

### Current: Phase 1 (Foundation) ✅
- Core infrastructure complete
- All unified types defined
- DEX utilities ready
- Compiles cleanly with zero warnings

### Next: Phase 2 (Binance)
- Public API (markets, ticker, orderbook, OHLCV, trades)
- Private API (orders, balance, positions, funding)
- Authentication (HMAC-SHA256, RSA, Ed25519)
- Rate limiting
- Error mapping
- Integration tests

### Future Phases
- Phase 3: Bybit + OKX
- Phase 4: Uniswap V2/V3
- Phase 5: PancakeSwap
- Phase 6: Examples, documentation, benchmarks

## Why ccxt-rs?

**Problem**: Rust trading bots must implement exchange APIs from scratch or use FFI to Python CCXT — both error-prone and slow.

**Solution**: Unified async Rust API leveraging:
- Type system for compile-time safety
- `rust_decimal::Decimal` for precise financial math (no float rounding errors)
- Native async/await for performance
- Feature flags for minimal binary size
- Existing DEX SDKs (Uniswap, etc.) for DEX support

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [CCXT](https://github.com/ccxt/ccxt) - The original multi-exchange trading library
- [shuhuiluo](https://github.com/shuhuiluo) - Uniswap SDK Rust implementations
- [infinitefield](https://github.com/infinitefield/hypersdk) - Hyperliquid SDK
- [alloy-rs](https://github.com/alloy-rs) - Modern Ethereum library

## Links

- [CCXT Documentation](https://docs.ccxt.com/)
- [CCXT GitHub](https://github.com/ccxt/ccxt)
- [Uniswap V3 SDK Rust](https://github.com/shuhuiluo/uniswap-v3-sdk-rs)

---

**Status**: Phase 1 (Foundation) Complete ✅ | Active Development 🚧
