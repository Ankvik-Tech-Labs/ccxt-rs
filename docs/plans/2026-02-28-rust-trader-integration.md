# ccxt-rs: rust_trader Integration Plan

**Date**: 2026-02-28
**Context**: `rust_trader` at `../rust_trader/` depends on ccxt-rs for two roles:
1. **Binance** — historical OHLCV data collection for backtesting (already works ✅)
2. **DEXes** — live/paper trade execution (Hyperliquid ✅ ready, Uniswap V3 ❌ needs swap execution)

This plan covers the ccxt-rs work needed to make rust_trader's Phase 3 (live DEX trading) possible.

---

## Priority 1: Uniswap V3 Swap Execution (CRITICAL)

**File**: `src/uniswap/exchange.rs`
**Current**: `create_order` returns `NotSupported`
**Goal**: Execute real ERC-20 → ERC-20 swaps via Uniswap V3 SwapRouter

### What needs to be implemented

**`create_order` in `src/uniswap/exchange.rs`:**

```rust
async fn create_order(
    &self,
    symbol: &str,          // e.g. "WETH/USDC"
    order_type: OrderType, // Only Market supported initially
    side: OrderSide,       // Buy or Sell
    amount: Decimal,       // Amount of base token
    price: Option<Decimal>,// Slippage tolerance as price (None = auto 0.5%)
    params: Option<&Params>,
) -> Result<Order>
```

### Implementation approach

**Step 1: Token resolution**
- Parse `symbol` into `token_in` / `token_out` addresses (use existing `fetch_markets` data)
- Determine `fee_tier` from market info (500, 3000, or 10000 bps)

**Step 2: Quote via `Quoter` contract**
Use the Uniswap V3 Quoter contract to get `amountOut` for `amountIn`:
- Mainnet Quoter V2: `0x61fFE014bA17989E743c5F6cB21bF9697530B21e`
- Arbitrum Quoter V2: `0x61fFE014bA17989E743c5F6cB21bF9697530B21e` (same)
- Call `quoteExactInputSingle` (view function, no gas)

**Step 3: Slippage calculation**
```
min_amount_out = quoted_amount_out * (1 - slippage_pct / 100)
```
Default slippage: 0.5% (configurable via `params["slippage"]`)

**Step 4: Approve token if needed**
Use existing `Erc20Token` from `src/dex/erc20.rs`:
```rust
token_in.approve(SWAP_ROUTER_ADDRESS, amount).await?;
```

**Step 5: Call SwapRouter**
SwapRouter V2 addresses:
- Ethereum/Arbitrum: `0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45`
- Polygon: `0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45`

Call `exactInputSingle` with:
```solidity
struct ExactInputSingleParams {
    address tokenIn;
    address tokenOut;
    uint24  fee;
    address recipient;
    uint256 amountIn;
    uint256 amountOutMinimum;
    uint160 sqrtPriceLimitX96; // 0 = no limit
}
```

**Step 6: Build and submit tx via alloy**
```rust
use alloy::sol;
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ISwapRouter,
    "abi/SwapRouter02.json"
);
```

Or use the existing `src/dex/wallet.rs` pattern with raw ABI encoding.

**Step 7: Wait for receipt and return Order**
Return `Order` with `id = tx_hash`, `status = Closed`, `filled = amount`, `cost = amount_out`.

### Files to create/modify

- `src/uniswap/exchange.rs` — implement `create_order`
- `abi/SwapRouter02.json` — SwapRouter ABI (get from Uniswap GitHub or etherscan)
- `abi/QuoterV2.json` — Quoter ABI
- `src/uniswap/swap.rs` — (new) swap helper logic (quote, approve, execute)
- `tests/uniswap_swap_test.rs` — integration test against Arbitrum fork

### Testing approach

Use `anvil` (from Foundry) forking Arbitrum mainnet:
```bash
anvil --fork-url $ARBITRUM_RPC --fork-block-number <recent>
```

Test:
1. Quote WETH → USDC on Arbitrum, verify quote is reasonable
2. Approve router for WETH
3. Execute swap, verify USDC balance increases
4. Verify `Order` returned has correct fields

### Estimated effort: 3-5 days

---

## Priority 2: Uniswap V4 Support (Grant Differentiator)

**Status**: Not started
**Value**: Strong differentiator for Arbitrum/Optimism grants — V4 hooks are very new

### What's needed

V4 uses a completely different architecture (singleton PoolManager, hooks):
- `PoolManager` contract: `0x000000...` (deployed Jan 2025)
- Hooks: Custom contracts that plug into swap lifecycle
- `UnlockCallback` pattern instead of direct swaps

**Approach**: Add `src/uniswap_v4/` module alongside existing V3 code.

### Key difference from V3

```
V3: SwapRouter.exactInputSingle(params) → direct swap
V4: PoolManager.unlock(data) → callback → PoolManager.swap(key, params, hookData)
```

### Files to create

- `src/uniswap_v4/exchange.rs` — `UniswapV4` implementing `Exchange` trait
- `src/uniswap_v4/pool_manager.rs` — PoolManager interaction
- `src/uniswap_v4/hooks.rs` — Hook data encoding helpers
- Feature flag: `uniswap_v4`

### Estimated effort: 2-3 weeks

---

## Priority 3: alloy 0.8 → 1.x Upgrade

**Current**: `alloy = "0.8"`
**Target**: `alloy = "1.x"` (latest stable)

### Why this matters

- `alloy` 1.x has breaking API changes but is the ecosystem standard
- Enables using `hypersdk` and other alloy 1.x-dependent crates
- Required for long-term maintenance

### Breaking changes to audit

1. Provider API changes (`.provider()` → `.provider_ref()`)
2. `TransactionRequest` field name changes
3. `Signer` trait changes
4. Network type parameter changes

### Files to update

All files in `src/dex/`, `src/hyperliquid/`, `src/uniswap/`

### Estimated effort: 3-5 days (mostly mechanical)

---

## Priority 4: dYdX v4 Support (Grant Target)

**Status**: Not started
**Value**: dYdX has actively funded Rust trading clients ($50K-$200K)

### What's needed

dYdX v4 is a Cosmos-based chain with:
- REST + WebSocket API (no EVM)
- CLOB (central limit order book) on-chain
- Native USDC for settlement

### Key endpoints for rust_trader

- `GET /v4/candles/perpetualMarkets/{ticker}` — OHLCV ✅ (simple REST)
- `GET /v4/orderbooks/perpetualMarkets/{ticker}` — Order book
- `POST /v4/orders` — Place order (requires Cosmos signing)
- `GET /v4/fills` — Trade history

### Cosmos signing

Unlike EVM chains, dYdX v4 uses:
- `secp256k1` key pair
- Cosmos `StdTx` transaction format
- `cosmos-sdk` style message encoding (protobuf)

Dependencies to add:
```toml
cosmos-sdk-proto = "0.20"   # protobuf types
k256 = "0.13"               # secp256k1 signing
bech32 = "0.11"             # address encoding (dydx1...)
```

### Files to create

- `src/dydx/` — New module
- `src/dydx/exchange.rs` — `DyDxV4` implementing `Exchange` trait
- `src/dydx/signing.rs` — Cosmos tx signing
- Feature flag: `dydx`

### Estimated effort: 2-3 weeks

---

## Priority 5: Binance Data Quality Improvements (for rust_trader)

**Status**: Works but could be more robust

### Improvements needed

1. **Pagination helper**: Add `fetch_ohlcv_range(symbol, tf, since, until) -> Result<Vec<OHLCV>>`
   - Automatically paginates through full date range
   - Handles rate limiting between pages
   - Progress callback for CLI progress bars
   - Location: `src/binance/data.rs` (new file)

2. **Symbol normalization**: Ensure "BTC/USDT" and "BTCUSDT" both work
   - Currently: manual mapping in parsers
   - Improvement: auto-detect format

3. **Futures OHLCV**: Currently auto-detects spot vs futures by symbol suffix
   - Add explicit `params["market"] = "futures"` override

### Estimated effort: 1-2 days

---

## Quick Wins (Do First)

These can be done in a few hours and unblock rust_trader:

1. **`fetch_ohlcv_range` in Binance** (Priority 5, item 1) — enables rust_trader's `download-data` command to work without manual pagination
2. **Timeframe → string helper** — `ccxt::types::Timeframe::as_str()` already exists, just make sure it's public and re-exported from prelude
3. **Fix dead_code warning** in `src/binance/ws_connection.rs` — one pre-existing warning that appears when rust_trader builds with `--features live-data`

---

## Implementation Order Recommendation

```
Week 1: Quick wins + Uniswap V3 swap (Priority 1)
Week 2: Uniswap V3 swap testing + alloy upgrade (Priority 3)
Week 3: dYdX v4 OHLCV + basic order support (Priority 4)
Week 4: Uniswap V4 research + prototype (Priority 2)
```

---

## Grant Alignment

| Work Item | Grant Target | Value |
|-----------|-------------|-------|
| Uniswap V3 swaps | Arbitrum UAGP, Optimism RetroPGF | Core DEX execution |
| Uniswap V4 | Arbitrum UAGP (V4 tooling) | Differentiated |
| dYdX v4 | dYdX Grants ($50K-$200K) | Specific target |
| Hyperliquid (done ✅) | Hyperliquid grants | Already applicable |

---

## How to Use This Plan

Open a Claude Code session in this directory (`/Users/avik/git_projects/github/ccxt-rs/`) and reference this file. The highest-impact first task is Priority 1 (Uniswap V3 swap execution) — it unlocks DEX trading in rust_trader and is the foundation for the Arbitrum grant application.

Start with:
```
Read this plan and implement Priority 5 item 1 (fetch_ohlcv_range in Binance) as a warm-up,
then tackle Priority 1 (Uniswap V3 create_order).
```
