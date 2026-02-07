//! Parsers for converting Hyperliquid API responses to unified types

use crate::base::errors::{CcxtError, Result};
use crate::base::signer::{timestamp_ms, timestamp_to_iso8601};
use crate::hyperliquid::constants;
use crate::hyperliquid::types::*;
use crate::types::*;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

// ============================================================================
// Symbol conversion
// ============================================================================

/// Convert unified symbol to Hyperliquid internal name.
///
/// "BTC/USD:USDC" → "BTC"
/// "ETH/USD:USDC" → "ETH"
pub fn symbol_to_hyperliquid(unified_symbol: &str) -> Result<String> {
    // Format: BASE/QUOTE:SETTLE → we want BASE
    let base = unified_symbol
        .split('/')
        .next()
        .ok_or_else(|| CcxtError::BadSymbol(format!("Invalid symbol format: {}", unified_symbol)))?;
    Ok(base.to_string())
}

/// Convert Hyperliquid internal name to unified symbol.
///
/// "BTC" → "BTC/USD:USDC"
pub fn symbol_from_hyperliquid(hl_symbol: &str) -> String {
    format!("{}/USD:USDC", hl_symbol)
}

/// Build asset index lookup from meta: "BTC" → 0, "ETH" → 1, ...
pub fn build_asset_index(meta: &HlMeta) -> HashMap<String, u32> {
    meta.universe
        .iter()
        .enumerate()
        .map(|(i, asset)| (asset.name.clone(), i as u32))
        .collect()
}

/// Build reverse asset name lookup: 0 → "BTC", 1 → "ETH", ...
pub fn build_asset_names(meta: &HlMeta) -> HashMap<u32, String> {
    meta.universe
        .iter()
        .enumerate()
        .map(|(i, asset)| (i as u32, asset.name.clone()))
        .collect()
}

// ============================================================================
// Timeframe conversion
// ============================================================================

/// Convert unified Timeframe to Hyperliquid interval string.
pub fn timeframe_to_hyperliquid(tf: Timeframe) -> Result<&'static str> {
    match tf {
        Timeframe::OneMinute => Ok("1m"),
        Timeframe::ThreeMinutes => Ok("3m"),
        Timeframe::FiveMinutes => Ok("5m"),
        Timeframe::FifteenMinutes => Ok("15m"),
        Timeframe::ThirtyMinutes => Ok("30m"),
        Timeframe::OneHour => Ok("1h"),
        Timeframe::TwoHours => Ok("2h"),
        Timeframe::FourHours => Ok("4h"),
        Timeframe::EightHours => Ok("8h"),
        Timeframe::TwelveHours => Ok("12h"),
        Timeframe::OneDay => Ok("1d"),
        Timeframe::ThreeDays => Ok("3d"),
        Timeframe::OneWeek => Ok("1w"),
        Timeframe::OneMonth => Ok("1M"),
        _ => Err(CcxtError::NotSupported(format!(
            "Timeframe {:?} not supported by Hyperliquid",
            tf
        ))),
    }
}

// ============================================================================
// Market parsing
// ============================================================================

/// Parse meta + asset contexts into unified Market list.
pub fn parse_markets(meta: &HlMeta, asset_ctxs: Option<&[HlAssetCtx]>) -> Result<Vec<Market>> {
    let mut markets = Vec::with_capacity(meta.universe.len());

    for (i, asset) in meta.universe.iter().enumerate() {
        let symbol = symbol_from_hyperliquid(&asset.name);
        let taker = Decimal::from_str(constants::DEFAULT_TAKER_FEE)
            .unwrap_or(Decimal::ZERO);
        let maker = Decimal::from_str(constants::DEFAULT_MAKER_FEE)
            .unwrap_or(Decimal::ZERO);

        // Derive price precision from asset context if available
        let price_precision = asset_ctxs
            .and_then(|ctxs| ctxs.get(i))
            .and_then(|ctx| {
                // Count decimal places in the mark price
                if let Some(dot_pos) = ctx.mark_px.find('.') {
                    let decimals = ctx.mark_px.len() - dot_pos - 1;
                    Some(decimals as i32)
                } else {
                    None
                }
            });

        markets.push(Market {
            symbol,
            base: asset.name.clone(),
            quote: "USD".to_string(),
            settle: Some("USDC".to_string()),
            base_id: asset.name.clone(),
            quote_id: "USD".to_string(),
            settle_id: Some("USDC".to_string()),
            market_type: "swap".to_string(),
            spot: false,
            margin: false,
            swap: true,
            future: false,
            option: false,
            active: true,
            contract: Some(true),
            linear: Some(true),
            inverse: Some(false),
            taker: Some(taker),
            maker: Some(maker),
            contract_size: Some(Decimal::ONE),
            expiry: None,
            expiry_datetime: None,
            strike: None,
            option_type: None,
            created: None,
            margin_modes: None,
            precision: MarketPrecision {
                amount: Some(asset.sz_decimals as i32),
                price: price_precision,
                cost: None,
                base: None,
                quote: None,
            },
            limits: MarketLimits {
                leverage: Some(MinMax {
                    min: Some(Decimal::ONE),
                    max: Some(Decimal::from(asset.max_leverage)),
                }),
                amount: None,
                price: None,
                cost: None,
            },
            info: None,
        });
    }

    Ok(markets)
}

// ============================================================================
// Ticker parsing
// ============================================================================

/// Parse allMids response into a list of Tickers.
pub fn parse_tickers(mids: &serde_json::Value, meta: &HlMeta) -> Result<Vec<Ticker>> {
    let mids_obj = mids
        .as_object()
        .ok_or_else(|| CcxtError::ParseError("allMids response is not an object".to_string()))?;

    let now = timestamp_ms();
    let datetime = timestamp_to_iso8601(now);
    let mut tickers = Vec::new();

    for asset in &meta.universe {
        if let Some(mid_val) = mids_obj.get(&asset.name) {
            if let Some(mid_str) = mid_val.as_str() {
                if let Ok(mid) = Decimal::from_str(mid_str) {
                    let symbol = symbol_from_hyperliquid(&asset.name);
                    tickers.push(Ticker {
                        symbol,
                        timestamp: now,
                        datetime: datetime.clone(),
                        high: None,
                        low: None,
                        bid: None,
                        bid_volume: None,
                        ask: None,
                        ask_volume: None,
                        vwap: None,
                        open: None,
                        close: Some(mid),
                        last: Some(mid),
                        previous_close: None,
                        change: None,
                        percentage: None,
                        average: None,
                        base_volume: None,
                        quote_volume: None,
                        index_price: None,
                        mark_price: None,
                        info: None,
                    });
                }
            }
        }
    }

    Ok(tickers)
}

/// Parse a single ticker from allMids for a specific symbol.
pub fn parse_ticker(mids: &serde_json::Value, hl_symbol: &str) -> Result<Ticker> {
    let mid_str = mids
        .get(hl_symbol)
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            CcxtError::BadSymbol(format!("Symbol {} not found in allMids", hl_symbol))
        })?;

    let mid = Decimal::from_str(mid_str)
        .map_err(|e| CcxtError::ParseError(format!("Invalid mid price: {}", e)))?;

    let now = timestamp_ms();
    let symbol = symbol_from_hyperliquid(hl_symbol);

    Ok(Ticker {
        symbol,
        timestamp: now,
        datetime: timestamp_to_iso8601(now),
        high: None,
        low: None,
        bid: None,
        bid_volume: None,
        ask: None,
        ask_volume: None,
        vwap: None,
        open: None,
        close: Some(mid),
        last: Some(mid),
        previous_close: None,
        change: None,
        percentage: None,
        average: None,
        base_volume: None,
        quote_volume: None,
        index_price: None,
        mark_price: None,
        info: None,
    })
}

/// Parse a ticker enriched with metaAndAssetCtxs data.
pub fn parse_ticker_with_ctx(
    hl_symbol: &str,
    ctx: &HlAssetCtx,
) -> Result<Ticker> {
    let mark = Decimal::from_str(&ctx.mark_px)
        .map_err(|e| CcxtError::ParseError(format!("Invalid mark price: {}", e)))?;

    let prev_day = Decimal::from_str(&ctx.prev_day_px).ok();
    let volume = Decimal::from_str(&ctx.day_ntl_vlm).ok();

    let change = prev_day.map(|p| mark - p);
    let percentage = prev_day.and_then(|p| {
        if p.is_zero() {
            None
        } else {
            Some((mark - p) / p * Decimal::from(100))
        }
    });

    let now = timestamp_ms();
    let symbol = symbol_from_hyperliquid(hl_symbol);

    Ok(Ticker {
        symbol,
        timestamp: now,
        datetime: timestamp_to_iso8601(now),
        high: None,
        low: None,
        bid: None,
        bid_volume: None,
        ask: None,
        ask_volume: None,
        vwap: None,
        open: prev_day,
        close: Some(mark),
        last: Some(mark),
        previous_close: prev_day,
        change,
        percentage,
        average: None,
        base_volume: None,
        quote_volume: volume,
        index_price: None,
        mark_price: None,
        info: None,
    })
}

// ============================================================================
// Order book parsing
// ============================================================================

/// Parse l2Book response into unified OrderBook.
pub fn parse_order_book(book: &HlL2Book, symbol: &str) -> Result<OrderBook> {
    if book.levels.len() < 2 {
        return Err(CcxtError::ParseError(
            "L2 book must have bid and ask arrays".to_string(),
        ));
    }

    let bids = parse_book_side(&book.levels[0])?;
    let asks = parse_book_side(&book.levels[1])?;

    Ok(OrderBook {
        symbol: symbol_from_hyperliquid(symbol),
        timestamp: book.time as i64,
        datetime: timestamp_to_iso8601(book.time as i64),
        nonce: None,
        bids,
        asks,
        info: None,
    })
}

fn parse_book_side(levels: &[HlLevel]) -> Result<Vec<OrderBookEntry>> {
    levels
        .iter()
        .map(|level| {
            let price = Decimal::from_str(&level.px)
                .map_err(|e| CcxtError::ParseError(format!("Invalid price: {}", e)))?;
            let size = Decimal::from_str(&level.sz)
                .map_err(|e| CcxtError::ParseError(format!("Invalid size: {}", e)))?;
            Ok((price, size))
        })
        .collect()
}

// ============================================================================
// OHLCV parsing
// ============================================================================

/// Parse candleSnapshot response into unified OHLCV list.
pub fn parse_ohlcv(candles: &[HlCandle]) -> Result<Vec<OHLCV>> {
    candles
        .iter()
        .map(|c| {
            Ok(OHLCV {
                timestamp: c.t as i64,
                open: parse_decimal(&c.o, "open")?,
                high: parse_decimal(&c.h, "high")?,
                low: parse_decimal(&c.l, "low")?,
                close: parse_decimal(&c.c, "close")?,
                volume: parse_decimal(&c.v, "volume")?,
                info: None,
            })
        })
        .collect()
}

// ============================================================================
// Trade parsing
// ============================================================================

/// Parse recentTrades response into unified Trade list.
pub fn parse_trades(trades: &[HlRecentTrade], unified_symbol: &str) -> Result<Vec<Trade>> {
    trades
        .iter()
        .map(|t| {
            let price = parse_decimal(&t.px, "price")?;
            let amount = parse_decimal(&t.sz, "size")?;
            let cost = price * amount;
            let side = parse_side(&t.side)?;

            Ok(Trade {
                id: t.tid.to_string(),
                symbol: unified_symbol.to_string(),
                order: None,
                timestamp: t.time as i64,
                datetime: timestamp_to_iso8601(t.time as i64),
                side,
                price,
                amount,
                cost,
                fee: None,
                taker_or_maker: None,
                info: None,
            })
        })
        .collect()
}

/// Parse userFills response into unified Trade list.
pub fn parse_user_fills(fills: &[HlUserFill]) -> Result<Vec<Trade>> {
    fills
        .iter()
        .map(|f| {
            let price = parse_decimal(&f.px, "price")?;
            let amount = parse_decimal(&f.sz, "size")?;
            let cost = price * amount;
            let side = parse_side(&f.side)?;
            let fee_cost = parse_decimal(&f.fee, "fee")?;
            let symbol = symbol_from_hyperliquid(&f.coin);

            Ok(Trade {
                id: f.tid.to_string(),
                symbol,
                order: Some(f.oid.to_string()),
                timestamp: f.time as i64,
                datetime: timestamp_to_iso8601(f.time as i64),
                side,
                price,
                amount,
                cost,
                fee: Some(TradeFee {
                    cost: fee_cost,
                    currency: f.fee_token.clone(),
                    rate: None,
                }),
                taker_or_maker: f.crossed.map(|c| {
                    if c {
                        "taker".to_string()
                    } else {
                        "maker".to_string()
                    }
                }),
                info: None,
            })
        })
        .collect()
}

// ============================================================================
// Position parsing
// ============================================================================

/// Parse clearinghouseState into unified Position list.
pub fn parse_positions(state: &HlClearinghouseState) -> Result<Vec<Position>> {
    let now = timestamp_ms();
    let datetime = timestamp_to_iso8601(now);

    state
        .asset_positions
        .iter()
        .filter_map(|wrapper| {
            let pos = &wrapper.position;
            let szi = Decimal::from_str(&pos.szi).ok()?;
            if szi.is_zero() {
                return None;
            }

            let side = if szi > Decimal::ZERO {
                PositionSide::Long
            } else {
                PositionSide::Short
            };

            let margin_mode = if pos.leverage.leverage_type == "isolated" {
                MarginMode::Isolated
            } else {
                MarginMode::Cross
            };

            let symbol = symbol_from_hyperliquid(&pos.coin);

            Some(Ok(Position {
                symbol,
                id: None,
                timestamp: now,
                datetime: datetime.clone(),
                side,
                margin_mode,
                contracts: szi.abs(),
                contract_size: Some(Decimal::ONE),
                notional: Decimal::from_str(&pos.position_value).ok(),
                leverage: Some(Decimal::from(pos.leverage.value)),
                entry_price: pos.entry_px.as_ref().and_then(|p| Decimal::from_str(p).ok()),
                mark_price: None,
                unrealized_pnl: Decimal::from_str(&pos.unrealized_pnl).ok(),
                realized_pnl: None,
                collateral: None,
                initial_margin: Decimal::from_str(&pos.margin_used).ok(),
                maintenance_margin: None,
                liquidation_price: pos
                    .liquidation_px
                    .as_ref()
                    .and_then(|p| Decimal::from_str(p).ok()),
                margin_ratio: None,
                percentage: Decimal::from_str(&pos.return_on_equity).ok().map(|roe| {
                    roe * Decimal::from(100)
                }),
                stop_loss_price: None,
                take_profit_price: None,
                hedged: None,
                info: None,
            }))
        })
        .collect()
}

// ============================================================================
// Balance parsing
// ============================================================================

/// Parse clearinghouseState into unified Balances.
pub fn parse_balances(state: &HlClearinghouseState) -> Result<Balances> {
    let now = timestamp_ms();

    let account_value = parse_decimal(&state.margin_summary.account_value, "accountValue")?;
    let withdrawable = parse_decimal(&state.withdrawable, "withdrawable")?;
    let used = account_value - withdrawable;

    let mut balances = HashMap::new();
    balances.insert(
        "USDC".to_string(),
        Balance::new("USDC".to_string(), withdrawable, used),
    );

    Ok(Balances {
        timestamp: now,
        datetime: timestamp_to_iso8601(now),
        balances,
        info: None,
    })
}

// ============================================================================
// Order parsing
// ============================================================================

/// Parse openOrders response into unified Order list.
pub fn parse_open_orders(orders: &[HlOpenOrder]) -> Result<Vec<Order>> {
    orders
        .iter()
        .map(|o| {
            let side = parse_side(&o.side)?;
            let price = parse_decimal(&o.limit_px, "limitPx")?;
            let amount = parse_decimal(&o.sz, "sz")?;
            let symbol = symbol_from_hyperliquid(&o.coin);

            Ok(Order {
                id: o.oid.to_string(),
                client_order_id: None,
                symbol,
                order_type: OrderType::Limit,
                side,
                status: OrderStatus::Open,
                timestamp: o.timestamp as i64,
                datetime: timestamp_to_iso8601(o.timestamp as i64),
                last_trade_timestamp: None,
                price: Some(price),
                average: None,
                amount,
                filled: None,
                remaining: Some(amount),
                cost: None,
                fee: None,
                time_in_force: None,
                post_only: None,
                reduce_only: None,
                stop_price: None,
                trigger_price: None,
                stop_loss_price: None,
                take_profit_price: None,
                last_update_timestamp: None,
                trades: None,
                info: None,
            })
        })
        .collect()
}

/// Parse orderStatus response into a unified Order.
pub fn parse_order_status(status_resp: &HlOrderStatusResponse) -> Result<Order> {
    let order = &status_resp.order;
    let side = parse_side(&order.side)?;
    let price = parse_decimal(&order.limit_px, "limitPx")?;
    let sz = parse_decimal(&order.sz, "sz")?;
    let symbol = symbol_from_hyperliquid(&order.coin);

    let orig_sz = order
        .orig_sz
        .as_ref()
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or(sz);

    let status = parse_order_status_string(&status_resp.status);
    let filled = if sz < orig_sz { Some(orig_sz - sz) } else { None };

    let order_type = match order.order_type.as_deref() {
        Some("Market") => OrderType::Market,
        _ => OrderType::Limit,
    };

    let tif = order.tif.as_deref().map(parse_time_in_force);
    let reduce_only = order.reduce_only;

    Ok(Order {
        id: order.oid.to_string(),
        client_order_id: order.cloid.clone(),
        symbol,
        order_type,
        side,
        status,
        timestamp: order.timestamp as i64,
        datetime: timestamp_to_iso8601(order.timestamp as i64),
        last_trade_timestamp: Some(status_resp.status_timestamp as i64),
        price: Some(price),
        average: None,
        amount: orig_sz,
        filled,
        remaining: Some(sz),
        cost: None,
        fee: None,
        time_in_force: tif,
        post_only: tif.map(|t| t == TimeInForce::PostOnly),
        reduce_only,
        stop_price: None,
        trigger_price: None,
        stop_loss_price: None,
        take_profit_price: None,
        last_update_timestamp: None,
        trades: None,
        info: None,
    })
}

/// Parse order placement response into an Order.
pub fn parse_order_response(
    status_entry: &HlOrderStatusEntry,
    symbol: &str,
    side: OrderSide,
    order_type: OrderType,
    amount: Decimal,
    price: Option<Decimal>,
) -> Result<Order> {
    let now = timestamp_ms();

    match status_entry {
        HlOrderStatusEntry::Resting { resting } => Ok(Order {
            id: resting.oid.to_string(),
            client_order_id: None,
            symbol: symbol.to_string(),
            order_type,
            side,
            status: OrderStatus::Open,
            timestamp: now,
            datetime: timestamp_to_iso8601(now),
            last_trade_timestamp: None,
            price,
            average: None,
            amount,
            filled: Some(Decimal::ZERO),
            remaining: Some(amount),
            cost: None,
            fee: None,
            time_in_force: None,
            post_only: None,
            reduce_only: None,
            stop_price: None,
            trigger_price: None,
            stop_loss_price: None,
            take_profit_price: None,
            last_update_timestamp: None,
            trades: None,
            info: None,
        }),
        HlOrderStatusEntry::Filled { filled } => {
            let avg_px = Decimal::from_str(&filled.avg_px).ok();
            let total_sz = Decimal::from_str(&filled.total_sz).ok().unwrap_or(amount);
            let cost = avg_px.map(|p| p * total_sz);

            Ok(Order {
                id: filled.oid.to_string(),
                client_order_id: None,
                symbol: symbol.to_string(),
                order_type,
                side,
                status: OrderStatus::Closed,
                timestamp: now,
                datetime: timestamp_to_iso8601(now),
                last_trade_timestamp: Some(now),
                price,
                average: avg_px,
                amount,
                filled: Some(total_sz),
                remaining: Some(Decimal::ZERO),
                cost,
                fee: None,
                time_in_force: None,
                post_only: None,
                reduce_only: None,
                stop_price: None,
                trigger_price: None,
                stop_loss_price: None,
                take_profit_price: None,
                last_update_timestamp: None,
                trades: None,
                info: None,
            })
        }
        HlOrderStatusEntry::Error { error } => {
            Err(CcxtError::InvalidOrder(error.clone()))
        }
    }
}

// ============================================================================
// Funding rate parsing
// ============================================================================

/// Parse fundingHistory response into a unified FundingRate.
pub fn parse_funding_rate(entries: &[HlFundingEntry], unified_symbol: &str) -> Result<FundingRate> {
    let entry = entries
        .last()
        .ok_or_else(|| CcxtError::ParseError("Empty funding history".to_string()))?;

    let rate = parse_decimal(&entry.funding_rate, "fundingRate")?;

    Ok(FundingRate {
        symbol: unified_symbol.to_string(),
        timestamp: entry.time as i64,
        datetime: timestamp_to_iso8601(entry.time as i64),
        funding_rate: rate,
        funding_timestamp: None,
        funding_datetime: None,
        mark_price: None,
        index_price: None,
        interest_rate: None,
        estimated_settle_price: None,
        interval: None,
        previous_funding_rate: None,
        previous_funding_timestamp: None,
        previous_funding_datetime: None,
        next_funding_rate: None,
        next_funding_timestamp: None,
        next_funding_datetime: None,
        info: None,
    })
}

/// Parse funding rate from asset context (live data).
pub fn parse_funding_rate_from_ctx(
    ctx: &HlAssetCtx,
    unified_symbol: &str,
) -> Result<FundingRate> {
    let rate = parse_decimal(&ctx.funding, "funding")?;
    let mark = Decimal::from_str(&ctx.mark_px).ok();
    let oracle = Decimal::from_str(&ctx.oracle_px).ok();
    let now = timestamp_ms();

    Ok(FundingRate {
        symbol: unified_symbol.to_string(),
        timestamp: now,
        datetime: timestamp_to_iso8601(now),
        funding_rate: rate,
        funding_timestamp: None,
        funding_datetime: None,
        mark_price: mark,
        index_price: oracle,
        interest_rate: None,
        estimated_settle_price: None,
        interval: None,
        previous_funding_rate: None,
        previous_funding_timestamp: None,
        previous_funding_datetime: None,
        next_funding_rate: None,
        next_funding_timestamp: None,
        next_funding_datetime: None,
        info: None,
    })
}

// ============================================================================
// Helper functions
// ============================================================================

fn parse_decimal(s: &str, field_name: &str) -> Result<Decimal> {
    Decimal::from_str(s)
        .map_err(|e| CcxtError::ParseError(format!("Invalid {} '{}': {}", field_name, s, e)))
}

fn parse_side(side: &str) -> Result<OrderSide> {
    match side {
        "B" | "b" | "Buy" | "buy" => Ok(OrderSide::Buy),
        "A" | "a" | "Sell" | "sell" => Ok(OrderSide::Sell),
        _ => Err(CcxtError::ParseError(format!("Unknown side: {}", side))),
    }
}

fn parse_order_status_string(status: &str) -> OrderStatus {
    match status {
        "open" => OrderStatus::Open,
        "filled" | "triggered" => OrderStatus::Closed,
        "canceled" | "marginCanceled" | "vaultWithdrawalCanceled"
        | "openInterestCapCanceled" | "selfTradeCanceled" | "reduceOnlyCanceled"
        | "siblingFilledCanceled" | "delistedCanceled" | "liquidatedCanceled"
        | "scheduledCancel" => OrderStatus::Canceled,
        "rejected" | "tickRejected" | "minTradeNtlRejected" | "perpMarginRejected"
        | "reduceOnlyRejected" | "badAloPxRejected" | "iocCancelRejected"
        | "badTriggerPxRejected" | "marketOrderNoLiquidityRejected"
        | "positionIncreaseAtOpenInterestCapRejected"
        | "positionFlipAtOpenInterestCapRejected"
        | "tooAggressiveAtOpenInterestCapRejected"
        | "openInterestIncreaseRejected" | "insufficientSpotBalanceRejected"
        | "oracleRejected" | "perpMaxPositionRejected" => OrderStatus::Rejected,
        _ => OrderStatus::Open,
    }
}

fn parse_time_in_force(tif: &str) -> TimeInForce {
    match tif {
        "Gtc" | "GTC" => TimeInForce::Gtc,
        "Ioc" | "IOC" => TimeInForce::Ioc,
        "Alo" | "ALO" => TimeInForce::PostOnly,
        _ => TimeInForce::Gtc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_to_hyperliquid() {
        assert_eq!(symbol_to_hyperliquid("BTC/USD:USDC").unwrap(), "BTC");
        assert_eq!(symbol_to_hyperliquid("ETH/USD:USDC").unwrap(), "ETH");
        assert_eq!(symbol_to_hyperliquid("SOL/USD:USDC").unwrap(), "SOL");
    }

    #[test]
    fn test_symbol_from_hyperliquid() {
        assert_eq!(symbol_from_hyperliquid("BTC"), "BTC/USD:USDC");
        assert_eq!(symbol_from_hyperliquid("ETH"), "ETH/USD:USDC");
    }

    #[test]
    fn test_build_asset_index() {
        let meta = HlMeta {
            universe: vec![
                HlAssetInfo {
                    name: "BTC".to_string(),
                    sz_decimals: 5,
                    max_leverage: 50,
                },
                HlAssetInfo {
                    name: "ETH".to_string(),
                    sz_decimals: 4,
                    max_leverage: 50,
                },
            ],
        };
        let index = build_asset_index(&meta);
        assert_eq!(index.get("BTC"), Some(&0));
        assert_eq!(index.get("ETH"), Some(&1));
    }

    #[test]
    fn test_parse_side() {
        assert_eq!(parse_side("B").unwrap(), OrderSide::Buy);
        assert_eq!(parse_side("A").unwrap(), OrderSide::Sell);
        assert_eq!(parse_side("Buy").unwrap(), OrderSide::Buy);
        assert_eq!(parse_side("Sell").unwrap(), OrderSide::Sell);
        assert!(parse_side("X").is_err());
    }

    #[test]
    fn test_parse_order_status_string() {
        assert_eq!(parse_order_status_string("open"), OrderStatus::Open);
        assert_eq!(parse_order_status_string("filled"), OrderStatus::Closed);
        assert_eq!(parse_order_status_string("canceled"), OrderStatus::Canceled);
        assert_eq!(
            parse_order_status_string("marginCanceled"),
            OrderStatus::Canceled
        );
        assert_eq!(
            parse_order_status_string("rejected"),
            OrderStatus::Rejected
        );
    }

    #[test]
    fn test_parse_balances() {
        let state = HlClearinghouseState {
            asset_positions: vec![],
            margin_summary: HlMarginSummary {
                account_value: "10000.50".to_string(),
                total_margin_used: "500.25".to_string(),
                total_ntl_pos: "5000.00".to_string(),
                total_raw_usd: "9500.25".to_string(),
            },
            cross_margin_summary: None,
            withdrawable: "9500.25".to_string(),
            time: None,
        };

        let balances = parse_balances(&state).unwrap();
        let usdc = balances.balances.get("USDC").unwrap();
        assert_eq!(usdc.free, Decimal::from_str("9500.25").unwrap());
        assert_eq!(usdc.total, Decimal::from_str("10000.50").unwrap());
    }

    #[test]
    fn test_timeframe_to_hyperliquid() {
        assert_eq!(timeframe_to_hyperliquid(Timeframe::OneMinute).unwrap(), "1m");
        assert_eq!(timeframe_to_hyperliquid(Timeframe::OneHour).unwrap(), "1h");
        assert_eq!(timeframe_to_hyperliquid(Timeframe::OneDay).unwrap(), "1d");
    }

    #[test]
    fn test_parse_ticker() {
        let mids = serde_json::json!({
            "BTC": "95000.5",
            "ETH": "3800.25"
        });

        let ticker = parse_ticker(&mids, "BTC").unwrap();
        assert_eq!(ticker.symbol, "BTC/USD:USDC");
        assert_eq!(ticker.last, Some(Decimal::from_str("95000.5").unwrap()));
    }

    #[test]
    fn test_parse_order_book() {
        let book = HlL2Book {
            coin: "BTC".to_string(),
            levels: vec![
                vec![
                    HlLevel {
                        px: "95000.0".to_string(),
                        sz: "1.5".to_string(),
                        n: 3,
                    },
                    HlLevel {
                        px: "94999.0".to_string(),
                        sz: "2.0".to_string(),
                        n: 1,
                    },
                ],
                vec![
                    HlLevel {
                        px: "95001.0".to_string(),
                        sz: "0.5".to_string(),
                        n: 2,
                    },
                    HlLevel {
                        px: "95002.0".to_string(),
                        sz: "3.0".to_string(),
                        n: 4,
                    },
                ],
            ],
            time: 1707000000000,
        };

        let ob = parse_order_book(&book, "BTC").unwrap();
        assert_eq!(ob.symbol, "BTC/USD:USDC");
        assert_eq!(ob.bids.len(), 2);
        assert_eq!(ob.asks.len(), 2);
        assert_eq!(ob.bids[0].0, Decimal::from_str("95000.0").unwrap());
        assert_eq!(ob.asks[0].0, Decimal::from_str("95001.0").unwrap());
    }
}
