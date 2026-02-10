//! Binance data parsing utilities
//!
//! Functions to convert Binance API responses to unified ccxt types.

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

/// Extract an optional decimal field from a JSON object by key
fn optional_decimal(json: &Value, key: &str) -> Result<Option<Decimal>> {
    json.get(key)
        .and_then(|v| v.as_str())
        .map(parse_decimal)
        .transpose()
}

/// Extract an optional decimal from either string or number JSON value
fn optional_decimal_flexible(json: &Value, key: &str) -> Option<Decimal> {
    json.get(key).and_then(|v| {
        if let Some(s) = v.as_str() {
            Decimal::from_str(s).ok()
        } else if let Some(f) = v.as_f64() {
            Decimal::try_from(f).ok()
        } else {
            v.as_i64().map(Decimal::from)
        }
    })
}

// ============================================================================
// Symbol Conversion
// ============================================================================

/// Convert unified symbol to Binance format
///
/// Handles both spot ("BTC/USDT" -> "BTCUSDT") and
/// futures ("BTC/USDT:USDT" -> "BTCUSDT") symbols.
///
/// # Example
/// ```
/// use ccxt::binance::parsers::symbol_to_binance;
/// assert_eq!(symbol_to_binance("BTC/USDT"), "BTCUSDT");
/// assert_eq!(symbol_to_binance("BTC/USDT:USDT"), "BTCUSDT");
/// ```
pub fn symbol_to_binance(symbol: &str) -> String {
    // Strip settle currency (after ':') for futures
    let base_symbol = symbol.split(':').next().unwrap_or(symbol);
    base_symbol.replace('/', "")
}

/// Convert Binance spot symbol to unified format
///
/// Uses a heuristic to split the symbol (looks for common quote currencies)
///
/// # Example
/// ```
/// use ccxt::binance::parsers::symbol_from_binance;
/// assert_eq!(symbol_from_binance("BTCUSDT"), "BTC/USDT");
/// assert_eq!(symbol_from_binance("ETHBTC"), "ETH/BTC");
/// ```
pub fn symbol_from_binance(binance_symbol: &str) -> String {
    let quote_currencies = ["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB", "EUR", "GBP", "TRY", "DAI"];

    for quote in &quote_currencies {
        if binance_symbol.ends_with(quote) {
            let quote_byte_len = quote.len();
            let total_byte_len = binance_symbol.len();

            if total_byte_len > quote_byte_len {
                let base = &binance_symbol[..total_byte_len - quote_byte_len];
                if !base.is_empty() {
                    return format!("{}/{}", base, quote);
                }
            }
        }
    }

    binance_symbol.to_string()
}

/// Convert Binance futures symbol to unified format with settle currency
///
/// # Example
/// ```
/// use ccxt::binance::parsers::symbol_from_binance_futures;
/// assert_eq!(symbol_from_binance_futures("BTCUSDT"), "BTC/USDT:USDT");
/// assert_eq!(symbol_from_binance_futures("ETHUSDT"), "ETH/USDT:USDT");
/// ```
pub fn symbol_from_binance_futures(binance_symbol: &str) -> String {
    let spot = symbol_from_binance(binance_symbol);
    if spot.contains('/') {
        let quote = spot.split('/').next_back().unwrap_or("USDT");
        format!("{}:{}", spot, quote)
    } else {
        spot
    }
}

/// Count decimal places from Binance tick/step size
///
/// # Example
/// ```
/// use ccxt::binance::parsers::count_decimals;
/// assert_eq!(count_decimals("0.01"), 2);
/// assert_eq!(count_decimals("0.00001"), 5);
/// assert_eq!(count_decimals("1"), 0);
/// ```
pub fn count_decimals(value_str: &str) -> i32 {
    if let Some(dot_pos) = value_str.find('.') {
        value_str[dot_pos + 1..].trim_end_matches('0').len() as i32
    } else {
        0
    }
}

/// Convert unified Timeframe to Binance interval string
pub fn timeframe_to_binance(timeframe: Timeframe) -> &'static str {
    match timeframe {
        Timeframe::OneMinute => "1m",
        Timeframe::ThreeMinutes => "3m",
        Timeframe::FiveMinutes => "5m",
        Timeframe::FifteenMinutes => "15m",
        Timeframe::ThirtyMinutes => "30m",
        Timeframe::OneHour => "1h",
        Timeframe::TwoHours => "2h",
        Timeframe::FourHours => "4h",
        Timeframe::SixHours => "6h",
        Timeframe::EightHours => "8h",
        Timeframe::TwelveHours => "12h",
        Timeframe::OneDay => "1d",
        Timeframe::ThreeDays => "3d",
        Timeframe::OneWeek => "1w",
        Timeframe::OneMonth => "1M",
    }
}

// ============================================================================
// Public Data Parsers
// ============================================================================

/// Parse Binance ticker to unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = json
        .get("closeTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing closeTime in ticker".to_string()))?;

    let high = optional_decimal(json, "highPrice")?;
    let low = optional_decimal(json, "lowPrice")?;

    let average = match (high, low) {
        (Some(h), Some(l)) => Some((h + l) / Decimal::from(2)),
        _ => None,
    };

    let last_price = optional_decimal(json, "lastPrice")?;

    Ok(Ticker {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        high,
        low,
        bid: optional_decimal(json, "bidPrice")?,
        bid_volume: optional_decimal(json, "bidQty")?,
        ask: optional_decimal(json, "askPrice")?,
        ask_volume: optional_decimal(json, "askQty")?,
        vwap: optional_decimal(json, "weightedAvgPrice")?,
        open: optional_decimal(json, "openPrice")?,
        close: last_price,
        last: last_price,
        previous_close: optional_decimal(json, "prevClosePrice")?,
        change: optional_decimal(json, "priceChange")?,
        percentage: optional_decimal(json, "priceChangePercent")?,
        average,
        base_volume: optional_decimal(json, "volume")?,
        quote_volume: optional_decimal(json, "quoteVolume")?,
        index_price: None,
        mark_price: None,
        info: Some(json.clone()),
    })
}

/// Parse Binance order book to unified OrderBook
pub fn parse_order_book(json: &Value, symbol: &str) -> Result<OrderBook> {
    let bids = json
        .get("bids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing bids in order book".to_string()))?;

    let asks = json
        .get("asks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing asks in order book".to_string()))?;

    let mut parsed_bids = Vec::new();
    for bid in bids {
        if let Some(arr) = bid.as_array() {
            if arr.len() >= 2 {
                let price = arr[0].as_str().ok_or_else(|| CcxtError::ParseError("Invalid bid price".to_string()))?;
                let amount = arr[1].as_str().ok_or_else(|| CcxtError::ParseError("Invalid bid amount".to_string()))?;
                parsed_bids.push((parse_decimal(price)?, parse_decimal(amount)?));
            }
        }
    }

    let mut parsed_asks = Vec::new();
    for ask in asks {
        if let Some(arr) = ask.as_array() {
            if arr.len() >= 2 {
                let price = arr[0].as_str().ok_or_else(|| CcxtError::ParseError("Invalid ask price".to_string()))?;
                let amount = arr[1].as_str().ok_or_else(|| CcxtError::ParseError("Invalid ask amount".to_string()))?;
                parsed_asks.push((parse_decimal(price)?, parse_decimal(amount)?));
            }
        }
    }

    let timestamp = chrono::Utc::now().timestamp_millis();
    let nonce = json.get("lastUpdateId").and_then(|v| v.as_u64());

    Ok(OrderBook {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        nonce,
        bids: parsed_bids,
        asks: parsed_asks,
        info: Some(json.clone()),
    })
}

/// Parse Binance trade to unified Trade (public trades)
pub fn parse_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing id in trade".to_string()))?
        .to_string();

    let timestamp = json
        .get("time")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing time in trade".to_string()))?;

    let is_buyer_maker = json
        .get("isBuyerMaker")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| CcxtError::ParseError("Missing isBuyerMaker in trade".to_string()))?;

    let side = if is_buyer_maker {
        OrderSide::Sell
    } else {
        OrderSide::Buy
    };

    let price = json
        .get("price")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing price in trade".to_string()))?;

    let amount = json
        .get("qty")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing qty in trade".to_string()))?;

    let cost = json
        .get("quoteQty")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteQty in trade".to_string()))?;

    Ok(Trade {
        id,
        symbol: symbol.to_string(),
        order: None,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        side,
        price: parse_decimal(price)?,
        amount: parse_decimal(amount)?,
        cost: parse_decimal(cost)?,
        fee: None,
        taker_or_maker: None,
        info: Some(json.clone()),
    })
}

/// Parse Binance kline (OHLCV) to unified OHLCV
pub fn parse_ohlcv(json: &Value) -> Result<OHLCV> {
    let arr = json
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("Kline must be an array".to_string()))?;

    if arr.len() < 6 {
        return Err(CcxtError::ParseError("Kline array too short".to_string()));
    }

    let timestamp = arr[0]
        .as_i64()
        .ok_or_else(|| CcxtError::ParseError("Invalid timestamp in kline".to_string()))?;

    let open = arr[1].as_str().ok_or_else(|| CcxtError::ParseError("Invalid open in kline".to_string()))?;
    let high = arr[2].as_str().ok_or_else(|| CcxtError::ParseError("Invalid high in kline".to_string()))?;
    let low = arr[3].as_str().ok_or_else(|| CcxtError::ParseError("Invalid low in kline".to_string()))?;
    let close = arr[4].as_str().ok_or_else(|| CcxtError::ParseError("Invalid close in kline".to_string()))?;
    let volume = arr[5].as_str().ok_or_else(|| CcxtError::ParseError("Invalid volume in kline".to_string()))?;

    Ok(OHLCV {
        timestamp,
        open: parse_decimal(open)?,
        high: parse_decimal(high)?,
        low: parse_decimal(low)?,
        close: parse_decimal(close)?,
        volume: parse_decimal(volume)?,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Market Parsers
// ============================================================================

/// Parse Binance spot exchangeInfo symbol to unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let symbol_str = json
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in exchangeInfo".to_string()))?;

    let base = json
        .get("baseAsset")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing baseAsset".to_string()))?;

    let quote = json
        .get("quoteAsset")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteAsset".to_string()))?;

    let symbol = format!("{}/{}", base, quote);

    let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("TRADING");
    let active = status == "TRADING";

    let is_spot = json.get("isSpotTradingAllowed").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_margin = json.get("isMarginTradingAllowed").and_then(|v| v.as_bool()).unwrap_or(false);

    let filters = json.get("filters").and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing filters".to_string()))?;

    let mut price_precision = None;
    let mut amount_precision = None;
    let mut min_price = None;
    let mut max_price = None;
    let mut min_amount = None;
    let mut max_amount = None;
    let mut min_cost = None;
    let mut max_cost = None;

    for filter in filters {
        let filter_type = filter.get("filterType").and_then(|v| v.as_str());
        match filter_type {
            Some("PRICE_FILTER") => {
                if let Some(tick_size) = filter.get("tickSize").and_then(|v| v.as_str()) {
                    price_precision = Some(count_decimals(tick_size));
                    min_price = filter.get("minPrice").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                    max_price = filter.get("maxPrice").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                }
            }
            Some("LOT_SIZE") => {
                if let Some(step_size) = filter.get("stepSize").and_then(|v| v.as_str()) {
                    amount_precision = Some(count_decimals(step_size));
                    min_amount = filter.get("minQty").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                    max_amount = filter.get("maxQty").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                }
            }
            Some("NOTIONAL") | Some("MIN_NOTIONAL") => {
                min_cost = filter.get("minNotional").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                max_cost = filter.get("maxNotional").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
            }
            _ => {}
        }
    }

    Ok(Market {
        id: symbol_str.to_string(),
        symbol,
        base: base.to_string(),
        quote: quote.to_string(),
        settle: None,
        base_id: base.to_string(),
        quote_id: quote.to_string(),
        settle_id: None,
        market_type: "spot".to_string(),
        spot: is_spot,
        margin: is_margin,
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
            base: None,
            quote: None,
        },
        limits: MarketLimits {
            amount: Some(MinMax { min: min_amount, max: max_amount }),
            price: Some(MinMax { min: min_price, max: max_price }),
            cost: Some(MinMax { min: min_cost, max: max_cost }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

/// Parse Binance futures exchangeInfo symbol to unified Market
pub fn parse_futures_market(json: &Value) -> Result<Market> {
    let symbol_str = json
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in futures exchangeInfo".to_string()))?;

    let base = json
        .get("baseAsset")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing baseAsset".to_string()))?;

    let quote = json
        .get("quoteAsset")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing quoteAsset".to_string()))?;

    let margin_asset = json
        .get("marginAsset")
        .and_then(|v| v.as_str())
        .unwrap_or(quote);

    // Only handle PERPETUAL contracts
    let contract_type = json.get("contractType").and_then(|v| v.as_str()).unwrap_or("");
    if contract_type != "PERPETUAL" {
        return Err(CcxtError::ParseError(format!("Skipping non-perpetual contract: {}", contract_type)));
    }

    let symbol = format!("{}/{}:{}", base, quote, margin_asset);

    let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("TRADING");
    let active = status == "TRADING";

    let filters = json.get("filters").and_then(|v| v.as_array());

    let mut price_precision = None;
    let mut amount_precision = None;
    let mut min_price = None;
    let mut max_price = None;
    let mut min_amount = None;
    let mut max_amount = None;
    let mut min_cost = None;

    if let Some(filters) = filters {
        for filter in filters {
            let filter_type = filter.get("filterType").and_then(|v| v.as_str());
            match filter_type {
                Some("PRICE_FILTER") => {
                    if let Some(tick_size) = filter.get("tickSize").and_then(|v| v.as_str()) {
                        price_precision = Some(count_decimals(tick_size));
                        min_price = filter.get("minPrice").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                        max_price = filter.get("maxPrice").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                    }
                }
                Some("LOT_SIZE") => {
                    if let Some(step_size) = filter.get("stepSize").and_then(|v| v.as_str()) {
                        amount_precision = Some(count_decimals(step_size));
                        min_amount = filter.get("minQty").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                        max_amount = filter.get("maxQty").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                    }
                }
                Some("MIN_NOTIONAL") => {
                    min_cost = filter.get("notional").and_then(|v| v.as_str()).map(parse_decimal).transpose()?;
                }
                _ => {}
            }
        }
    }

    Ok(Market {
        id: symbol_str.to_string(),
        symbol,
        base: base.to_string(),
        quote: quote.to_string(),
        settle: Some(margin_asset.to_string()),
        base_id: base.to_string(),
        quote_id: quote.to_string(),
        settle_id: Some(margin_asset.to_string()),
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
            amount: Some(MinMax { min: min_amount, max: max_amount }),
            price: Some(MinMax { min: min_price, max: max_price }),
            cost: Some(MinMax { min: min_cost, max: None }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

// ============================================================================
// Order Parser
// ============================================================================

/// Parse Binance order response (spot or futures) to unified Order
pub fn parse_order(json: &Value, symbol: &str, _is_futures: bool) -> Result<Order> {
    let order_id = json.get("orderId")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_default();

    let client_order_id = json.get("clientOrderId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let timestamp = json.get("time")
        .or_else(|| json.get("transactTime"))
        .or_else(|| json.get("updateTime"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let update_time = json.get("updateTime")
        .and_then(|v| v.as_i64());

    // Status mapping
    let status_str = json.get("status").and_then(|v| v.as_str()).unwrap_or("NEW");
    let status = match status_str {
        "NEW" => OrderStatus::Open,
        "PARTIALLY_FILLED" => OrderStatus::Open,
        "FILLED" => OrderStatus::Closed,
        "CANCELED" => OrderStatus::Canceled,
        "EXPIRED" => OrderStatus::Expired,
        "REJECTED" => OrderStatus::Rejected,
        _ => OrderStatus::Open,
    };

    // Type mapping
    let type_str = json.get("type").and_then(|v| v.as_str()).unwrap_or("LIMIT");
    let order_type = match type_str {
        "LIMIT" => OrderType::Limit,
        "MARKET" => OrderType::Market,
        "STOP_LOSS" | "STOP" => OrderType::StopLoss,
        "STOP_LOSS_LIMIT" | "STOP_MARKET" => OrderType::StopLossLimit,
        "TAKE_PROFIT" => OrderType::TakeProfit,
        "TAKE_PROFIT_LIMIT" | "TAKE_PROFIT_MARKET" => OrderType::TakeProfitLimit,
        "TRAILING_STOP_MARKET" => OrderType::TrailingStop,
        _ => OrderType::Limit,
    };

    // Side mapping
    let side_str = json.get("side").and_then(|v| v.as_str()).unwrap_or("BUY");
    let side = if side_str == "SELL" { OrderSide::Sell } else { OrderSide::Buy };

    // Amounts
    let orig_qty = optional_decimal_flexible(json, "origQty")
        .ok_or_else(|| CcxtError::ParseError("Missing field: origQty in order".into()))?;
    let executed_qty = optional_decimal_flexible(json, "executedQty")
        .ok_or_else(|| CcxtError::ParseError("Missing field: executedQty in order".into()))?;
    let cumulative_quote = optional_decimal_flexible(json, "cummulativeQuoteQty")
        .or_else(|| optional_decimal_flexible(json, "cumQuote"));

    let remaining = orig_qty - executed_qty;
    let cost = cumulative_quote;
    let average = if executed_qty > Decimal::ZERO {
        cost.map(|c| c / executed_qty)
    } else {
        None
    };

    let price = optional_decimal_flexible(json, "price");
    let stop_price = optional_decimal_flexible(json, "stopPrice")
        .filter(|p| !p.is_zero());

    // Time in force
    let tif = json.get("timeInForce").and_then(|v| v.as_str()).map(|s| match s {
        "GTC" => TimeInForce::Gtc,
        "IOC" => TimeInForce::Ioc,
        "FOK" => TimeInForce::Fok,
        _ => TimeInForce::Gtc,
    });

    let reduce_only = json.get("reduceOnly").and_then(|v| v.as_bool());

    // Parse fills for fee aggregation (from create_order response)
    let fee = parse_fills_fee(json);

    Ok(Order {
        id: order_id,
        client_order_id,
        symbol: symbol.to_string(),
        order_type,
        side,
        status,
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        last_trade_timestamp: update_time,
        price,
        average,
        amount: orig_qty,
        filled: Some(executed_qty),
        remaining: Some(remaining),
        cost,
        fee,
        time_in_force: tif,
        post_only: None,
        reduce_only,
        stop_price,
        trigger_price: stop_price,
        stop_loss_price: None,
        take_profit_price: None,
        last_update_timestamp: update_time,
        trades: None,
        info: Some(json.clone()),
    })
}

/// Parse fills array from create_order response to aggregate fee
fn parse_fills_fee(json: &Value) -> Option<OrderFee> {
    let fills = json.get("fills").and_then(|v| v.as_array())?;
    if fills.is_empty() {
        return None;
    }

    let mut total_fee = Decimal::ZERO;
    let mut fee_currency = String::new();

    for fill in fills {
        if let Some(commission) = fill.get("commission").and_then(|v| v.as_str()).and_then(|s| Decimal::from_str(s).ok()) {
            total_fee += commission;
        }
        if fee_currency.is_empty() {
            if let Some(asset) = fill.get("commissionAsset").and_then(|v| v.as_str()) {
                fee_currency = asset.to_string();
            }
        }
    }

    if fee_currency.is_empty() {
        return None;
    }

    Some(OrderFee {
        cost: total_fee,
        currency: fee_currency,
        rate: None,
    })
}

// ============================================================================
// Balance Parser
// ============================================================================

/// Parse Binance spot account response to unified Balances
pub fn parse_balance_spot(json: &Value) -> Result<Balances> {
    let balances_array = json.get("balances")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing balances in account".to_string()))?;

    let timestamp = json.get("updateTime")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let mut balances = HashMap::new();
    let mut free_map = HashMap::new();
    let mut used_map = HashMap::new();
    let mut total_map = HashMap::new();

    for bal in balances_array {
        let asset = bal.get("asset").and_then(|v| v.as_str()).unwrap_or("");
        let free = bal.get("free").and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);
        let locked = bal.get("locked").and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);

        // Skip zero balances
        if free.is_zero() && locked.is_zero() {
            continue;
        }

        let total = free + locked;
        balances.insert(asset.to_string(), Balance::new(asset.to_string(), free, locked));
        free_map.insert(asset.to_string(), free);
        used_map.insert(asset.to_string(), locked);
        total_map.insert(asset.to_string(), total);
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

/// Parse Binance futures account response to unified Balances
pub fn parse_balance_futures(json: &Value) -> Result<Balances> {
    let assets_array = json.get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CcxtError::ParseError("Missing assets in futures account".to_string()))?;

    let timestamp = json.get("updateTime")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let mut balances = HashMap::new();
    let mut free_map = HashMap::new();
    let mut used_map = HashMap::new();
    let mut total_map = HashMap::new();

    for asset_json in assets_array {
        let asset = asset_json.get("asset").and_then(|v| v.as_str()).unwrap_or("");
        let wallet_balance = asset_json.get("walletBalance").and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);
        let available = asset_json.get("availableBalance").and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ZERO);

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

/// Parse a user trade (from /api/v3/myTrades or /fapi/v1/userTrades)
pub fn parse_my_trade(json: &Value, symbol: &str) -> Result<Trade> {
    let id = json.get("id")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string())
        .unwrap_or_default();

    let order_id = json.get("orderId")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string());

    let timestamp = json.get("time")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let is_buyer = json.get("isBuyer")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let side = if is_buyer { OrderSide::Buy } else { OrderSide::Sell };

    let is_maker = json.get("isMaker")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let taker_or_maker = if is_maker { "maker" } else { "taker" };

    let price = optional_decimal_flexible(json, "price")
        .ok_or_else(|| CcxtError::ParseError("Missing field: price in my_trade".into()))?;
    let qty = optional_decimal_flexible(json, "qty")
        .ok_or_else(|| CcxtError::ParseError("Missing field: qty in my_trade".into()))?;
    let quote_qty = optional_decimal_flexible(json, "quoteQty")
        .unwrap_or_else(|| price * qty);

    let commission = optional_decimal_flexible(json, "commission")
        .ok_or_else(|| CcxtError::ParseError("Missing field: commission in my_trade".into()))?;
    let commission_asset = json.get("commissionAsset")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let fee = if !commission_asset.is_empty() {
        Some(TradeFee {
            cost: commission,
            currency: commission_asset,
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
        cost: quote_qty,
        fee,
        taker_or_maker: Some(taker_or_maker.to_string()),
        info: Some(json.clone()),
    })
}

// ============================================================================
// Position Parser
// ============================================================================

/// Parse Binance futures position risk to unified Position
pub fn parse_position(json: &Value) -> Result<Position> {
    let binance_symbol = json.get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing symbol in position".to_string()))?;

    let symbol = symbol_from_binance_futures(binance_symbol);

    let position_amt = optional_decimal_flexible(json, "positionAmt")
        .ok_or_else(|| CcxtError::ParseError("Missing field: positionAmt in position".into()))?;

    let side = if position_amt > Decimal::ZERO {
        PositionSide::Long
    } else if position_amt < Decimal::ZERO {
        PositionSide::Short
    } else {
        PositionSide::Both
    };

    let margin_type = json.get("marginType")
        .and_then(|v| v.as_str())
        .unwrap_or("cross");
    let margin_mode = if margin_type == "isolated" {
        MarginMode::Isolated
    } else {
        MarginMode::Cross
    };

    let entry_price = optional_decimal_flexible(json, "entryPrice");
    let mark_price = optional_decimal_flexible(json, "markPrice");
    let unrealized_pnl = optional_decimal_flexible(json, "unRealizedProfit");
    let liquidation_price = optional_decimal_flexible(json, "liquidationPrice")
        .filter(|p| !p.is_zero());
    let leverage = optional_decimal_flexible(json, "leverage");
    let notional = optional_decimal_flexible(json, "notional").map(|n| n.abs());
    let initial_margin = optional_decimal_flexible(json, "initialMargin");
    let maintenance_margin = optional_decimal_flexible(json, "maintMargin");
    let update_time = json.get("updateTime").and_then(|v| v.as_i64());

    let now = chrono::Utc::now().timestamp_millis();

    Ok(Position {
        symbol,
        id: None,
        timestamp: update_time.unwrap_or(now),
        datetime: timestamp_to_iso8601(update_time.unwrap_or(now)),
        side,
        margin_mode,
        contracts: position_amt.abs(),
        contract_size: Some(Decimal::ONE),
        notional,
        leverage,
        entry_price,
        mark_price,
        unrealized_pnl,
        realized_pnl: None,
        collateral: None,
        initial_margin,
        maintenance_margin,
        liquidation_price,
        margin_ratio: None,
        percentage: None,
        stop_loss_price: None,
        take_profit_price: None,
        hedged: None,
        maintenance_margin_percentage: None,
        initial_margin_percentage: None,
        last_update_timestamp: update_time,
        last_price: None,
        info: Some(json.clone()),
    })
}

// ============================================================================
// Funding Rate Parser
// ============================================================================

/// Parse Binance premiumIndex to unified FundingRate
pub fn parse_funding_rate(json: &Value, symbol: &str) -> Result<FundingRate> {
    let timestamp = json.get("time")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing field: time in funding rate".into()))?;

    let funding_rate = optional_decimal_flexible(json, "lastFundingRate")
        .ok_or_else(|| CcxtError::ParseError("Missing field: lastFundingRate in funding rate".into()))?;

    let next_funding_time = json.get("nextFundingTime").and_then(|v| v.as_i64());

    Ok(FundingRate {
        symbol: symbol.to_string(),
        timestamp,
        datetime: timestamp_to_iso8601(timestamp),
        funding_rate,
        funding_timestamp: next_funding_time,
        funding_datetime: next_funding_time.map(timestamp_to_iso8601),
        mark_price: optional_decimal_flexible(json, "markPrice"),
        index_price: optional_decimal_flexible(json, "indexPrice"),
        interest_rate: optional_decimal_flexible(json, "interestRate"),
        estimated_settle_price: optional_decimal_flexible(json, "estimatedSettlePrice"),
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

/// Parse Binance capital config to unified Currency
pub fn parse_currency(json: &Value) -> Result<Currency> {
    let coin = json.get("coin")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing coin in currency".to_string()))?;

    let name = json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());

    let is_legal_money = json.get("isLegalMoney").and_then(|v| v.as_bool()).unwrap_or(false);
    let trading = json.get("trading").and_then(|v| v.as_bool()).unwrap_or(true);
    let active = trading && !is_legal_money;

    let network_list = json.get("networkList").and_then(|v| v.as_array());

    let mut networks = Vec::new();
    let mut default_deposit = false;
    let mut default_withdraw = false;
    let mut default_fee: Option<Decimal> = None;

    if let Some(net_list) = network_list {
        for net in net_list {
            let network_id = net.get("network").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let net_name = net.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            let deposit_enable = net.get("depositEnable").and_then(|v| v.as_bool()).unwrap_or(false);
            let withdraw_enable = net.get("withdrawEnable").and_then(|v| v.as_bool()).unwrap_or(false);
            let withdraw_fee = optional_decimal_flexible(net, "withdrawFee");

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
                id: network_id.clone(),
                network: network_id,
                name: net_name,
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
        active,
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

/// Parse Binance deposit history entry
pub fn parse_deposit(json: &Value) -> Result<Deposit> {
    let id = json.get("id")
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|i| i.to_string())))
        .ok_or_else(|| CcxtError::ParseError("Missing field: id in deposit".into()))?;

    let txid = json.get("txId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timestamp = json.get("insertTime").and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing field: insertTime in deposit".into()))?;
    let network = json.get("network").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("address").and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing field: address in deposit".into()))?
        .to_string();
    let tag = json.get("addressTag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let amount = optional_decimal_flexible(json, "amount")
        .ok_or_else(|| CcxtError::ParseError("Missing field: amount in deposit".into()))?;
    let coin = json.get("coin").and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing field: coin in deposit".into()))?
        .to_string();

    // Status: 0=pending, 1=ok, 6=credited
    let status_code = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    let status = match status_code {
        1 | 6 => TransactionStatus::Ok,
        0 => TransactionStatus::Pending,
        _ => TransactionStatus::Failed,
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

/// Parse Binance withdrawal history entry
pub fn parse_withdrawal(json: &Value) -> Result<Withdrawal> {
    let id = json.get("id")
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|i| i.to_string())))
        .ok_or_else(|| CcxtError::ParseError("Missing field: id in withdrawal".into()))?;

    let txid = json.get("txId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let timestamp = json.get("applyTime")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing field: applyTime in withdrawal".into()))
        .and_then(|s| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc().timestamp_millis())
                .map_err(|_| CcxtError::ParseError(format!("Invalid applyTime format: {}", s)))
        })?;
    let network = json.get("network").and_then(|v| v.as_str()).map(|s| s.to_string());
    let address = json.get("address").and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing field: address in withdrawal".into()))?
        .to_string();
    let tag = json.get("addressTag").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
    let amount = optional_decimal_flexible(json, "amount")
        .ok_or_else(|| CcxtError::ParseError("Missing field: amount in withdrawal".into()))?;
    let coin = json.get("coin").and_then(|v| v.as_str())
        .ok_or_else(|| CcxtError::ParseError("Missing field: coin in withdrawal".into()))?
        .to_string();

    // Status: 0=email sent, 1=cancelled, 2=awaiting, 3=rejected, 4=processing, 5=failure, 6=completed
    let status_code = json.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
    let status = match status_code {
        6 => TransactionStatus::Ok,
        1 => TransactionStatus::Canceled,
        3 | 5 => TransactionStatus::Failed,
        _ => TransactionStatus::Pending,
    };

    let transaction_fee = optional_decimal_flexible(json, "transactionFee");
    let fee = transaction_fee.map(|cost| deposit::TransactionFee {
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
        assert_eq!(symbol_to_binance("BTC/USDT"), "BTCUSDT");
        assert_eq!(symbol_to_binance("ETH/BTC"), "ETHBTC");

        assert_eq!(symbol_from_binance("BTCUSDT"), "BTC/USDT");
        assert_eq!(symbol_from_binance("ETHBTC"), "ETH/BTC");
    }

    #[test]
    fn test_symbol_futures_conversion() {
        assert_eq!(symbol_to_binance("BTC/USDT:USDT"), "BTCUSDT");
        assert_eq!(symbol_from_binance_futures("BTCUSDT"), "BTC/USDT:USDT");
        assert_eq!(symbol_from_binance_futures("ETHUSDT"), "ETH/USDT:USDT");
    }

    #[test]
    fn test_count_decimals() {
        assert_eq!(count_decimals("0.01"), 2);
        assert_eq!(count_decimals("0.00001"), 5);
        assert_eq!(count_decimals("1"), 0);
        assert_eq!(count_decimals("0.10"), 1);
    }

    #[test]
    fn test_parse_order_spot_new() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "orderId": 12345,
            "clientOrderId": "test123",
            "transactTime": 1700000000000_i64,
            "price": "50000.00",
            "origQty": "0.001",
            "executedQty": "0.000",
            "cummulativeQuoteQty": "0.00",
            "status": "NEW",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "BUY"
        });

        let order = parse_order(&json, "BTC/USDT", false).unwrap();
        assert_eq!(order.id, "12345");
        assert_eq!(order.client_order_id.as_deref(), Some("test123"));
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.amount, Decimal::from_str("0.001").unwrap());
        assert_eq!(order.filled, Some(Decimal::ZERO));
        assert_eq!(order.remaining, Some(Decimal::from_str("0.001").unwrap()));
    }

    #[test]
    fn test_parse_order_spot_filled() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "orderId": 99999,
            "transactTime": 1700000000000_i64,
            "price": "0.00",
            "origQty": "0.010",
            "executedQty": "0.010",
            "cummulativeQuoteQty": "500.00",
            "status": "FILLED",
            "type": "MARKET",
            "side": "BUY",
            "fills": [
                {"commission": "0.00001", "commissionAsset": "BTC"},
                {"commission": "0.000005", "commissionAsset": "BTC"}
            ]
        });

        let order = parse_order(&json, "BTC/USDT", false).unwrap();
        assert_eq!(order.status, OrderStatus::Closed);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.cost, Some(Decimal::from_str("500.00").unwrap()));
        assert_eq!(order.average, Some(Decimal::from_str("50000").unwrap()));
        assert_eq!(order.remaining, Some(Decimal::ZERO));

        let fee = order.fee.unwrap();
        assert_eq!(fee.cost, Decimal::from_str("0.000015").unwrap());
        assert_eq!(fee.currency, "BTC");
    }

    #[test]
    fn test_parse_order_futures() {
        let json = serde_json::json!({
            "orderId": 55555,
            "symbol": "BTCUSDT",
            "status": "NEW",
            "clientOrderId": "fut_order",
            "price": "48000.0",
            "origQty": "0.100",
            "executedQty": "0.000",
            "cumQuote": "0.0",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "SELL",
            "reduceOnly": true,
            "updateTime": 1700000000000_i64
        });

        let order = parse_order(&json, "BTC/USDT:USDT", true).unwrap();
        assert_eq!(order.id, "55555");
        assert_eq!(order.side, OrderSide::Sell);
        assert_eq!(order.reduce_only, Some(true));
        assert_eq!(order.symbol, "BTC/USDT:USDT");
    }

    #[test]
    fn test_parse_balance_spot() {
        let json = serde_json::json!({
            "updateTime": 1700000000000_i64,
            "balances": [
                {"asset": "BTC", "free": "0.5", "locked": "0.1"},
                {"asset": "USDT", "free": "1000.0", "locked": "200.0"},
                {"asset": "ETH", "free": "0.0", "locked": "0.0"}
            ]
        });

        let balances = parse_balance_spot(&json).unwrap();
        assert_eq!(balances.balances.len(), 2); // ETH skipped (zero)

        let btc = balances.balances.get("BTC").unwrap();
        assert_eq!(btc.free, Decimal::from_str("0.5").unwrap());
        assert_eq!(btc.used, Decimal::from_str("0.1").unwrap());
        assert_eq!(btc.total, Decimal::from_str("0.6").unwrap());

        assert_eq!(*balances.free.get("BTC").unwrap(), Decimal::from_str("0.5").unwrap());
        assert_eq!(*balances.total.get("USDT").unwrap(), Decimal::from_str("1200.0").unwrap());
    }

    #[test]
    fn test_parse_position() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "positionAmt": "0.100",
            "entryPrice": "48000.0",
            "markPrice": "50000.0",
            "unRealizedProfit": "200.0",
            "liquidationPrice": "40000.0",
            "leverage": "10",
            "marginType": "isolated",
            "notional": "5000.0",
            "initialMargin": "500.0",
            "maintMargin": "25.0",
            "updateTime": 1700000000000_i64
        });

        let position = parse_position(&json).unwrap();
        assert_eq!(position.symbol, "BTC/USDT:USDT");
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.contracts, Decimal::from_str("0.100").unwrap());
        assert_eq!(position.margin_mode, MarginMode::Isolated);
        assert_eq!(position.entry_price, Some(Decimal::from_str("48000.0").unwrap()));
        assert_eq!(position.leverage, Some(Decimal::from(10)));
    }

    #[test]
    fn test_parse_funding_rate() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "markPrice": "50000.00000000",
            "indexPrice": "49990.00000000",
            "lastFundingRate": "0.00010000",
            "nextFundingTime": 1700028800000_i64,
            "interestRate": "0.00010000",
            "time": 1700000000000_i64
        });

        let fr = parse_funding_rate(&json, "BTC/USDT:USDT").unwrap();
        assert_eq!(fr.funding_rate, Decimal::from_str("0.00010000").unwrap());
        assert_eq!(fr.mark_price, Some(Decimal::from_str("50000.00000000").unwrap()));
        assert_eq!(fr.next_funding_timestamp, Some(1700028800000));
    }

    #[test]
    fn test_parse_my_trade() {
        let json = serde_json::json!({
            "id": 12345,
            "orderId": 99999,
            "symbol": "BTCUSDT",
            "price": "50000.00",
            "qty": "0.001",
            "quoteQty": "50.00",
            "commission": "0.00001",
            "commissionAsset": "BTC",
            "time": 1700000000000_i64,
            "isBuyer": true,
            "isMaker": false
        });

        let trade = parse_my_trade(&json, "BTC/USDT").unwrap();
        assert_eq!(trade.id, "12345");
        assert_eq!(trade.order, Some("99999".to_string()));
        assert_eq!(trade.side, OrderSide::Buy);
        assert_eq!(trade.taker_or_maker, Some("taker".to_string()));
        let fee = trade.fee.unwrap();
        assert_eq!(fee.cost, Decimal::from_str("0.00001").unwrap());
        assert_eq!(fee.currency, "BTC");
    }

    #[test]
    fn test_parse_deposit() {
        let json = serde_json::json!({
            "id": "d12345",
            "txId": "0xabc123",
            "coin": "USDT",
            "network": "ETH",
            "address": "0x1234567890",
            "amount": "100.5",
            "status": 1,
            "insertTime": 1700000000000_i64
        });

        let dep = parse_deposit(&json).unwrap();
        assert_eq!(dep.id, "d12345");
        assert_eq!(dep.currency, "USDT");
        assert_eq!(dep.amount, Decimal::from_str("100.5").unwrap());
        assert_eq!(dep.status, TransactionStatus::Ok);
    }

    #[test]
    fn test_parse_withdrawal() {
        let json = serde_json::json!({
            "id": "w99999",
            "txId": "0xdef456",
            "coin": "ETH",
            "network": "ETH",
            "address": "0xabcdef",
            "amount": "1.5",
            "transactionFee": "0.005",
            "status": 6,
            "applyTime": "2023-11-15 10:00:00"
        });

        let wd = parse_withdrawal(&json).unwrap();
        assert_eq!(wd.id, "w99999");
        assert_eq!(wd.currency, "ETH");
        assert_eq!(wd.amount, Decimal::from_str("1.5").unwrap());
        assert_eq!(wd.status, TransactionStatus::Ok);
        assert!(wd.fee.is_some());
    }

    #[test]
    fn test_parse_futures_market() {
        let json = serde_json::json!({
            "symbol": "BTCUSDT",
            "pair": "BTCUSDT",
            "contractType": "PERPETUAL",
            "baseAsset": "BTC",
            "quoteAsset": "USDT",
            "marginAsset": "USDT",
            "status": "TRADING",
            "filters": [
                {"filterType": "PRICE_FILTER", "tickSize": "0.10", "minPrice": "0.10", "maxPrice": "100000.00"},
                {"filterType": "LOT_SIZE", "stepSize": "0.001", "minQty": "0.001", "maxQty": "1000.0"}
            ]
        });

        let market = parse_futures_market(&json).unwrap();
        assert_eq!(market.symbol, "BTC/USDT:USDT");
        assert_eq!(market.id, "BTCUSDT");
        assert!(market.swap);
        assert!(!market.spot);
        assert_eq!(market.settle, Some("USDT".to_string()));
        assert_eq!(market.contract, Some(true));
        assert_eq!(market.linear, Some(true));
    }
}
