//! Bybit API response parsers
//!
//! Convert Bybit v5 API responses to unified CCXT types

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

/// Convert unified symbol to Bybit format
///
/// Handles both spot ("BTC/USDT" -> "BTCUSDT") and
/// linear ("BTC/USDT:USDT" -> "BTCUSDT") symbols.
pub fn symbol_to_bybit(symbol: &str) -> String {
    let base_symbol = symbol.split(':').next().unwrap_or(symbol);
    base_symbol.replace('/', "")
}

/// Convert Bybit spot symbol to unified format
pub fn symbol_from_bybit(bybit_symbol: &str) -> String {
    let quote_currencies = ["USDT", "USDC", "BTC", "ETH", "EUR", "DAI"];

    for quote in &quote_currencies {
        if bybit_symbol.ends_with(quote) {
            let quote_len = quote.len();
            let total_len = bybit_symbol.len();

            if total_len > quote_len {
                let base = &bybit_symbol[..total_len - quote_len];
                if !base.is_empty() {
                    return format!("{}/{}", base, quote);
                }
            }
        }
    }

    bybit_symbol.to_string()
}

/// Convert Bybit linear symbol to unified format with settle currency
pub fn symbol_from_bybit_linear(bybit_symbol: &str) -> String {
    let spot = symbol_from_bybit(bybit_symbol);
    if spot.contains('/') {
        let quote = spot.split('/').next_back().unwrap_or("USDT");
        format!("{}:{}", spot, quote)
    } else {
        spot
    }
}

/// Convert CCXT timeframe to Bybit interval
pub fn timeframe_to_bybit(timeframe: &Timeframe) -> String {
    match timeframe {
        Timeframe::OneMinute => "1",
        Timeframe::ThreeMinutes => "3",
        Timeframe::FiveMinutes => "5",
        Timeframe::FifteenMinutes => "15",
        Timeframe::ThirtyMinutes => "30",
        Timeframe::OneHour => "60",
        Timeframe::TwoHours => "120",
        Timeframe::FourHours => "240",
        Timeframe::SixHours => "360",
        Timeframe::TwelveHours => "720",
        Timeframe::OneDay => "D",
        Timeframe::OneWeek => "W",
        Timeframe::OneMonth => "M",
        _ => "60",
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

/// Parse Bybit spot market info into unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let symbol_str = json
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in market".to_string()))?;

    let base_coin = json
        .get("baseCoin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing baseCoin".to_string()))?;

    let quote_coin = json
        .get("quoteCoin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteCoin".to_string()))?;

    let symbol = format!("{}/{}", base_coin, quote_coin);

    let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("Trading");
    let active = status == "Trading";

    let lot_size_filter = json.get("lotSizeFilter");
    let price_filter = json.get("priceFilter");

    let min_order_qty = lot_size_filter
        .and_then(|f| f.get("minOrderQty"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());
    let max_order_qty = lot_size_filter
        .and_then(|f| f.get("maxOrderQty"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let min_price = price_filter
        .and_then(|f| f.get("minPrice"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());
    let max_price = price_filter
        .and_then(|f| f.get("maxPrice"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let tick_size_str = price_filter.and_then(|f| f.get("tickSize")).and_then(|v| v.as_str());
    let step_size_str = lot_size_filter
        .and_then(|f| f.get("basePrecision"))
        .and_then(|v| v.as_str());

    let price_precision = tick_size_str.map(count_decimals);
    let amount_precision = step_size_str.map(count_decimals);

    Ok(Market {
        id: symbol_str.to_string(),
        symbol,
        base: base_coin.to_string(),
        quote: quote_coin.to_string(),
        settle: None,
        base_id: base_coin.to_string(),
        quote_id: quote_coin.to_string(),
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
            amount: Some(MinMax {
                min: min_order_qty,
                max: max_order_qty,
            }),
            price: Some(MinMax {
                min: min_price,
                max: max_price,
            }),
            cost: Some(MinMax {
                min: None,
                max: None,
            }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

/// Parse Bybit linear (derivatives) market into unified Market
pub fn parse_linear_market(json: &Value) -> Result<Market> {
    let symbol_str = json
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in linear market".to_string()))?;

    let base_coin = json
        .get("baseCoin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing baseCoin".to_string()))?;

    let quote_coin = json
        .get("quoteCoin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteCoin".to_string()))?;

    let settle_coin = json
        .get("settleCoin")
        .and_then(|v| v.as_str())
        .unwrap_or(quote_coin);

    let contract_type = json.get("contractType").and_then(|v| v.as_str()).unwrap_or("");
    if contract_type != "LinearPerpetual" {
        return Err(CcxtError::ParseError(format!("Skipping non-perpetual: {}", contract_type)));
    }

    let symbol = format!("{}/{}:{}", base_coin, quote_coin, settle_coin);
    let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("Trading");
    let active = status == "Trading";

    let lot_size_filter = json.get("lotSizeFilter");
    let price_filter = json.get("priceFilter");
    let leverage_filter = json.get("leverageFilter");

    let min_qty = lot_size_filter
        .and_then(|f| f.get("minOrderQty"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());
    let max_qty = lot_size_filter
        .and_then(|f| f.get("maxOrderQty"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let min_price = price_filter
        .and_then(|f| f.get("minPrice"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());
    let max_price = price_filter
        .and_then(|f| f.get("maxPrice"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let tick_size_str = price_filter.and_then(|f| f.get("tickSize")).and_then(|v| v.as_str());
    let qty_step_str = lot_size_filter.and_then(|f| f.get("qtyStep")).and_then(|v| v.as_str());

    let price_precision = tick_size_str.map(count_decimals);
    let amount_precision = qty_step_str.map(count_decimals);

    let max_leverage = leverage_filter
        .and_then(|f| f.get("maxLeverage"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());
    let min_leverage = leverage_filter
        .and_then(|f| f.get("minLeverage"))
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    Ok(Market {
        id: symbol_str.to_string(),
        symbol,
        base: base_coin.to_string(),
        quote: quote_coin.to_string(),
        settle: Some(settle_coin.to_string()),
        base_id: base_coin.to_string(),
        quote_id: quote_coin.to_string(),
        settle_id: Some(settle_coin.to_string()),
        market_type: "swap".to_string(),
        spot: false,
        margin: false,
        swap: true,
        future: false,
        option: false,
        active,
        contract: Some(true),
        linear: Some(true),
        inverse: Some(false),
        taker: None,
        maker: None,
        contract_size: Some(Decimal::ONE),
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
            amount: Some(MinMax {
                min: min_qty,
                max: max_qty,
            }),
            price: Some(MinMax {
                min: min_price,
                max: max_price,
            }),
            cost: Some(MinMax {
                min: None,
                max: None,
            }),
            leverage: Some(MinMax {
                min: min_leverage,
                max: max_leverage,
            }),
        },
        info: Some(json.clone()),
    })
}

// ============================================================================
// Ticker Parser
// ============================================================================

/// Parse Bybit ticker into unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = chrono::Utc::now().timestamp_millis();

    let last = optional_decimal(json, "lastPrice");
    let bid = optional_decimal(json, "bid1Price");
    let ask = optional_decimal(json, "ask1Price");
    let high = optional_decimal(json, "highPrice24h");
    let low = optional_decimal(json, "lowPrice24h");
    let volume = optional_decimal(json, "volume24h");
    let quote_volume = optional_decimal(json, "turnover24h");
    let prev_price = optional_decimal(json, "prevPrice24h");

    let change = match (last, prev_price) {
        (Some(l), Some(p)) => Some(l - p),
        _ => None,
    };

    let percentage = optional_decimal(json, "price24hPcnt")
        .map(|p| p * Decimal::from(100));

    Ok(Ticker {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        high,
        low,
        bid,
        bid_volume: optional_decimal(json, "bid1Size"),
        ask,
        ask_volume: optional_decimal(json, "ask1Size"),
        vwap: None,
        open: prev_price,
        close: last,
        last,
        previous_close: prev_price,
        change,
        percentage,
        average: None,
        base_volume: volume,
        quote_volume,
        index_price: optional_decimal(json, "indexPrice"),
        mark_price: optional_decimal(json, "markPrice"),
        info: Some(json.clone()),
    })
}

// ============================================================================
// OrderBook Parser
// ============================================================================

/// Parse Bybit order book into unified OrderBook
pub fn parse_orderbook(json: &Value, symbol: &str) -> Result<OrderBook> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| json.get("ts").and_then(|v| v.as_i64()))
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let bids_array = json
        .get("b")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing bids (b) in orderbook".to_string()))?;

    let asks_array = json
        .get("a")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing asks (a) in orderbook".to_string()))?;

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

/// Parse Bybit OHLCV candle into unified OHLCV
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

/// Parse Bybit public trade into unified Trade
pub fn parse_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json
        .get("execId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing execId".to_string()))?
        .to_string();

    let timestamp = json
        .get("time")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| CcxtError::ParseError("Missing or invalid time".to_string()))?;

    let price = parse_decimal(
        json.get("price")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing price".to_string()))?,
    )?;

    let amount = parse_decimal(
        json.get("size")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing size".to_string()))?,
    )?;

    let side_str = json
        .get("side")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing side".to_string()))?;

    let side = match side_str {
        "Buy" => OrderSide::Buy,
        "Sell" => OrderSide::Sell,
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

/// Parse Bybit order response to unified Order
pub fn parse_order(json: &Value, symbol: &str) -> Result<Order> {
    let order_id = json
        .get("orderId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let client_order_id = json
        .get("orderLinkId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // Timestamps
    let created_time = json
        .get("createdTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let updated_time = json
        .get("updatedTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());

    // Status mapping
    let status_str = json.get("orderStatus").and_then(|v| v.as_str()).unwrap_or("New");
    let status = match status_str {
        "New" | "PartiallyFilled" | "Untriggered" => OrderStatus::Open,
        "Filled" => OrderStatus::Closed,
        "Cancelled" | "PartiallyFilledCanceled" | "Deactivated" => OrderStatus::Canceled,
        "Rejected" => OrderStatus::Rejected,
        "Triggered" => OrderStatus::Open,
        _ => OrderStatus::Open,
    };

    // Type mapping
    let type_str = json.get("orderType").and_then(|v| v.as_str()).unwrap_or("Limit");
    let order_type = match type_str {
        "Limit" => OrderType::Limit,
        "Market" => OrderType::Market,
        _ => OrderType::Limit,
    };

    // Side mapping
    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("Buy");
    let side = if side_str == "Sell" { OrderSide::Sell } else { OrderSide::Buy };

    // Amounts
    let orig_qty = optional_decimal(json, "qty")
        .ok_or_else(|| CcxtError::ParseError("Missing field: qty in order".into()))?;
    let cum_exec_qty = optional_decimal(json, "cumExecQty")
        .ok_or_else(|| CcxtError::ParseError("Missing field: cumExecQty in order".into()))?;
    let cum_exec_value = optional_decimal(json, "cumExecValue");
    let cum_exec_fee = optional_decimal(json, "cumExecFee");

    let remaining = orig_qty - cum_exec_qty;
    let average = if cum_exec_qty > Decimal::ZERO {
        cum_exec_value.map(|v| v / cum_exec_qty)
    } else {
        None
    };

    let price = optional_decimal(json, "price");
    let trigger_price = optional_decimal(json, "triggerPrice").filter(|p| !p.is_zero());
    let stop_loss = optional_decimal(json, "stopLoss").filter(|p| !p.is_zero());
    let take_profit = optional_decimal(json, "takeProfit").filter(|p| !p.is_zero());

    // Time in force
    let tif = json.get("timeInForce").and_then(|v| v.as_str()).map(|s| match s {
        "GTC" => TimeInForce::Gtc,
        "IOC" => TimeInForce::Ioc,
        "FOK" => TimeInForce::Fok,
        "PostOnly" => TimeInForce::Gtc,
        _ => TimeInForce::Gtc,
    });

    let post_only = json.get("timeInForce").and_then(|v| v.as_str()).map(|s| s == "PostOnly");
    let reduce_only = json.get("reduceOnly").and_then(|v| v.as_bool());

    // Fee
    let fee = cum_exec_fee.map(|cost| OrderFee {
        cost,
        currency: String::new(),
        rate: None,
    });

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
        amount: orig_qty,
        filled: Some(cum_exec_qty),
        remaining: Some(remaining),
        cost: cum_exec_value,
        fee,
        time_in_force: tif,
        post_only,
        reduce_only,
        stop_price: trigger_price,
        trigger_price,
        stop_loss_price: stop_loss,
        take_profit_price: take_profit,
        last_update_timestamp: updated_time,
        trades: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Balance Parser
// ============================================================================

/// Parse Bybit account wallet-balance response to unified Balances
pub fn parse_balance(json: &Value) -> Result<Balances> {
    let coins = json
        .get("coin")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing coin array in balance".to_string()))?;

    let timestamp = json
        .get("accountIMRate")
        .and_then(|_| Some(chrono::Utc::now().timestamp_millis()))
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let mut balances = HashMap::new();
    let mut free_map = HashMap::new();
    let mut used_map = HashMap::new();
    let mut total_map = HashMap::new();

    for coin in coins {
        let asset = coin.get("coin").and_then(|v| v.as_str()).unwrap_or("");
        let wallet_balance = optional_decimal(coin, "walletBalance").unwrap_or(Decimal::ZERO);
        let available = optional_decimal(coin, "availableToWithdraw")
            .or_else(|| optional_decimal(coin, "free"))
            .unwrap_or(wallet_balance);

        if wallet_balance.is_zero() && available.is_zero() {
            continue;
        }

        let used = wallet_balance - available;
        balances.insert(asset.to_string(), Balance::new(asset.to_string(), available, used));
        free_map.insert(asset.to_string(), available);
        used_map.insert(asset.to_string(), used);
        total_map.insert(asset.to_string(), wallet_balance);
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

/// Parse Bybit execution/trade to unified Trade
pub fn parse_my_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json
        .get("execId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let order_id = json
        .get("orderId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let timestamp = json
        .get("execTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("Buy");
    let side = if side_str == "Sell" { OrderSide::Sell } else { OrderSide::Buy };

    let is_maker = json
        .get("isMaker")
        .and_then(|v| v.as_str())
        .map(|s| s == "true")
        .or_else(|| json.get("isMaker").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let taker_or_maker = if is_maker { "maker" } else { "taker" };

    let price = optional_decimal(json, "execPrice").unwrap_or(Decimal::ZERO);
    let qty = optional_decimal(json, "execQty").unwrap_or(Decimal::ZERO);
    let exec_value = optional_decimal(json, "execValue").unwrap_or(price * qty);

    let exec_fee = optional_decimal(json, "execFee").unwrap_or(Decimal::ZERO);
    let fee_currency = json
        .get("feeCurrency")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let fee = if !fee_currency.is_empty() || !exec_fee.is_zero() {
        Some(TradeFee {
            cost: exec_fee,
            currency: fee_currency,
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
        cost: exec_value,
        fee,
        taker_or_maker: Some(taker_or_maker.to_string()),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Position Parser
// ============================================================================

/// Parse Bybit position to unified Position
pub fn parse_position(json: &Value) -> Result<Position> {
    let bybit_symbol = json
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in position".to_string()))?;

    let symbol = symbol_from_bybit_linear(bybit_symbol);

    let size = optional_decimal(json, "size").unwrap_or(Decimal::ZERO);

    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("");
    let side = match side_str {
        "Buy" => PositionSide::Long,
        "Sell" => PositionSide::Short,
        _ => {
            if size > Decimal::ZERO {
                PositionSide::Long
            } else if size < Decimal::ZERO {
                PositionSide::Short
            } else {
                PositionSide::Both
            }
        }
    };

    let trade_mode = json.get("tradeMode").and_then(|v| v.as_i64()).unwrap_or(0);
    let margin_mode = if trade_mode == 1 {
        MarginMode::Isolated
    } else {
        MarginMode::Cross
    };

    let entry_price = optional_decimal(json, "avgPrice")
        .or_else(|| optional_decimal(json, "entryPrice"));
    let mark_price = optional_decimal(json, "markPrice");
    let unrealized_pnl = optional_decimal(json, "unrealisedPnl");
    let cum_realized_pnl = optional_decimal(json, "cumRealisedPnl");
    let liquidation_price = optional_decimal(json, "liqPrice").filter(|p| !p.is_zero());
    let leverage = optional_decimal(json, "leverage");
    let position_value = optional_decimal(json, "positionValue");
    let position_mm = optional_decimal(json, "positionMM");
    let position_im = optional_decimal(json, "positionIM");

    let updated_time = json
        .get("updatedTime")
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
        contracts: size.abs(),
        contract_size: Some(Decimal::ONE),
        notional: position_value,
        leverage,
        entry_price,
        mark_price,
        unrealized_pnl,
        realized_pnl: cum_realized_pnl,
        collateral: None,
        initial_margin: position_im,
        maintenance_margin: position_mm,
        liquidation_price,
        margin_ratio: None,
        percentage: None,
        stop_loss_price: optional_decimal(json, "stopLoss").filter(|p| !p.is_zero()),
        take_profit_price: optional_decimal(json, "takeProfit").filter(|p| !p.is_zero()),
        hedged: None,
        maintenance_margin_percentage: None,
        initial_margin_percentage: None,
        last_update_timestamp: updated_time,
        last_price: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Funding Rate Parser
// ============================================================================

/// Parse Bybit linear ticker to unified FundingRate
pub fn parse_funding_rate(json: &Value, symbol: &str) -> Result<FundingRate> {
    let timestamp = chrono::Utc::now().timestamp_millis();

    let funding_rate = optional_decimal(json, "fundingRate").unwrap_or(Decimal::ZERO);
    let next_funding_time = json
        .get("nextFundingTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());

    Ok(FundingRate {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        funding_rate,
        funding_timestamp: next_funding_time,
        funding_datetime: next_funding_time.map(timestamp_to_iso8601),
        mark_price: optional_decimal(json, "markPrice"),
        index_price: optional_decimal(json, "indexPrice"),
        interest_rate: None,
        estimated_settle_price: None,
        interval: Some("8h".to_string()),
        previous_funding_rate: None,
        previous_funding_timestamp: None,
        previous_funding_datetime: None,
        next_funding_rate: None,
        next_funding_timestamp: next_funding_time,
        next_funding_datetime: next_funding_time.map(timestamp_to_iso8601),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Currency Parser
// ============================================================================

/// Parse Bybit coin info to unified Currency
pub fn parse_currency(json: &Value) -> Result<Currency> {
    let coin = json
        .get("coin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing coin".to_string()))?;

    let name = json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());

    let chains = json.get("chains").and_then(|v| v.as_array());

    let mut networks = Vec::new();
    let mut default_deposit = false;
    let mut default_withdraw = false;
    let mut default_fee: Option<Decimal> = None;

    if let Some(chain_list) = chains {
        for chain in chain_list {
            let chain_type = chain.get("chainType").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let chain_name = chain.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string());
            let deposit_enable = chain.get("chainDeposit").and_then(|v| v.as_str()).map(|s| s == "1").unwrap_or(false);
            let withdraw_enable = chain.get("chainWithdraw").and_then(|v| v.as_str()).map(|s| s == "1").unwrap_or(false);
            let withdraw_fee = optional_decimal(chain, "withdrawFee");

            if deposit_enable {
                default_deposit = true;
            }
            if withdraw_enable {
                default_withdraw = true;
            }
            if default_fee.is_none() {
                default_fee = withdraw_fee;
            }

            networks.push(Network {
                id: chain_type.clone(),
                network: chain_type,
                name: chain_name,
                deposit: Some(deposit_enable),
                withdraw: Some(withdraw_enable),
                fee: withdraw_fee,
                precision: None,
                limits: None,
            });
        }
    }

    Ok(Currency {
        code: coin.to_string(),
        id: coin.to_string(),
        name,
        active: default_deposit || default_withdraw,
        deposit: Some(default_deposit),
        withdraw: Some(default_withdraw),
        fee: default_fee,
        precision: None,
        limits: None,
        networks: if networks.is_empty() { None } else { Some(networks) },
        info: Some(json.clone()),
    })
}

// ============================================================================
// Deposit / Withdrawal Parsers
// ============================================================================

/// Parse Bybit deposit record
pub fn parse_deposit(json: &Value) -> Result<Deposit> {
    let id = json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let txid = json.get("txID").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let timestamp = json
        .get("successAt")
        .or_else(|| json.get("createTime"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let network = json.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("toAddress").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tag = json.get("tag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let amount = optional_decimal(json, "amount").unwrap_or(Decimal::ZERO);
    let coin = json.get("coin").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Status: 0=unknown, 1=toConfirm, 2=processing, 3=success, 4=deposit failed
    let status_code = json
        .get("status")
        .and_then(|v| v.as_i64())
        .or_else(|| json.get("status").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()))
        .unwrap_or(0);
    let status = match status_code {
        3 => TransactionStatus::Ok,
        4 => TransactionStatus::Failed,
        _ => TransactionStatus::Pending,
    };

    Ok(Deposit {
        id,
        txid,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        network,
        address,
        tag,
        transaction_type: TransactionType::Deposit,
        amount,
        currency: coin,
        status,
        updated: None,
        fee: None,
        info: Some(json.clone()),
    })
}

/// Parse Bybit withdrawal record
pub fn parse_withdrawal(json: &Value) -> Result<Withdrawal> {
    let id = json
        .get("withdrawId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let txid = json.get("txID").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());

    let timestamp = json
        .get("createTime")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let network = json.get("chain").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("toAddress").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tag = json.get("tag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let amount = optional_decimal(json, "amount").unwrap_or(Decimal::ZERO);
    let coin = json.get("coin").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Status: SecurityCheck, Pending, success, CancelByUser, Reject, Fail, BlockchainConfirmed
    let status_str = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let status = match status_str {
        "success" | "BlockchainConfirmed" => TransactionStatus::Ok,
        "CancelByUser" => TransactionStatus::Canceled,
        "Reject" | "Fail" => TransactionStatus::Failed,
        _ => TransactionStatus::Pending,
    };

    let withdraw_fee = optional_decimal(json, "withdrawFee");
    let fee = withdraw_fee.map(|cost| deposit::TransactionFee {
        cost,
        currency: coin.clone(),
    });

    Ok(Withdrawal {
        id,
        txid,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        network,
        address,
        tag,
        transaction_type: TransactionType::Withdrawal,
        amount,
        currency: coin,
        status,
        updated: None,
        fee,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion() {
        assert_eq!(symbol_to_bybit("BTC/USDT"), "BTCUSDT");
        assert_eq!(symbol_to_bybit("ETH/USDC"), "ETHUSDC");

        assert_eq!(symbol_from_bybit("BTCUSDT"), "BTC/USDT");
        assert_eq!(symbol_from_bybit("ETHUSDC"), "ETH/USDC");
        assert_eq!(symbol_from_bybit("ETHBTC"), "ETH/BTC");
    }

    #[test]
    fn test_symbol_linear_conversion() {
        assert_eq!(symbol_to_bybit("BTC/USDT:USDT"), "BTCUSDT");
        assert_eq!(symbol_from_bybit_linear("BTCUSDT"), "BTC/USDT:USDT");
        assert_eq!(symbol_from_bybit_linear("ETHUSDT"), "ETH/USDT:USDT");
    }

    #[test]
    fn test_timeframe_conversion() {
        assert_eq!(timeframe_to_bybit(&Timeframe::OneMinute), "1");
        assert_eq!(timeframe_to_bybit(&Timeframe::FiveMinutes), "5");
        assert_eq!(timeframe_to_bybit(&Timeframe::OneHour), "60");
        assert_eq!(timeframe_to_bybit(&Timeframe::OneDay), "D");
    }

    #[test]
    fn test_count_decimals() {
        assert_eq!(count_decimals("0.01"), 2);
        assert_eq!(count_decimals("0.001"), 3);
        assert_eq!(count_decimals("1"), 0);
    }

    #[test]
    fn test_parse_order_new() {
        let json = serde_json::json!({
            "orderId": "1234567890",
            "orderLinkId": "test_link",
            "symbol": "BTCUSDT",
            "side": "Buy",
            "orderType": "Limit",
            "price": "50000.00",
            "qty": "0.001",
            "cumExecQty": "0.000",
            "cumExecValue": "0.00",
            "orderStatus": "New",
            "timeInForce": "GTC",
            "createdTime": "1700000000000",
            "updatedTime": "1700000000000"
        });

        let order = parse_order(&json, "BTC/USDT").unwrap();
        assert_eq!(order.id, "1234567890");
        assert_eq!(order.client_order_id.as_deref(), Some("test_link"));
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.amount, Decimal::from_str("0.001").unwrap());
        assert_eq!(order.filled, Some(Decimal::ZERO));
    }

    #[test]
    fn test_parse_order_filled() {
        let json = serde_json::json!({
            "orderId": "99999",
            "symbol": "ETHUSDT",
            "side": "Sell",
            "orderType": "Market",
            "qty": "1.0",
            "cumExecQty": "1.0",
            "cumExecValue": "3000.00",
            "cumExecFee": "1.80",
            "orderStatus": "Filled",
            "createdTime": "1700000000000",
            "updatedTime": "1700000000000"
        });

        let order = parse_order(&json, "ETH/USDT").unwrap();
        assert_eq!(order.status, OrderStatus::Closed);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.cost, Some(Decimal::from_str("3000.00").unwrap()));
        assert_eq!(order.average, Some(Decimal::from_str("3000.00").unwrap()));
        assert_eq!(order.remaining, Some(Decimal::ZERO));
        assert!(order.fee.is_some());
    }

    #[test]
    fn test_parse_balance() {
        let json = serde_json::json!({
            "accountType": "UNIFIED",
            "coin": [
                {"coin": "BTC", "walletBalance": "0.5", "availableToWithdraw": "0.4"},
                {"coin": "USDT", "walletBalance": "10000.0", "availableToWithdraw": "8000.0"},
                {"coin": "ETH", "walletBalance": "0.0", "availableToWithdraw": "0.0"}
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
            "symbol": "BTCUSDT",
            "side": "Buy",
            "size": "0.1",
            "avgPrice": "48000.0",
            "markPrice": "50000.0",
            "unrealisedPnl": "200.0",
            "liqPrice": "40000.0",
            "leverage": "10",
            "tradeMode": 1,
            "positionValue": "5000.0",
            "positionIM": "500.0",
            "positionMM": "25.0",
            "updatedTime": "1700000000000"
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
            "symbol": "BTCUSDT",
            "lastPrice": "50000.00",
            "markPrice": "50000.00",
            "indexPrice": "49990.00",
            "fundingRate": "0.0001",
            "nextFundingTime": "1700028800000"
        });

        let fr = parse_funding_rate(&json, "BTC/USDT:USDT").unwrap();
        assert_eq!(fr.funding_rate, Decimal::from_str("0.0001").unwrap());
        assert_eq!(fr.next_funding_timestamp, Some(1700028800000));
    }

    #[test]
    fn test_parse_my_trade() {
        let json = serde_json::json!({
            "execId": "trade123",
            "orderId": "order456",
            "side": "Buy",
            "execPrice": "50000.0",
            "execQty": "0.001",
            "execValue": "50.0",
            "execFee": "0.03",
            "feeCurrency": "USDT",
            "isMaker": "false",
            "execTime": "1700000000000"
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
            "id": "dep123",
            "txID": "0xabc",
            "coin": "USDT",
            "chain": "ETH",
            "toAddress": "0x1234",
            "amount": "100.5",
            "status": 3,
            "successAt": "1700000000000"
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
            "withdrawId": "wd999",
            "txID": "0xdef",
            "coin": "ETH",
            "chain": "ETH",
            "toAddress": "0xabcdef",
            "amount": "1.5",
            "withdrawFee": "0.005",
            "status": "success",
            "createTime": "1700000000000"
        });

        let wd = parse_withdrawal(&json).unwrap();
        assert_eq!(wd.id, "wd999");
        assert_eq!(wd.currency, "ETH");
        assert_eq!(wd.amount, Decimal::from_str("1.5").unwrap());
        assert_eq!(wd.status, TransactionStatus::Ok);
        assert!(wd.fee.is_some());
    }

    #[test]
    fn test_parse_linear_market() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "baseCoin": "BTC",
            "quoteCoin": "USDT",
            "settleCoin": "USDT",
            "contractType": "LinearPerpetual",
            "status": "Trading",
            "lotSizeFilter": {
                "minOrderQty": "0.001",
                "maxOrderQty": "100.0",
                "qtyStep": "0.001"
            },
            "priceFilter": {
                "minPrice": "0.10",
                "maxPrice": "199999.80",
                "tickSize": "0.10"
            },
            "leverageFilter": {
                "minLeverage": "1",
                "maxLeverage": "100"
            }
        });

        let market = parse_linear_market(&json).unwrap();
        assert_eq!(market.symbol, "BTC/USDT:USDT");
        assert_eq!(market.id, "BTCUSDT");
        assert!(market.swap);
        assert!(!market.spot);
        assert_eq!(market.settle, Some("USDT".to_string()));
        assert_eq!(market.contract, Some(true));
        assert_eq!(market.linear, Some(true));
    }
}
