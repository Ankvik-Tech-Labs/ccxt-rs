//! OKX API response parsers
//!
//! Convert OKX v5 API responses to unified CCXT types

use crate::base::{
    decimal::parse_decimal,
    errors::{CcxtError, Result},
    signer::timestamp_to_iso8601,
};
use crate::types::*;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;

/// Extract an optional decimal from a JSON string field
fn optional_decimal(json: &Value, key: &str) -> Option<Decimal> {
    json.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .and_then(|s| Decimal::from_str(s).ok())
}

// ============================================================================
// Symbol Conversion
// ============================================================================

/// Convert unified symbol to OKX instId format
///
/// Handles both spot ("BTC/USDT" -> "BTC-USDT") and
/// swap ("BTC/USDT:USDT" -> "BTC-USDT-SWAP") symbols.
pub fn symbol_to_okx(symbol: &str) -> String {
    if symbol.contains(':') {
        // Derivatives: "BTC/USDT:USDT" -> "BTC-USDT-SWAP"
        let base_symbol = symbol.split(':').next().unwrap_or(symbol);
        format!("{}-SWAP", base_symbol.replace('/', "-"))
    } else {
        // Spot: "BTC/USDT" -> "BTC-USDT"
        symbol.replace('/', "-")
    }
}

/// Convert OKX instId to unified symbol format
///
/// Handles both spot ("BTC-USDT" -> "BTC/USDT") and
/// swap ("BTC-USDT-SWAP" -> "BTC/USDT:USDT") symbols.
pub fn symbol_from_okx(okx_symbol: &str) -> String {
    if okx_symbol.ends_with("-SWAP") {
        // Derivatives: "BTC-USDT-SWAP" -> "BTC/USDT:USDT"
        let base = okx_symbol.trim_end_matches("-SWAP");
        let parts: Vec<&str> = base.splitn(2, '-').collect();
        if parts.len() == 2 {
            format!("{}/{}:{}", parts[0], parts[1], parts[1])
        } else {
            okx_symbol.replace('-', "/")
        }
    } else {
        // Spot: "BTC-USDT" -> "BTC/USDT"
        let parts: Vec<&str> = okx_symbol.splitn(2, '-').collect();
        if parts.len() == 2 {
            format!("{}/{}", parts[0], parts[1])
        } else {
            okx_symbol.to_string()
        }
    }
}

/// Check if a unified symbol is a swap/derivatives symbol
pub fn is_swap_symbol(symbol: &str) -> bool {
    symbol.contains(':')
}

/// Get OKX instType for a unified symbol
pub fn inst_type_for_symbol(symbol: &str) -> &'static str {
    if is_swap_symbol(symbol) {
        "SWAP"
    } else {
        "SPOT"
    }
}

/// Convert CCXT timeframe to OKX bar interval
pub fn timeframe_to_okx(timeframe: &Timeframe) -> String {
    match timeframe {
        Timeframe::OneMinute => "1m",
        Timeframe::ThreeMinutes => "3m",
        Timeframe::FiveMinutes => "5m",
        Timeframe::FifteenMinutes => "15m",
        Timeframe::ThirtyMinutes => "30m",
        Timeframe::OneHour => "1H",
        Timeframe::TwoHours => "2H",
        Timeframe::FourHours => "4H",
        Timeframe::SixHours => "6H",
        Timeframe::TwelveHours => "12H",
        Timeframe::OneDay => "1D",
        Timeframe::ThreeDays => "3D",
        Timeframe::OneWeek => "1W",
        Timeframe::OneMonth => "1M",
        _ => "1H",
    }
    .to_string()
}

/// Count decimal places in a string number
pub fn count_decimals(value_str: &str) -> i32 {
    if let Some(dot_pos) = value_str.find('.') {
        value_str[dot_pos + 1..].trim_end_matches('0').len() as i32
    } else {
        0
    }
}

// ============================================================================
// Market Parsers
// ============================================================================

/// Parse OKX spot market info into unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let inst_id = json
        .get("instId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing instId in market".to_string()))?;

    let base_ccy = json
        .get("baseCcy")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing baseCcy".to_string()))?;

    let quote_ccy = json
        .get("quoteCcy")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteCcy".to_string()))?;

    let symbol = format!("{}/{}", base_ccy, quote_ccy);

    let state = json.get("state").and_then(|v| v.as_str()).unwrap_or("live");
    let active = state == "live";

    let lot_sz = json.get("lotSz").and_then(|v| v.as_str());
    let tick_sz = json.get("tickSz").and_then(|v| v.as_str());

    let min_sz = json.get("minSz").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());
    let max_lmt_sz = json.get("maxLmtSz").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());
    let min_order_sz = json.get("minOrderSz").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());

    let amount_precision = lot_sz.map(count_decimals);
    let price_precision = tick_sz.map(count_decimals);

    Ok(Market {
        id: inst_id.to_string(),
        symbol,
        base: base_ccy.to_string(),
        quote: quote_ccy.to_string(),
        settle: None,
        base_id: base_ccy.to_string(),
        quote_id: quote_ccy.to_string(),
        settle_id: None,
        market_type: "spot".to_string(),
        spot: true,
        margin: false,
        swap: false,
        future: false,
        option: false,
        active,
        contract: None,
        linear: None,
        inverse: None,
        taker: None,
        maker: None,
        contract_size: None,
        expiry: None,
        expiry_datetime: None,
        strike: None,
        option_type: None,
        created: None,
        margin_modes: None,
        precision: MarketPrecision {
            price: price_precision,
            amount: amount_precision,
            cost: None,
            base: amount_precision,
            quote: price_precision,
        },
        limits: MarketLimits {
            amount: Some(MinMax { min: min_sz, max: max_lmt_sz }),
            price: Some(MinMax { min: None, max: None }),
            cost: Some(MinMax { min: min_order_sz, max: None }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

/// Parse OKX SWAP market info into unified Market
pub fn parse_swap_market(json: &Value) -> Result<Market> {
    let inst_id = json
        .get("instId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing instId in swap market".to_string()))?;

    // Only handle perpetual swaps (instType=SWAP)
    let inst_type = json.get("instType").and_then(|v| v.as_str()).unwrap_or("");
    if inst_type != "SWAP" {
        return Err(CcxtError::ParseError(format!("Skipping non-SWAP instType: {}", inst_type)));
    }

    // Parse "BTC-USDT-SWAP" -> base="BTC", quote="USDT"
    let parts: Vec<&str> = inst_id.trim_end_matches("-SWAP").splitn(2, '-').collect();
    if parts.len() < 2 {
        return Err(CcxtError::ParseError(format!("Cannot parse instId: {}", inst_id)));
    }
    let base = parts[0];
    let quote = parts[1];

    let settle_ccy = json.get("settleCcy").and_then(|v| v.as_str()).unwrap_or(quote);
    let ct_type = json.get("ctType").and_then(|v| v.as_str()).unwrap_or("linear");

    let symbol = format!("{}/{}:{}", base, quote, settle_ccy);

    let state = json.get("state").and_then(|v| v.as_str()).unwrap_or("live");
    let active = state == "live";

    let lot_sz = json.get("lotSz").and_then(|v| v.as_str());
    let tick_sz = json.get("tickSz").and_then(|v| v.as_str());
    let ct_val = json.get("ctVal").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());
    let min_sz = json.get("minSz").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());
    let max_lmt_sz = json.get("maxLmtSz").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());
    let lever = json.get("lever").and_then(|v| v.as_str()).and_then(|s| parse_decimal(s).ok());

    let amount_precision = lot_sz.map(count_decimals);
    let price_precision = tick_sz.map(count_decimals);

    let is_linear = ct_type == "linear";

    Ok(Market {
        id: inst_id.to_string(),
        symbol,
        base: base.to_string(),
        quote: quote.to_string(),
        settle: Some(settle_ccy.to_string()),
        base_id: base.to_string(),
        quote_id: quote.to_string(),
        settle_id: Some(settle_ccy.to_string()),
        market_type: "swap".to_string(),
        spot: false,
        margin: false,
        swap: true,
        future: false,
        option: false,
        active,
        contract: Some(true),
        linear: Some(is_linear),
        inverse: Some(!is_linear),
        taker: None,
        maker: None,
        contract_size: ct_val,
        expiry: None,
        expiry_datetime: None,
        strike: None,
        option_type: None,
        created: None,
        margin_modes: Some(MarginModes {
            cross: Some(true),
            isolated: Some(true),
        }),
        precision: MarketPrecision {
            price: price_precision,
            amount: amount_precision,
            cost: None,
            base: None,
            quote: None,
        },
        limits: MarketLimits {
            amount: Some(MinMax { min: min_sz, max: max_lmt_sz }),
            price: Some(MinMax { min: None, max: None }),
            cost: Some(MinMax { min: None, max: None }),
            leverage: Some(MinMax { min: Some(Decimal::ONE), max: lever }),
        },
        info: Some(json.clone()),
    })
}

// ============================================================================
// Ticker Parser
// ============================================================================

/// Parse OKX ticker into unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let last = optional_decimal(json, "last");
    let bid = optional_decimal(json, "bidPx");
    let ask = optional_decimal(json, "askPx");
    let high = optional_decimal(json, "high24h");
    let low = optional_decimal(json, "low24h");
    let volume = optional_decimal(json, "vol24h");
    let quote_volume = optional_decimal(json, "volCcy24h");
    let open = optional_decimal(json, "open24h");

    let change = match (last, open) {
        (Some(l), Some(o)) => Some(l - o),
        _ => None,
    };

    let percentage = match (last, open) {
        (Some(l), Some(o)) if o != Decimal::ZERO => Some(((l - o) / o) * Decimal::from(100)),
        _ => None,
    };

    Ok(Ticker {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        high,
        low,
        bid,
        bid_volume: optional_decimal(json, "bidSz"),
        ask,
        ask_volume: optional_decimal(json, "askSz"),
        vwap: None,
        open,
        close: last,
        last,
        previous_close: open,
        change,
        percentage,
        average: None,
        base_volume: volume,
        quote_volume,
        index_price: None,
        mark_price: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// OrderBook Parser
// ============================================================================

/// Parse OKX order book into unified OrderBook
pub fn parse_orderbook(json: &Value, symbol: &str) -> Result<OrderBook> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let bids_array = json
        .get("bids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing bids in orderbook".to_string()))?;

    let asks_array = json
        .get("asks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing asks in orderbook".to_string()))?;

    let bids: Vec<(Decimal, Decimal)> = bids_array
        .iter()
        .filter_map(|item| {
            let arr = item.as_array()?;
            let price = parse_decimal(arr.first()?.as_str()?).ok()?;
            let amount = parse_decimal(arr.get(1)?.as_str()?).ok()?;
            Some((price, amount))
        })
        .collect();

    let asks: Vec<(Decimal, Decimal)> = asks_array
        .iter()
        .filter_map(|item| {
            let arr = item.as_array()?;
            let price = parse_decimal(arr.first()?.as_str()?).ok()?;
            let amount = parse_decimal(arr.get(1)?.as_str()?).ok()?;
            Some((price, amount))
        })
        .collect();

    Ok(OrderBook {
        symbol: symbol.to_string(),
        bids,
        asks,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        nonce: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// OHLCV Parser
// ============================================================================

/// Parse OKX OHLCV candle into unified OHLCV
pub fn parse_ohlcv(json: &Value) -> Result<OHLCV> {
    let arr = json
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("OHLCV data is not an array".to_string()))?;

    if arr.len() < 6 {
        return Err(CcxtError::ParseError("OHLCV array too short".to_string()));
    }

    let timestamp = arr[0]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid timestamp".to_string()))?
        .parse::<i64>()
        .map_err(|e| CcxtError::ParseError(format!("Failed to parse timestamp: {}", e)))?;

    Ok(OHLCV {
        timestamp,
        open: parse_decimal(arr[1].as_str().ok_or_else(|| CcxtError::ParseError("Invalid open".to_string()))?)?,
        high: parse_decimal(arr[2].as_str().ok_or_else(|| CcxtError::ParseError("Invalid high".to_string()))?)?,
        low: parse_decimal(arr[3].as_str().ok_or_else(|| CcxtError::ParseError("Invalid low".to_string()))?)?,
        close: parse_decimal(arr[4].as_str().ok_or_else(|| CcxtError::ParseError("Invalid close".to_string()))?)?,
        volume: parse_decimal(arr[5].as_str().ok_or_else(|| CcxtError::ParseError("Invalid volume".to_string()))?)?,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Trade Parser
// ============================================================================

/// Parse OKX trade into unified Trade
pub fn parse_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json
        .get("tradeId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing tradeId".to_string()))?
        .to_string();

    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| CcxtError::ParseError("Missing or invalid ts".to_string()))?;

    let price = parse_decimal(
        json.get("px")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing px".to_string()))?,
    )?;

    let amount = parse_decimal(
        json.get("sz")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing sz".to_string()))?,
    )?;

    let side_str = json
        .get("side")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing side".to_string()))?;

    let side = match side_str {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return Err(CcxtError::ParseError(format!("Unknown side: {}", side_str))),
    };

    let cost = price * amount;

    Ok(Trade {
        id,
        symbol: symbol.to_string(),
        order: None,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        side,
        price,
        amount,
        cost,
        fee: None,
        taker_or_maker: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Order Parser
// ============================================================================

/// Parse OKX order response to unified Order
pub fn parse_order(json: &Value, symbol: &str) -> Result<Order> {
    let order_id = json
        .get("ordId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let client_order_id = json
        .get("clOrdId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Timestamps
    let created_time = json
        .get("cTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let updated_time = json
        .get("uTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());

    // Status mapping
    let state = json.get("state").and_then(|v| v.as_str()).unwrap_or("live");
    let status = match state {
        "live" | "partially_filled" => OrderStatus::Open,
        "filled" => OrderStatus::Closed,
        "canceled" | "mmp_canceled" => OrderStatus::Canceled,
        _ => OrderStatus::Open,
    };

    // Type mapping
    let ord_type = json.get("ordType").and_then(|v| v.as_str()).unwrap_or("limit");
    let order_type = match ord_type {
        "market" => OrderType::Market,
        "limit" | "post_only" | "fok" | "ioc" => OrderType::Limit,
        _ => OrderType::Limit,
    };

    // Side mapping
    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("buy");
    let side = if side_str == "sell" { OrderSide::Sell } else { OrderSide::Buy };

    // Amounts
    let sz = optional_decimal(json, "sz").unwrap_or(Decimal::ZERO);
    let acc_fill_sz = optional_decimal(json, "accFillSz").unwrap_or(Decimal::ZERO);
    let avg_px = optional_decimal(json, "avgPx");
    let fill_px = optional_decimal(json, "fillPx");

    let remaining = sz - acc_fill_sz;
    let cost = avg_px.map(|p| p * acc_fill_sz);
    let average = if acc_fill_sz > Decimal::ZERO { avg_px.or(fill_px) } else { None };

    let price = optional_decimal(json, "px");
    let tp_trigger = optional_decimal(json, "tpTriggerPx").filter(|p| !p.is_zero());
    let sl_trigger = optional_decimal(json, "slTriggerPx").filter(|p| !p.is_zero());

    // Time in force
    let tif = match ord_type {
        "fok" => Some(TimeInForce::Fok),
        "ioc" => Some(TimeInForce::Ioc),
        _ => Some(TimeInForce::Gtc),
    };

    let post_only = Some(ord_type == "post_only");
    let reduce_only = json.get("reduceOnly").and_then(|v| v.as_str()).map(|s| s == "true");

    // Fee (OKX returns negative fee as cost)
    let fee_val = optional_decimal(json, "fee").map(|f| f.abs());
    let fee_ccy = json.get("feeCcy").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
    let fee = match (fee_val, fee_ccy) {
        (Some(cost), Some(ccy)) if !cost.is_zero() => Some(OrderFee {
            cost,
            currency: ccy.to_string(),
            rate: None,
        }),
        _ => None,
    };

    Ok(Order {
        id: order_id,
        client_order_id,
        symbol: symbol.to_string(),
        order_type,
        side,
        status,
        timestamp: created_time,
        datetime: timestamp_to_iso8601(created_time),
        last_trade_timestamp: updated_time,
        price,
        average,
        amount: sz,
        filled: Some(acc_fill_sz),
        remaining: Some(remaining),
        cost,
        fee,
        time_in_force: tif,
        post_only,
        reduce_only,
        stop_price: sl_trigger.or(tp_trigger),
        trigger_price: sl_trigger.or(tp_trigger),
        stop_loss_price: sl_trigger,
        take_profit_price: tp_trigger,
        last_update_timestamp: updated_time,
        trades: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Balance Parser
// ============================================================================

/// Parse OKX account balance response to unified Balances
pub fn parse_balance(json: &Value) -> Result<Balances> {
    let details = json
        .get("details")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing details in balance".to_string()))?;

    let timestamp = json
        .get("uTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let mut balances = HashMap::new();
    let mut free_map = HashMap::new();
    let mut used_map = HashMap::new();
    let mut total_map = HashMap::new();

    for detail in details {
        let ccy = detail.get("ccy").and_then(|v| v.as_str()).unwrap_or("");
        let available = optional_decimal(detail, "availBal")
            .or_else(|| optional_decimal(detail, "availEq"))
            .unwrap_or(Decimal::ZERO);
        let frozen = optional_decimal(detail, "frozenBal").unwrap_or(Decimal::ZERO);
        let cash_bal = optional_decimal(detail, "cashBal").unwrap_or(available + frozen);

        if cash_bal.is_zero() && available.is_zero() {
            continue;
        }

        let used = frozen;
        let total = cash_bal;

        balances.insert(ccy.to_string(), Balance::new(ccy.to_string(), available, used));
        free_map.insert(ccy.to_string(), available);
        used_map.insert(ccy.to_string(), used);
        total_map.insert(ccy.to_string(), total);
    }

    Ok(Balances {
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        balances,
        free: free_map,
        used: used_map,
        total: total_map,
        info: Some(json.clone()),
    })
}

// ============================================================================
// My Trade Parser
// ============================================================================

/// Parse OKX fill/execution to unified Trade
pub fn parse_my_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json
        .get("tradeId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let order_id = json
        .get("ordId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("buy");
    let side = if side_str == "sell" { OrderSide::Sell } else { OrderSide::Buy };

    let exec_type = json.get("execType").and_then(|v| v.as_str()).unwrap_or("");
    let taker_or_maker = match exec_type {
        "M" => "maker",
        "T" => "taker",
        _ => "taker",
    };

    let price = optional_decimal(json, "fillPx").unwrap_or(Decimal::ZERO);
    let qty = optional_decimal(json, "fillSz").unwrap_or(Decimal::ZERO);
    let cost = price * qty;

    let fee_val = optional_decimal(json, "fee").map(|f| f.abs()).unwrap_or(Decimal::ZERO);
    let fee_ccy = json.get("feeCcy").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let fee = if !fee_ccy.is_empty() || !fee_val.is_zero() {
        Some(TradeFee {
            cost: fee_val,
            currency: fee_ccy,
            rate: None,
        })
    } else {
        None
    };

    Ok(Trade {
        id,
        symbol: symbol.to_string(),
        order: order_id,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        side,
        price,
        amount: qty,
        cost,
        fee,
        taker_or_maker: Some(taker_or_maker.to_string()),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Position Parser
// ============================================================================

/// Parse OKX position to unified Position
pub fn parse_position(json: &Value) -> Result<Position> {
    let inst_id = json
        .get("instId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing instId in position".to_string()))?;

    let symbol = symbol_from_okx(inst_id);

    let pos = optional_decimal(json, "pos").unwrap_or(Decimal::ZERO);

    let pos_side = json.get("posSide").and_then(|v| v.as_str()).unwrap_or("");
    let side = match pos_side {
        "long" => PositionSide::Long,
        "short" => PositionSide::Short,
        _ => {
            if pos > Decimal::ZERO {
                PositionSide::Long
            } else if pos < Decimal::ZERO {
                PositionSide::Short
            } else {
                PositionSide::Both
            }
        }
    };

    let mgn_mode = json.get("mgnMode").and_then(|v| v.as_str()).unwrap_or("cross");
    let margin_mode = if mgn_mode == "isolated" {
        MarginMode::Isolated
    } else {
        MarginMode::Cross
    };

    let entry_price = optional_decimal(json, "avgPx");
    let mark_price = optional_decimal(json, "markPx");
    let unrealized_pnl = optional_decimal(json, "upl");
    let realized_pnl = optional_decimal(json, "realizedPnl");
    let liquidation_price = optional_decimal(json, "liqPx").filter(|p| !p.is_zero());
    let leverage = optional_decimal(json, "lever");
    let notional = optional_decimal(json, "notionalUsd");
    let margin = optional_decimal(json, "margin");
    let mmr = optional_decimal(json, "mmr");

    let updated_time = json
        .get("uTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());

    let now = chrono::Utc::now().timestamp_millis();

    Ok(Position {
        symbol,
        id: None,
        timestamp: updated_time.unwrap_or(now),
        datetime: timestamp_to_iso8601(updated_time.unwrap_or(now)),
        side,
        margin_mode,
        contracts: pos.abs(),
        contract_size: optional_decimal(json, "ctVal"),
        notional,
        leverage,
        entry_price,
        mark_price,
        unrealized_pnl,
        realized_pnl,
        collateral: None,
        initial_margin: margin,
        maintenance_margin: None,
        liquidation_price,
        margin_ratio: mmr,
        percentage: None,
        stop_loss_price: None,
        take_profit_price: None,
        hedged: None,
        maintenance_margin_percentage: mmr,
        initial_margin_percentage: None,
        last_update_timestamp: updated_time,
        last_price: optional_decimal(json, "last"),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Funding Rate Parser
// ============================================================================

/// Parse OKX funding rate to unified FundingRate
pub fn parse_funding_rate(json: &Value, symbol: &str) -> Result<FundingRate> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let funding_rate = optional_decimal(json, "fundingRate").unwrap_or(Decimal::ZERO);
    let next_funding_rate = optional_decimal(json, "nextFundingRate");
    let funding_time = json
        .get("fundingTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());
    let next_funding_time = json
        .get("nextFundingTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());

    Ok(FundingRate {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        funding_rate,
        funding_timestamp: funding_time,
        funding_datetime: funding_time.map(timestamp_to_iso8601),
        mark_price: None,
        index_price: None,
        interest_rate: None,
        estimated_settle_price: None,
        interval: Some("8h".to_string()),
        previous_funding_rate: None,
        previous_funding_timestamp: None,
        previous_funding_datetime: None,
        next_funding_rate,
        next_funding_timestamp: next_funding_time,
        next_funding_datetime: next_funding_time.map(timestamp_to_iso8601),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Currency Parser
// ============================================================================

/// Parse OKX currency info to unified Currency
pub fn parse_currency(json: &Value) -> Result<Currency> {
    let ccy = json
        .get("ccy")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing ccy".to_string()))?;

    let name = json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());

    let can_dep = json.get("canDep").and_then(|v| v.as_bool()).unwrap_or(false);
    let can_wd = json.get("canWd").and_then(|v| v.as_bool()).unwrap_or(false);

    let chain = json.get("chain").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let min_fee = optional_decimal(json, "minFee");
    let min_wd = optional_decimal(json, "minWd");

    let network = if !chain.is_empty() {
        Some(vec![Network {
            id: chain.clone(),
            network: chain.clone(),
            name: Some(chain),
            deposit: Some(can_dep),
            withdraw: Some(can_wd),
            fee: min_fee,
            precision: None,
            limits: None,
        }])
    } else {
        None
    };

    Ok(Currency {
        code: ccy.to_string(),
        id: ccy.to_string(),
        name,
        active: can_dep || can_wd,
        deposit: Some(can_dep),
        withdraw: Some(can_wd),
        fee: min_fee,
        precision: None,
        limits: if min_wd.is_some() {
            Some(CurrencyLimits {
                withdraw: Some(crate::types::currency::MinMax { min: min_wd, max: None }),
                deposit: None,
            })
        } else {
            None
        },
        networks: network,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Deposit / Withdrawal Parsers
// ============================================================================

/// Parse OKX deposit record
pub fn parse_deposit(json: &Value) -> Result<Deposit> {
    let id = json
        .get("depId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let txid = json.get("txId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let network = json.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let amount = optional_decimal(json, "amt").unwrap_or(Decimal::ZERO);
    let ccy = json.get("ccy").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // OKX deposit states: 0=waiting, 1=deposited, 2=confirmed, 8=pending (no memo), 12=bounced, 13=awaiting
    let state = json.get("state").and_then(|v| v.as_str()).unwrap_or("0");
    let status = match state {
        "2" => TransactionStatus::Ok,
        "1" => TransactionStatus::Ok,
        "12" => TransactionStatus::Failed,
        _ => TransactionStatus::Pending,
    };

    Ok(Deposit {
        id,
        txid,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        network,
        address,
        tag: None,
        transaction_type: TransactionType::Deposit,
        amount,
        currency: ccy,
        status,
        updated: None,
        fee: None,
        info: Some(json.clone()),
    })
}

/// Parse OKX withdrawal record
pub fn parse_withdrawal(json: &Value) -> Result<Withdrawal> {
    let id = json
        .get("wdId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let txid = json.get("txId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let network = json.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let amount = optional_decimal(json, "amt").unwrap_or(Decimal::ZERO);
    let ccy = json.get("ccy").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // OKX withdrawal states: -3=canceling, -2=canceled, -1=failed, 0=pending, 1=sending,
    // 2=sent, 3=awaiting, 4-5=confirmed, 7=approved, 10=waiting transfer, 12=complete
    let state = json.get("state").and_then(|v| v.as_str()).unwrap_or("0");
    let status = match state {
        "4" | "5" | "12" => TransactionStatus::Ok,
        "-2" => TransactionStatus::Canceled,
        "-1" | "-3" => TransactionStatus::Failed,
        _ => TransactionStatus::Pending,
    };

    let fee_val = optional_decimal(json, "fee");
    let fee = fee_val.map(|cost| deposit::TransactionFee {
        cost: cost.abs(),
        currency: ccy.clone(),
    });

    Ok(Withdrawal {
        id,
        txid,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        network,
        address,
        tag: None,
        transaction_type: TransactionType::Withdrawal,
        amount,
        currency: ccy,
        status,
        updated: None,
        fee,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Status Parser
// ============================================================================

/// Parse OKX system status
pub fn parse_status(json: &Value) -> Result<ExchangeStatus> {
    let state = json
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("scheduled");

    let status = match state {
        "ongoing" => "maintenance",
        _ => "ok",
    };

    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    Ok(ExchangeStatus {
        status: status.to_string(),
        updated: timestamp,
        eta: None,
        url: None,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion_spot() {
        assert_eq!(symbol_to_okx("BTC/USDT"), "BTC-USDT");
        assert_eq!(symbol_to_okx("ETH/USDC"), "ETH-USDC");

        assert_eq!(symbol_from_okx("BTC-USDT"), "BTC/USDT");
        assert_eq!(symbol_from_okx("ETH-USDC"), "ETH/USDC");
    }

    #[test]
    fn test_symbol_conversion_swap() {
        assert_eq!(symbol_to_okx("BTC/USDT:USDT"), "BTC-USDT-SWAP");
        assert_eq!(symbol_to_okx("ETH/USDT:USDT"), "ETH-USDT-SWAP");

        assert_eq!(symbol_from_okx("BTC-USDT-SWAP"), "BTC/USDT:USDT");
        assert_eq!(symbol_from_okx("ETH-USDT-SWAP"), "ETH/USDT:USDT");
    }

    #[test]
    fn test_inst_type() {
        assert_eq!(inst_type_for_symbol("BTC/USDT"), "SPOT");
        assert_eq!(inst_type_for_symbol("BTC/USDT:USDT"), "SWAP");
        assert!(is_swap_symbol("BTC/USDT:USDT"));
        assert!(!is_swap_symbol("BTC/USDT"));
    }

    #[test]
    fn test_timeframe_conversion() {
        assert_eq!(timeframe_to_okx(&Timeframe::OneMinute), "1m");
        assert_eq!(timeframe_to_okx(&Timeframe::FiveMinutes), "5m");
        assert_eq!(timeframe_to_okx(&Timeframe::OneHour), "1H");
        assert_eq!(timeframe_to_okx(&Timeframe::OneDay), "1D");
    }

    #[test]
    fn test_count_decimals() {
        assert_eq!(count_decimals("0.01"), 2);
        assert_eq!(count_decimals("0.001"), 3);
        assert_eq!(count_decimals("1"), 0);
        assert_eq!(count_decimals("0.0100"), 2);
    }

    #[test]
    fn test_parse_order_live() {
        let json = serde_json::json!({
            "ordId": "12345",
            "clOrdId": "custom_id",
            "instId": "BTC-USDT",
            "side": "buy",
            "ordType": "limit",
            "px": "50000.00",
            "sz": "0.001",
            "accFillSz": "0",
            "avgPx": "",
            "state": "live",
            "cTime": "1700000000000",
            "uTime": "1700000000000",
            "fee": "0",
            "feeCcy": "USDT"
        });

        let order = parse_order(&json, "BTC/USDT").unwrap();
        assert_eq!(order.id, "12345");
        assert_eq!(order.client_order_id.as_deref(), Some("custom_id"));
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.amount, Decimal::from_str("0.001").unwrap());
        assert_eq!(order.filled, Some(Decimal::ZERO));
        assert_eq!(order.remaining, Some(Decimal::from_str("0.001").unwrap()));
    }

    #[test]
    fn test_parse_order_filled() {
        let json = serde_json::json!({
            "ordId": "99999",
            "instId": "ETH-USDT",
            "side": "sell",
            "ordType": "market",
            "sz": "1.0",
            "accFillSz": "1.0",
            "avgPx": "3000.00",
            "state": "filled",
            "cTime": "1700000000000",
            "uTime": "1700000000000",
            "fee": "-1.80",
            "feeCcy": "USDT"
        });

        let order = parse_order(&json, "ETH/USDT").unwrap();
        assert_eq!(order.status, OrderStatus::Closed);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.cost, Some(Decimal::from_str("3000.00").unwrap()));
        assert_eq!(order.average, Some(Decimal::from_str("3000.00").unwrap()));
        assert_eq!(order.remaining, Some(Decimal::ZERO));
        let fee = order.fee.unwrap();
        assert_eq!(fee.cost, Decimal::from_str("1.80").unwrap());
        assert_eq!(fee.currency, "USDT");
    }

    #[test]
    fn test_parse_balance() {
        let json = serde_json::json!({
            "uTime": "1700000000000",
            "details": [
                {"ccy": "BTC", "availBal": "0.4", "frozenBal": "0.1", "cashBal": "0.5"},
                {"ccy": "USDT", "availBal": "8000.0", "frozenBal": "2000.0", "cashBal": "10000.0"},
                {"ccy": "ETH", "availBal": "0", "frozenBal": "0", "cashBal": "0"}
            ]
        });

        let balances = parse_balance(&json).unwrap();
        assert_eq!(balances.balances.len(), 2); // ETH skipped
        let btc = balances.balances.get("BTC").unwrap();
        assert_eq!(btc.free, Decimal::from_str("0.4").unwrap());
        assert_eq!(btc.used, Decimal::from_str("0.1").unwrap());
        assert_eq!(btc.total, Decimal::from_str("0.5").unwrap());

        assert_eq!(*balances.free.get("USDT").unwrap(), Decimal::from_str("8000.0").unwrap());
        assert_eq!(*balances.total.get("USDT").unwrap(), Decimal::from_str("10000.0").unwrap());
    }

    #[test]
    fn test_parse_position() {
        let json = serde_json::json!({
            "instId": "BTC-USDT-SWAP",
            "pos": "0.1",
            "posSide": "long",
            "avgPx": "48000.0",
            "markPx": "50000.0",
            "upl": "200.0",
            "liqPx": "40000.0",
            "lever": "10",
            "mgnMode": "isolated",
            "notionalUsd": "5000.0",
            "margin": "500.0",
            "mmr": "0.004",
            "uTime": "1700000000000"
        });

        let position = parse_position(&json).unwrap();
        assert_eq!(position.symbol, "BTC/USDT:USDT");
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.contracts, Decimal::from_str("0.1").unwrap());
        assert_eq!(position.margin_mode, MarginMode::Isolated);
        assert_eq!(position.entry_price, Some(Decimal::from_str("48000.0").unwrap()));
        assert_eq!(position.leverage, Some(Decimal::from(10)));
    }

    #[test]
    fn test_parse_funding_rate() {
        let json = serde_json::json!({
            "instId": "BTC-USDT-SWAP",
            "fundingRate": "0.0001",
            "nextFundingRate": "0.00012",
            "fundingTime": "1700000000000",
            "nextFundingTime": "1700028800000",
            "ts": "1700000000000"
        });

        let fr = parse_funding_rate(&json, "BTC/USDT:USDT").unwrap();
        assert_eq!(fr.funding_rate, Decimal::from_str("0.0001").unwrap());
        assert_eq!(fr.next_funding_rate, Some(Decimal::from_str("0.00012").unwrap()));
        assert_eq!(fr.next_funding_timestamp, Some(1700028800000));
    }

    #[test]
    fn test_parse_my_trade() {
        let json = serde_json::json!({
            "tradeId": "trade123",
            "ordId": "order456",
            "instId": "BTC-USDT",
            "side": "buy",
            "fillPx": "50000.0",
            "fillSz": "0.001",
            "fee": "-0.03",
            "feeCcy": "USDT",
            "execType": "T",
            "ts": "1700000000000"
        });

        let trade = parse_my_trade(&json, "BTC/USDT").unwrap();
        assert_eq!(trade.id, "trade123");
        assert_eq!(trade.order, Some("order456".to_string()));
        assert_eq!(trade.side, OrderSide::Buy);
        assert_eq!(trade.taker_or_maker, Some("taker".to_string()));
        let fee = trade.fee.unwrap();
        assert_eq!(fee.cost, Decimal::from_str("0.03").unwrap());
        assert_eq!(fee.currency, "USDT");
    }

    #[test]
    fn test_parse_deposit() {
        let json = serde_json::json!({
            "depId": "dep123",
            "txId": "0xabc",
            "ccy": "USDT",
            "chain": "USDT-ERC20",
            "to": "0x1234",
            "amt": "100.5",
            "state": "2",
            "ts": "1700000000000"
        });

        let dep = parse_deposit(&json).unwrap();
        assert_eq!(dep.id, "dep123");
        assert_eq!(dep.currency, "USDT");
        assert_eq!(dep.amount, Decimal::from_str("100.5").unwrap());
        assert_eq!(dep.status, TransactionStatus::Ok);
    }

    #[test]
    fn test_parse_withdrawal() {
        let json = serde_json::json!({
            "wdId": "wd999",
            "txId": "0xdef",
            "ccy": "ETH",
            "chain": "ETH-ERC20",
            "to": "0xabcdef",
            "amt": "1.5",
            "fee": "0.005",
            "state": "4",
            "ts": "1700000000000"
        });

        let wd = parse_withdrawal(&json).unwrap();
        assert_eq!(wd.id, "wd999");
        assert_eq!(wd.currency, "ETH");
        assert_eq!(wd.amount, Decimal::from_str("1.5").unwrap());
        assert_eq!(wd.status, TransactionStatus::Ok);
        assert!(wd.fee.is_some());
    }

    #[test]
    fn test_parse_swap_market() {
        let json = serde_json::json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "settleCcy": "USDT",
            "ctType": "linear",
            "ctVal": "0.01",
            "lotSz": "1",
            "tickSz": "0.1",
            "minSz": "1",
            "maxLmtSz": "10000",
            "lever": "125",
            "state": "live"
        });

        let market = parse_swap_market(&json).unwrap();
        assert_eq!(market.symbol, "BTC/USDT:USDT");
        assert_eq!(market.id, "BTC-USDT-SWAP");
        assert!(market.swap);
        assert!(!market.spot);
        assert_eq!(market.settle, Some("USDT".to_string()));
        assert_eq!(market.contract, Some(true));
        assert_eq!(market.linear, Some(true));
    }
}
