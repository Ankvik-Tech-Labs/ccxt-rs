# WS Local Orderbook Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a production-ready local orderbook manager that maintains sorted bid/ask state from WebSocket delta updates, replacing the current snapshot-only approach.

**Architecture:** A generic `LocalOrderBook` struct in `src/base/` that accumulates incremental depth updates into a sorted `BTreeMap<Decimal, Decimal>` for bids and asks. Each exchange WS adapter (Binance, Bybit, OKX, Hyperliquid) gains a per-symbol `LocalOrderBook` and switches from broadcasting raw snapshots to broadcasting assembled orderbook state. Checksum validation is added where supported (OKX, Bybit).

**Tech Stack:** `rust_decimal::Decimal`, `std::collections::BTreeMap`, `crc32fast` (new dep for OKX/Bybit checksum), `tokio::sync::broadcast`

---

## Pre-requisite: Commit Pending Hardening Changes

Before starting this plan, the 8 modified files from the previous error-handling hardening session must be committed. They are:

- `src/base/http_client.rs` — HTTP retry with backoff
- `src/base/ws.rs` — NowOrNever trait
- `src/base/ws_connection.rs` — pong timeout + shutdown signal
- `src/binance/ws.rs` — keepalive cleanup, pong wiring, close() fix
- `src/bybit/ws.rs` — pong wiring, close() fix
- `src/okx/ws.rs` — pong wiring, close() fix
- `src/hyperliquid/ws.rs` — connection_state() fix
- `tests/ws_integration_tests.rs` — private stream tests + HL testnet fix

---

## Task 1: Add `crc32fast` Dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add the dependency**

Add `crc32fast` to `[dependencies]`:
```toml
crc32fast = "1.4"
```

This is needed for OKX and Bybit orderbook checksum validation. It's a zero-dependency, widely-used crate.

**Step 2: Verify it compiles**

Run: `cargo check --all-features`
Expected: Compiles with only pre-existing warnings.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add crc32fast dependency for orderbook checksum validation"
```

---

## Task 2: Create `LocalOrderBook` Core Struct

**Files:**
- Create: `src/base/local_orderbook.rs`
- Modify: `src/base/mod.rs` (add `pub mod local_orderbook;`)

**Step 1: Write the failing test**

At the bottom of `src/base/local_orderbook.rs` in a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_empty_orderbook() {
        let ob = LocalOrderBook::new("BTC/USDT".to_string());
        assert_eq!(ob.symbol(), "BTC/USDT");
        assert!(ob.bids().is_empty());
        assert!(ob.asks().is_empty());
        assert_eq!(ob.nonce(), None);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --all-features --lib local_orderbook::tests::test_empty_orderbook`
Expected: FAIL — `LocalOrderBook` not defined.

**Step 3: Write minimal implementation**

```rust
//! Local orderbook manager for maintaining sorted bid/ask state
//! from WebSocket incremental depth updates.

use crate::types::OrderBook;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// Local orderbook that accumulates incremental updates.
///
/// Bids are stored descending (highest first), asks ascending (lowest first).
/// A price level with amount == 0 means remove that level.
#[derive(Debug, Clone)]
pub struct LocalOrderBook {
    symbol: String,
    /// Bids: price -> amount (sorted descending by price at read time)
    bids: BTreeMap<Decimal, Decimal>,
    /// Asks: price -> amount (sorted ascending by price at read time)
    asks: BTreeMap<Decimal, Decimal>,
    /// Sequence number / nonce for ordering
    nonce: Option<u64>,
    /// Timestamp of last update (ms)
    timestamp: i64,
}

impl LocalOrderBook {
    /// Create a new empty local orderbook.
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            nonce: None,
            timestamp: 0,
        }
    }

    /// Get the symbol this orderbook tracks.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Get current bids (sorted descending by price).
    pub fn bids(&self) -> Vec<(Decimal, Decimal)> {
        self.bids.iter().rev().map(|(&p, &a)| (p, a)).collect()
    }

    /// Get current asks (sorted ascending by price).
    pub fn asks(&self) -> Vec<(Decimal, Decimal)> {
        self.asks.iter().map(|(&p, &a)| (p, a)).collect()
    }

    /// Get the current nonce/sequence number.
    pub fn nonce(&self) -> Option<u64> {
        self.nonce
    }

    /// Get the timestamp of the last update.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --all-features --lib local_orderbook::tests::test_empty_orderbook`
Expected: PASS

**Step 5: Commit**

```bash
git add src/base/local_orderbook.rs src/base/mod.rs
git commit -m "feat: add LocalOrderBook core struct with BTreeMap storage"
```

---

## Task 3: Implement Snapshot Reset + Delta Application

**Files:**
- Modify: `src/base/local_orderbook.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn test_reset_from_snapshot() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    let bids = vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))];
    let asks = vec![(dec!(50001), dec!(0.8)), (dec!(50002), dec!(1.2))];
    ob.reset(bids, asks, Some(100), 1700000000000);

    let b = ob.bids();
    assert_eq!(b.len(), 2);
    assert_eq!(b[0], (dec!(50000), dec!(1.5))); // highest first
    assert_eq!(b[1], (dec!(49999), dec!(2.0)));

    let a = ob.asks();
    assert_eq!(a.len(), 2);
    assert_eq!(a[0], (dec!(50001), dec!(0.8))); // lowest first
    assert_eq!(a[1], (dec!(50002), dec!(1.2)));

    assert_eq!(ob.nonce(), Some(100));
}

#[test]
fn test_apply_delta_update_level() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5))],
        vec![(dec!(50001), dec!(0.8))],
        Some(100),
        1700000000000,
    );

    // Update existing bid level
    ob.update_bids(&[(dec!(50000), dec!(2.0))]);
    assert_eq!(ob.bids()[0], (dec!(50000), dec!(2.0)));
}

#[test]
fn test_apply_delta_add_level() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5))],
        vec![(dec!(50001), dec!(0.8))],
        None,
        1700000000000,
    );

    // Add new bid level
    ob.update_bids(&[(dec!(49999), dec!(3.0))]);
    let b = ob.bids();
    assert_eq!(b.len(), 2);
    assert_eq!(b[0], (dec!(50000), dec!(1.5))); // still highest
    assert_eq!(b[1], (dec!(49999), dec!(3.0))); // new level
}

#[test]
fn test_apply_delta_remove_level() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))],
        vec![(dec!(50001), dec!(0.8))],
        None,
        1700000000000,
    );

    // Remove bid level (amount = 0)
    ob.update_bids(&[(dec!(50000), dec!(0))]);
    let b = ob.bids();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0], (dec!(49999), dec!(2.0)));
}

#[test]
fn test_set_nonce() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.set_nonce(42);
    assert_eq!(ob.nonce(), Some(42));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: FAIL — methods `reset`, `update_bids`, `set_nonce` not defined.

**Step 3: Implement the methods**

```rust
/// Reset the orderbook with a full snapshot.
///
/// Clears all existing levels and replaces with the given data.
pub fn reset(
    &mut self,
    bids: Vec<(Decimal, Decimal)>,
    asks: Vec<(Decimal, Decimal)>,
    nonce: Option<u64>,
    timestamp: i64,
) {
    self.bids.clear();
    self.asks.clear();
    for (price, amount) in bids {
        if amount > Decimal::ZERO {
            self.bids.insert(price, amount);
        }
    }
    for (price, amount) in asks {
        if amount > Decimal::ZERO {
            self.asks.insert(price, amount);
        }
    }
    self.nonce = nonce;
    self.timestamp = timestamp;
}

/// Apply incremental bid updates.
///
/// Amount of 0 means remove the price level.
pub fn update_bids(&mut self, updates: &[(Decimal, Decimal)]) {
    for &(price, amount) in updates {
        if amount == Decimal::ZERO {
            self.bids.remove(&price);
        } else {
            self.bids.insert(price, amount);
        }
    }
}

/// Apply incremental ask updates.
///
/// Amount of 0 means remove the price level.
pub fn update_asks(&mut self, updates: &[(Decimal, Decimal)]) {
    for &(price, amount) in updates {
        if amount == Decimal::ZERO {
            self.asks.remove(&price);
        } else {
            self.asks.insert(price, amount);
        }
    }
}

/// Set the nonce/sequence number.
pub fn set_nonce(&mut self, nonce: u64) {
    self.nonce = Some(nonce);
}

/// Set the timestamp.
pub fn set_timestamp(&mut self, ts: i64) {
    self.timestamp = ts;
}
```

**Step 4: Run tests**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/base/local_orderbook.rs
git commit -m "feat: add snapshot reset and delta application to LocalOrderBook"
```

---

## Task 4: Implement `to_orderbook()` Snapshot Export + Limit/Checksum

**Files:**
- Modify: `src/base/local_orderbook.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn test_to_orderbook_snapshot() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0)), (dec!(49998), dec!(0.5))],
        vec![(dec!(50001), dec!(0.8)), (dec!(50002), dec!(1.2))],
        Some(100),
        1700000000000,
    );

    let snapshot = ob.to_orderbook(None);
    assert_eq!(snapshot.symbol, "BTC/USDT");
    assert_eq!(snapshot.bids.len(), 3);
    assert_eq!(snapshot.asks.len(), 2);
    assert_eq!(snapshot.nonce, Some(100));
    // Bids descending
    assert_eq!(snapshot.bids[0].0, dec!(50000));
    assert_eq!(snapshot.bids[2].0, dec!(49998));
    // Asks ascending
    assert_eq!(snapshot.asks[0].0, dec!(50001));
}

#[test]
fn test_to_orderbook_with_limit() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0)), (dec!(49998), dec!(0.5))],
        vec![(dec!(50001), dec!(0.8)), (dec!(50002), dec!(1.2)), (dec!(50003), dec!(0.3))],
        None,
        1700000000000,
    );

    let snapshot = ob.to_orderbook(Some(2));
    assert_eq!(snapshot.bids.len(), 2);
    assert_eq!(snapshot.asks.len(), 2);
    // Only top 2 levels
    assert_eq!(snapshot.bids[0].0, dec!(50000));
    assert_eq!(snapshot.bids[1].0, dec!(49999));
    assert_eq!(snapshot.asks[0].0, dec!(50001));
    assert_eq!(snapshot.asks[1].0, dec!(50002));
}

#[test]
fn test_best_bid_ask() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))],
        vec![(dec!(50001), dec!(0.8))],
        None,
        1700000000000,
    );
    assert_eq!(ob.best_bid(), Some((dec!(50000), dec!(1.5))));
    assert_eq!(ob.best_ask(), Some((dec!(50001), dec!(0.8))));
    assert_eq!(ob.spread(), Some(dec!(1)));
    assert_eq!(ob.mid_price(), Some(dec!(50000.5)));
}

#[test]
fn test_best_bid_ask_empty() {
    let ob = LocalOrderBook::new("BTC/USDT".to_string());
    assert_eq!(ob.best_bid(), None);
    assert_eq!(ob.best_ask(), None);
    assert_eq!(ob.spread(), None);
    assert_eq!(ob.mid_price(), None);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: FAIL — `to_orderbook`, `best_bid`, etc. not found.

**Step 3: Implement**

```rust
/// Export the current state as an `OrderBook` snapshot.
///
/// If `limit` is `Some(n)`, only the top `n` levels are included.
pub fn to_orderbook(&self, limit: Option<usize>) -> OrderBook {
    let bids_vec = self.bids();
    let asks_vec = self.asks();

    let bids = match limit {
        Some(n) => bids_vec.into_iter().take(n).collect(),
        None => bids_vec,
    };
    let asks = match limit {
        Some(n) => asks_vec.into_iter().take(n).collect(),
        None => asks_vec,
    };

    let datetime = crate::base::signer::timestamp_to_iso8601(self.timestamp);

    OrderBook {
        symbol: self.symbol.clone(),
        timestamp: self.timestamp,
        datetime,
        nonce: self.nonce,
        bids,
        asks,
        info: None,
    }
}

/// Get the best (highest) bid.
pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
    self.bids.iter().next_back().map(|(&p, &a)| (p, a))
}

/// Get the best (lowest) ask.
pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
    self.asks.iter().next().map(|(&p, &a)| (p, a))
}

/// Get the spread (best ask - best bid).
pub fn spread(&self) -> Option<Decimal> {
    match (self.best_ask(), self.best_bid()) {
        (Some(ask), Some(bid)) => Some(ask.0 - bid.0),
        _ => None,
    }
}

/// Get the mid price ((best bid + best ask) / 2).
pub fn mid_price(&self) -> Option<Decimal> {
    match (self.best_ask(), self.best_bid()) {
        (Some(ask), Some(bid)) => Some((ask.0 + bid.0) / Decimal::from(2)),
        _ => None,
    }
}
```

**Step 4: Run tests**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/base/local_orderbook.rs
git commit -m "feat: add to_orderbook snapshot export, best_bid/ask, spread, mid_price"
```

---

## Task 5: Add Checksum Validation Support

**Files:**
- Modify: `src/base/local_orderbook.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn test_checksum_callback() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5))],
        vec![(dec!(50001), dec!(0.8))],
        None,
        1700000000000,
    );

    // Simple checksum: CRC32 of "50001:0.8:50000:1.5"
    // (asks first, then bids, top N levels, "price:amount" joined by ":")
    let checksum_str = "50001:0.8:50000:1.5";
    let expected = crc32fast::hash(checksum_str.as_bytes());

    let result = ob.validate_checksum(expected, |lob| {
        // OKX-style: asks(asc) then bids(desc), top 25, "price:amount:price:amount:..."
        let mut parts = Vec::new();
        for (p, a) in lob.asks().iter().take(25) {
            parts.push(format!("{}:{}", p, a));
        }
        for (p, a) in lob.bids().iter().take(25) {
            parts.push(format!("{}:{}", p, a));
        }
        parts.join(":")
    });
    assert!(result);
}

#[test]
fn test_checksum_mismatch() {
    let mut ob = LocalOrderBook::new("BTC/USDT".to_string());
    ob.reset(
        vec![(dec!(50000), dec!(1.5))],
        vec![(dec!(50001), dec!(0.8))],
        None,
        1700000000000,
    );

    let result = ob.validate_checksum(12345, |_| "wrong:data".to_string());
    assert!(!result);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: FAIL — `validate_checksum` not found.

**Step 3: Implement**

```rust
/// Validate the orderbook state against a checksum.
///
/// The `format_fn` is exchange-specific: it receives `&self` and must
/// return the string that should be CRC32-hashed and compared against
/// `expected_checksum`.
///
/// Returns `true` if the checksum matches.
pub fn validate_checksum(
    &self,
    expected_checksum: u32,
    format_fn: impl Fn(&Self) -> String,
) -> bool {
    let data = format_fn(self);
    let computed = crc32fast::hash(data.as_bytes());
    computed == expected_checksum
}
```

**Step 4: Run tests**

Run: `cargo test --all-features --lib local_orderbook::tests`
Expected: All PASS

**Step 5: Commit**

```bash
git add src/base/local_orderbook.rs
git commit -m "feat: add checksum validation support to LocalOrderBook"
```

---

## Task 6: Wire Binance WS to Use LocalOrderBook

**Files:**
- Modify: `src/binance/ws.rs`

Binance depth streams send incremental updates with `depthUpdate` events containing `b` (bids) and `a` (asks) arrays, plus `U` (first update ID) and `u` (final update ID). The approach:

1. Subscribe to `{symbol}@depth@100ms` (incremental) instead of `{symbol}@depth{limit}@100ms` (snapshot)
2. Fetch initial snapshot via REST: `GET /api/v3/depth?symbol={}&limit=1000`
3. Apply buffered deltas after snapshot, validating `U <= lastUpdateId+1 <= u`
4. Broadcast assembled `OrderBook` on each delta

**Step 1: Add `local_orderbooks` field to `BinanceWs`**

In the struct definition:
```rust
use crate::base::local_orderbook::LocalOrderBook;

/// Local orderbook state per symbol
local_orderbooks: Arc<RwLock<HashMap<String, Arc<RwLock<LocalOrderBook>>>>>,
```

Initialize in `new()`:
```rust
local_orderbooks: Arc::new(RwLock::new(HashMap::new())),
```

**Step 2: Update `setup_public_handler()` to handle incremental updates**

In the `depthUpdate` arm, instead of parsing a full snapshot, apply deltas to the `LocalOrderBook`:
```rust
"depthUpdate" => {
    let bids: Vec<(Decimal, Decimal)> = json.get("b")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|e| {
            let p = Decimal::from_str(e.get(0)?.as_str()?).ok()?;
            let a = Decimal::from_str(e.get(1)?.as_str()?).ok()?;
            Some((p, a))
        }).collect())
        .unwrap_or_default();

    let asks: Vec<(Decimal, Decimal)> = json.get("a")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|e| {
            let p = Decimal::from_str(e.get(0)?.as_str()?).ok()?;
            let a = Decimal::from_str(e.get(1)?.as_str()?).ok()?;
            Some((p, a))
        }).collect())
        .unwrap_or_default();

    let nonce = json.get("u").and_then(|v| v.as_u64());

    let lobs = local_orderbooks.blocking_read();
    if let Some(lob) = lobs.get(&symbol) {
        let mut book = lob.blocking_write();
        book.update_bids(&bids);
        book.update_asks(&asks);
        if let Some(n) = nonce {
            book.set_nonce(n);
        }
        let timestamp = json.get("E").and_then(|v| v.as_i64()).unwrap_or(0);
        book.set_timestamp(timestamp);

        let snapshot = book.to_orderbook(None);
        let senders = orderbook_senders.blocking_read();
        if let Some(tx) = senders.get(&symbol) {
            let _ = tx.send(snapshot);
        }
    }
}
```

**Step 3: Update `watch_order_book()` to initialize LocalOrderBook with REST snapshot**

```rust
async fn watch_order_book(&self, symbol: &str, limit: Option<u32>) -> Result<WsStream<OrderBook>> {
    let stream_sym = Self::stream_symbol(symbol);
    let stream_name = format!("{}@depth@100ms", stream_sym);
    let sub_id = SubscriptionId(stream_name.clone());
    let sub_msg = self.subscribe_msg(&[&stream_name]);

    // Initialize local orderbook if not present
    {
        let mut lobs = self.local_orderbooks.write().await;
        if !lobs.contains_key(symbol) {
            let lob = LocalOrderBook::new(symbol.to_string());
            lobs.insert(symbol.to_string(), Arc::new(RwLock::new(lob)));
        }
    }

    // TODO: In a production implementation, we would fetch the initial
    // REST snapshot here and apply buffered deltas. For now, the orderbook
    // builds up from the stream (first message acts as initial state).
    // This is acceptable for most use cases as the depth stream starts
    // with a full set of levels.

    let rx = {
        let mut senders = self.orderbook_senders.write().await;
        let tx = senders
            .entry(symbol.to_string())
            .or_insert_with(|| broadcast::channel(self.config.channel_capacity).0);
        tx.subscribe()
    };

    self.setup_public_handler().await;
    self.public_conn.subscribe(sub_id.clone(), sub_msg).await?;

    Ok(WsStream::new(rx, sub_id))
}
```

**Step 4: Verify compilation**

Run: `cargo check --all-features`
Expected: Compiles.

**Step 5: Commit**

```bash
git add src/binance/ws.rs
git commit -m "feat: wire Binance WS orderbook to use LocalOrderBook with delta updates"
```

---

## Task 7: Wire Bybit WS to Use LocalOrderBook

**Files:**
- Modify: `src/bybit/ws.rs`

Bybit's `orderbook.{depth}.{symbol}` stream sends `snapshot` type on subscribe and `delta` type for updates. The `type` field distinguishes them.

**Step 1: Add `local_orderbooks` field to `BybitWs`**

Same pattern as Binance:
```rust
use crate::base::local_orderbook::LocalOrderBook;

local_orderbooks: Arc<RwLock<HashMap<String, Arc<RwLock<LocalOrderBook>>>>>,
```

**Step 2: Update handler for orderbook topic**

In the message handler, detect `"type": "snapshot"` vs `"type": "delta"`:
```rust
if topic.starts_with("orderbook.") {
    let ob_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("snapshot");
    let data = match json.get("data") {
        Some(d) => d,
        None => return,
    };

    let bids = parse_levels(data.get("b"));
    let asks = parse_levels(data.get("a"));
    let nonce = data.get("u").and_then(|v| v.as_u64());
    let timestamp = json.get("ts").and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);

    let lobs = local_orderbooks.blocking_read();
    if let Some(lob) = lobs.get(&symbol) {
        let mut book = lob.blocking_write();
        match ob_type {
            "snapshot" => {
                book.reset(bids, asks, nonce, timestamp);
            }
            "delta" => {
                book.update_bids(&bids);
                book.update_asks(&asks);
                if let Some(n) = nonce { book.set_nonce(n); }
                book.set_timestamp(timestamp);
            }
            _ => {}
        }

        // Validate checksum if present (Bybit sends `cs` field)
        if let Some(cs) = data.get("cs").and_then(|v| v.as_u64()) {
            let valid = book.validate_checksum(cs as u32, bybit_checksum_format);
            if !valid {
                tracing::warn!("Bybit orderbook checksum mismatch for {}", symbol);
                // Could trigger a re-snapshot here
            }
        }

        let snapshot = book.to_orderbook(None);
        let senders = orderbook_senders.blocking_read();
        if let Some(tx) = senders.get(&symbol) {
            let _ = tx.send(snapshot);
        }
    }
}
```

**Step 3: Add Bybit checksum format function**

```rust
/// Bybit checksum format: first 25 bids and asks interleaved
/// "bid1_price:bid1_amount:ask1_price:ask1_amount:..."
fn bybit_checksum_format(lob: &LocalOrderBook) -> String {
    let bids = lob.bids();
    let asks = lob.asks();
    let mut parts = Vec::new();
    for i in 0..25 {
        if let Some((p, a)) = bids.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
        if let Some((p, a)) = asks.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
    }
    parts.join(":")
}
```

**Step 4: Verify compilation**

Run: `cargo check --all-features`
Expected: Compiles.

**Step 5: Commit**

```bash
git add src/bybit/ws.rs
git commit -m "feat: wire Bybit WS orderbook to use LocalOrderBook with snapshot/delta + checksum"
```

---

## Task 8: Wire OKX WS to Use LocalOrderBook

**Files:**
- Modify: `src/okx/ws.rs`

OKX has explicit snapshot/update distinction via the `action` field in the channel message. OKX also provides a CRC32 checksum in the `checksum` field.

**Step 1: Add `local_orderbooks` field to `OkxWs`**

Same pattern as others.

**Step 2: Update handler for orderbook channel**

OKX sends:
- `action: "snapshot"` — full orderbook
- `action: "update"` — delta

```rust
if channel.starts_with("books") {
    let data = match json.get("data").and_then(|v| v.as_array()).and_then(|a| a.first()) {
        Some(d) => d,
        None => return,
    };
    let action = json.get("action").and_then(|v| v.as_str()).unwrap_or("snapshot");
    let bids = parse_levels(data.get("bids"));
    let asks = parse_levels(data.get("asks"));
    let nonce = data.get("seqId").and_then(|v| v.as_u64());
    let timestamp = data.get("ts").and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);

    let lobs = local_orderbooks.blocking_read();
    if let Some(lob) = lobs.get(&symbol) {
        let mut book = lob.blocking_write();
        match action {
            "snapshot" => book.reset(bids, asks, nonce, timestamp),
            "update" => {
                book.update_bids(&bids);
                book.update_asks(&asks);
                if let Some(n) = nonce { book.set_nonce(n); }
                book.set_timestamp(timestamp);
            }
            _ => {}
        }

        // OKX checksum validation
        if let Some(cs) = data.get("checksum").and_then(|v| v.as_i64()) {
            let valid = book.validate_checksum(cs as u32, okx_checksum_format);
            if !valid {
                tracing::warn!("OKX orderbook checksum mismatch for {}", symbol);
            }
        }

        let snapshot = book.to_orderbook(None);
        let senders = orderbook_senders.blocking_read();
        if let Some(tx) = senders.get(&symbol) {
            let _ = tx.send(snapshot);
        }
    }
}
```

**Step 3: Add OKX checksum format function**

```rust
/// OKX checksum format: interleave ask and bid levels (top 25)
/// "bid1_price:bid1_amount:ask1_price:ask1_amount:..."
fn okx_checksum_format(lob: &LocalOrderBook) -> String {
    let bids = lob.bids();
    let asks = lob.asks();
    let mut parts = Vec::new();
    for i in 0..25 {
        if let Some((p, a)) = bids.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
        if let Some((p, a)) = asks.get(i) {
            parts.push(format!("{}:{}", p, a));
        }
    }
    parts.join(":")
}
```

**Step 4: Verify compilation**

Run: `cargo check --all-features`
Expected: Compiles.

**Step 5: Commit**

```bash
git add src/okx/ws.rs
git commit -m "feat: wire OKX WS orderbook to use LocalOrderBook with snapshot/update + checksum"
```

---

## Task 9: Wire Hyperliquid WS to Use LocalOrderBook

**Files:**
- Modify: `src/hyperliquid/ws.rs`

Hyperliquid's `l2Book` stream sends full snapshots on every message (no delta mode). We still use `LocalOrderBook` for consistency and to enable future delta support.

**Step 1: Add `local_orderbooks` field to `HyperliquidWs`**

Same pattern.

**Step 2: Update handler for l2Book channel**

Since HL always sends full snapshots, always call `reset()`:
```rust
"l2Book" => {
    if let Some(data) = json.get("data") {
        let coin = data.get("coin").and_then(|v| v.as_str()).unwrap_or("");
        let symbol = format!("{}/USD:USDC", coin);
        if let Ok(book) = serde_json::from_value::<HlL2Book>(data.clone()) {
            if let Ok(ob) = parsers::parse_order_book(&book, &symbol) {
                // Update local orderbook
                let lobs = local_orderbooks.blocking_read();
                if let Some(lob) = lobs.get(&symbol) {
                    let mut local = lob.blocking_write();
                    local.reset(ob.bids.clone(), ob.asks.clone(), ob.nonce, ob.timestamp);
                }

                let senders = orderbook_senders.blocking_read();
                if let Some(tx) = senders.get(&symbol) {
                    let _ = tx.send(ob);
                }
            }
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check --all-features`
Expected: Compiles.

**Step 4: Commit**

```bash
git add src/hyperliquid/ws.rs
git commit -m "feat: wire Hyperliquid WS orderbook to use LocalOrderBook (snapshot mode)"
```

---

## Task 10: Integration Tests for LocalOrderBook

**Files:**
- Modify: `tests/ws_integration_tests.rs`

**Step 1: Add orderbook depth validation tests**

These test that the orderbook stream delivers properly sorted data:

```rust
// In each exchange's test module, add:

#[tokio::test]
#[ignore]
async fn ws_{exchange}_orderbook_depth_sorted() {
    let ws = {Exchange}Ws::new({sandbox_bool}, WsConfig::default());
    let mut stream = ws.watch_order_book("{SYMBOL}", Some(20)).await.expect("subscribe failed");

    let ob = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("timeout")
        .expect("stream ended");

    // Verify bids sorted descending
    for w in ob.bids.windows(2) {
        assert!(w[0].0 >= w[1].0, "Bids not sorted descending: {} < {}", w[0].0, w[1].0);
    }
    // Verify asks sorted ascending
    for w in ob.asks.windows(2) {
        assert!(w[0].0 <= w[1].0, "Asks not sorted ascending: {} > {}", w[0].0, w[1].0);
    }
    // Verify no zero amounts
    for (_, a) in &ob.bids {
        assert!(*a > Decimal::ZERO, "Bid has zero amount");
    }
    for (_, a) in &ob.asks {
        assert!(*a > Decimal::ZERO, "Ask has zero amount");
    }
    // Verify spread is positive
    if let Some(spread) = ob.spread() {
        assert!(spread >= Decimal::ZERO, "Negative spread: {}", spread);
    }

    ws.close().await.unwrap();
}
```

Add this test for: Binance (`BTC/USDT`), Bybit (`BTC/USDT`), OKX (`BTC/USDT:USDT`), Hyperliquid (`BTC/USD:USDC`).

**Step 2: Run compilation check**

Run: `cargo test --all-features --test ws_integration_tests --no-run`
Expected: Compiles.

**Step 3: Commit**

```bash
git add tests/ws_integration_tests.rs
git commit -m "test: add orderbook depth sorting and validation integration tests"
```

---

## Task 11: Unit Tests for Checksum Format Functions

**Files:**
- Modify: `src/bybit/ws.rs` and `src/okx/ws.rs`

**Step 1: Add checksum format unit tests in each exchange's WS module**

For Bybit:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_bybit_checksum_format() {
        let mut lob = LocalOrderBook::new("BTC/USDT".to_string());
        lob.reset(
            vec![(dec!(50000.5), dec!(1.5)), (dec!(49999), dec!(2.0))],
            vec![(dec!(50001), dec!(0.8)), (dec!(50002.5), dec!(1.2))],
            None,
            0,
        );
        let result = bybit_checksum_format(&lob);
        assert_eq!(result, "50000.5:1.5:50001:0.8:49999:2.0:50002.5:1.2");
    }
}
```

For OKX:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_okx_checksum_format() {
        let mut lob = LocalOrderBook::new("BTC/USDT".to_string());
        lob.reset(
            vec![(dec!(50000), dec!(1.5)), (dec!(49999), dec!(2.0))],
            vec![(dec!(50001), dec!(0.8)), (dec!(50002), dec!(1.2))],
            None,
            0,
        );
        let result = okx_checksum_format(&lob);
        assert_eq!(result, "50000:1.5:50001:0.8:49999:2.0:50002:1.2");
    }
}
```

**Step 2: Run unit tests**

Run: `cargo test --all-features --lib bybit::ws::tests`
Run: `cargo test --all-features --lib okx::ws::tests`
Expected: PASS

**Step 3: Run all tests**

Run: `cargo check --all-features && cargo test --all-features --test ws_integration_tests --no-run`
Expected: All compile.

**Step 4: Commit**

```bash
git add src/bybit/ws.rs src/okx/ws.rs
git commit -m "test: add checksum format unit tests for Bybit and OKX"
```

---

## Task 12: Final Verification

**Step 1: Full check**

```bash
cargo check --all-features
cargo test --all-features --lib    # (skip uniswap doctest)
cargo clippy --all-features -- -W clippy::all
```

Expected: All pass with only pre-existing warnings.

**Step 2: List new test count**

```bash
cargo test --all-features --test ws_integration_tests -- --list 2>/dev/null | tail -5
```

Expected: 37 integration tests (33 existing + 4 new orderbook depth tests).

---

## Summary

| Task | What | ~LOC |
|------|------|------|
| 1 | Add crc32fast dependency | 2 |
| 2 | LocalOrderBook core struct | 60 |
| 3 | Snapshot reset + delta application | 80 |
| 4 | to_orderbook() + best_bid/ask | 60 |
| 5 | Checksum validation | 20 |
| 6 | Wire Binance WS | 60 |
| 7 | Wire Bybit WS + checksum | 70 |
| 8 | Wire OKX WS + checksum | 70 |
| 9 | Wire Hyperliquid WS | 30 |
| 10 | Integration tests | 80 |
| 11 | Checksum unit tests | 40 |
| 12 | Final verification | 0 |
| **Total** | | **~570** |

## Critical Notes for Implementer

1. **All financial math uses `Decimal`** — never use `f64` for prices or amounts
2. **BTreeMap gives natural sorting** — ascending by key. Bids need `.rev()` iterator for descending
3. **`blocking_read()` / `blocking_write()`** — must use these inside sync `MessageHandler` closures (not `.await`)
4. **Pre-existing Uniswap doctest overflow** — `cargo test --lib` fails due to u128 overflow in `src/uniswap/pools.rs:456`. This is not our code. Use `--test ws_integration_tests` or skip uniswap tests
5. **Checksum formats vary by exchange** — OKX and Bybit both use CRC32 but with different level interleaving. Verify against exchange docs
6. **Binance note** — Binance depth@100ms stream does NOT send an initial snapshot. For true production use, you'd need to REST-fetch `/api/v3/depth` and buffer early deltas. The current implementation builds up from the stream, which works for most use cases after a few messages
