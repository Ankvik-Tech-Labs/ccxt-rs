//! OKX API response parsers
//!
//! Convert OKX-specific responses to unified CCXT types

use crate::base::errors::{CcxtError, Result};
use crate::base::decimal::parse_decimal;
use crate::types::*;
use rust_decimal::Decimal;
use serde_json::Value;

/// Convert unified symbol (BTC/USDT) to OKX format (BTC-USDT)
pub fn convert_symbol_to_okx(symbol: &str) -> String {
    symbol.replace('/', "-")
}

/// Convert OKX symbol (BTC-USDT) to unified format (BTC/USDT)
pub fn convert_symbol_from_okx(okx_symbol: &str) -> String {
    okx_symbol.replace('-', "/")
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
        _ => "1H", // Default to 1 hour
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

/// Parse OKX market info into unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let _inst_id = json
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

    let state = json
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("live");

    let active = state == "live";

    // Parse lot size (amount precision)
    let lot_sz = json
        .get("lotSz")
        .and_then(|v| v.as_str());

    let min_sz = json
        .get("minSz")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let max_lmt_sz = json
        .get("maxLmtSz")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    // Parse tick size (price precision)
    let tick_sz = json
        .get("tickSz")
        .and_then(|v| v.as_str());

    // Parse min order size (cost)
    let min_order_sz = json
        .get("minOrderSz")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    // Calculate precision
    let amount_precision = lot_sz.map(count_decimals);
    let price_precision = tick_sz.map(count_decimals);

    Ok(Market {
        symbol: symbol.clone(),
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
        precision: MarketPrecision {
            price: price_precision,
            amount: amount_precision,
            cost: None,
            base: amount_precision,
            quote: price_precision,
        },
        limits: MarketLimits {
            amount: Some(MinMax {
                min: min_sz,
                max: max_lmt_sz,
            }),
            price: Some(MinMax {
                min: None,
                max: None,
            }),
            cost: Some(MinMax {
                min: min_order_sz,
                max: None,
            }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

/// Parse OKX ticker into unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let last = json
        .get("last")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let bid = json
        .get("bidPx")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let ask = json
        .get("askPx")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let high = json
        .get("high24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let low = json
        .get("low24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let volume = json
        .get("vol24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let quote_volume = json
        .get("volCcy24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let open = json
        .get("open24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let change = if let (Some(last_price), Some(open_price)) = (last, open) {
        Some(last_price - open_price)
    } else {
        None
    };

    let percentage = if let (Some(last_price), Some(open_price)) = (last, open) {
        if open_price != Decimal::ZERO {
            Some(((last_price - open_price) / open_price) * Decimal::from(100))
        } else {
            None
        }
    } else {
        None
    };

    Ok(Ticker {
        symbol: symbol.to_string(),
        timestamp,
        datetime: chrono::DateTime::from_timestamp(timestamp / 1000, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339(),
        high,
        low,
        bid,
        bid_volume: None,
        ask,
        ask_volume: None,
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
        info: Some(json.clone()),
    })
}

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
        datetime: chrono::DateTime::from_timestamp(timestamp / 1000, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339(),
        nonce: None,
        info: Some(json.clone()),
    })
}

/// Parse OKX OHLCV candle into unified OHLCV
pub fn parse_ohlcv(json: &Value) -> Result<OHLCV> {
    let arr = json
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("OHLCV data is not an array".to_string()))?;

    if arr.len() < 6 {
        return Err(CcxtError::ParseError("OHLCV array too short".to_string()));
    }

    // OKX candle format: [timestamp, open, high, low, close, volume, volCcy, volCcyQuote, confirm]
    let timestamp = arr[0]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid timestamp".to_string()))?
        .parse::<i64>()
        .map_err(|e| CcxtError::ParseError(format!("Failed to parse timestamp: {}", e)))?;

    let open = parse_decimal(
        arr[1]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Invalid open".to_string()))?,
    )?;

    let high = parse_decimal(
        arr[2]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Invalid high".to_string()))?,
    )?;

    let low = parse_decimal(
        arr[3]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Invalid low".to_string()))?,
    )?;

    let close = parse_decimal(
        arr[4]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Invalid close".to_string()))?,
    )?;

    let volume = parse_decimal(
        arr[5]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Invalid volume".to_string()))?,
    )?;

    Ok(OHLCV {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        info: Some(json.clone()),
    })
}

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
            .ok_or_else(|| CcxtError::ParseError("Missing px (price)".to_string()))?,
    )?;

    let amount = parse_decimal(
        json.get("sz")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcxtError::ParseError("Missing sz (size)".to_string()))?,
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
        datetime: chrono::DateTime::from_timestamp(timestamp / 1000, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339(),
        side,
        price,
        amount,
        cost,
        fee: None,
        taker_or_maker: None,
        info: Some(json.clone()),
    })
}

/// Parse OKX system status
pub fn parse_status(json: &Value) -> Result<ExchangeStatus> {
    let state = json
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("scheduled");

    let status = match state {
        "scheduled" => "ok",       // Scheduled maintenance (normal)
        "ongoing" => "maintenance", // Ongoing maintenance
        "pre_open" => "ok",        // Pre-open (normal)
        "completed" => "ok",       // Completed (normal)
        "canceled" => "ok",        // Canceled (normal)
        _ => "unknown",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion() {
        assert_eq!(convert_symbol_to_okx("BTC/USDT"), "BTC-USDT");
        assert_eq!(convert_symbol_to_okx("ETH/USDC"), "ETH-USDC");

        assert_eq!(convert_symbol_from_okx("BTC-USDT"), "BTC/USDT");
        assert_eq!(convert_symbol_from_okx("ETH-USDC"), "ETH/USDC");
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
        assert_eq!(count_decimals("0.0100"), 2); // Trailing zeros trimmed
    }
}
