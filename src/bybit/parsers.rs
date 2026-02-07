//! Bybit API response parsers
//!
//! Convert Bybit-specific responses to unified CCXT types

use crate::base::errors::{CcxtError, Result};
use crate::base::decimal::parse_decimal;
use crate::types::*;
use rust_decimal::Decimal;
use serde_json::Value;

/// Convert unified symbol (BTC/USDT) to Bybit format (BTCUSDT)
pub fn convert_symbol_to_bybit(symbol: &str) -> String {
    symbol.replace('/', "")
}

/// Convert Bybit symbol (BTCUSDT) to unified format (BTC/USDT)
pub fn convert_symbol_from_bybit(bybit_symbol: &str) -> String {
    // Bybit spot symbols typically end with USDT, USDC, or BTC
    if let Some(base) = bybit_symbol.strip_suffix("USDT") {
        format!("{}/USDT", base)
    } else if let Some(base) = bybit_symbol.strip_suffix("USDC") {
        format!("{}/USDC", base)
    } else if let Some(base) = bybit_symbol.strip_suffix("BTC") {
        format!("{}/BTC", base)
    } else {
        // Fallback: just return as-is
        bybit_symbol.to_string()
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
        _ => "60", // Default to 1 hour
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

/// Parse Bybit market info into unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let _symbol_str = json
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

    let status = json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("Trading");

    let active = status == "Trading";

    // Parse lot size filter for precision
    let lot_size_filter = json
        .get("lotSizeFilter")
        .ok_or_else(|| CcxtError::ParseError("Missing lotSizeFilter".to_string()))?;

    let min_order_qty = lot_size_filter
        .get("minOrderQty")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let max_order_qty = lot_size_filter
        .get("maxOrderQty")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    // Parse price filter
    let price_filter = json
        .get("priceFilter")
        .ok_or_else(|| CcxtError::ParseError("Missing priceFilter".to_string()))?;

    let min_price = price_filter
        .get("minPrice")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let max_price = price_filter
        .get("maxPrice")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let tick_size_str = price_filter
        .get("tickSize")
        .and_then(|v| v.as_str());

    let step_size_str = lot_size_filter
        .get("basePrecision")
        .and_then(|v| v.as_str());

    // Calculate precision from tick size and step size
    let price_precision = tick_size_str.map(count_decimals);
    let amount_precision = step_size_str.map(count_decimals);

    Ok(Market {
        symbol: symbol.clone(),
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

/// Parse Bybit ticker into unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = chrono::Utc::now().timestamp_millis();

    let last = json
        .get("lastPrice")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let bid = json
        .get("bid1Price")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let ask = json
        .get("ask1Price")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let high = json
        .get("highPrice24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let low = json
        .get("lowPrice24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let volume = json
        .get("volume24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let quote_volume = json
        .get("turnover24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let prev_price = json
        .get("prevPrice24h")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok());

    let change = if let (Some(last_price), Some(prev)) = (last, prev_price) {
        Some(last_price - prev)
    } else {
        None
    };

    let percentage = json
        .get("price24hPcnt")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_decimal(s).ok())
        .map(|p| p * Decimal::from(100)); // Convert to percentage

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
        open: prev_price,
        close: last,
        last,
        previous_close: prev_price,
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

/// Parse Bybit order book into unified OrderBook
pub fn parse_orderbook(json: &Value, symbol: &str) -> Result<OrderBook> {
    let timestamp = json
        .get("ts")
        .and_then(|v| v.as_i64())
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
        datetime: chrono::DateTime::from_timestamp(timestamp / 1000, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339(),
        nonce: None,
        info: Some(json.clone()),
    })
}

/// Parse Bybit OHLCV candle into unified OHLCV
pub fn parse_ohlcv(json: &Value) -> Result<OHLCV> {
    let arr = json
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("OHLCV data is not an array".to_string()))?;

    if arr.len() < 6 {
        return Err(CcxtError::ParseError("OHLCV array too short".to_string()));
    }

    // Bybit kline format: [timestamp, open, high, low, close, volume, turnover]
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

/// Parse Bybit trade into unified Trade
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_conversion() {
        assert_eq!(convert_symbol_to_bybit("BTC/USDT"), "BTCUSDT");
        assert_eq!(convert_symbol_to_bybit("ETH/USDC"), "ETHUSDC");

        assert_eq!(convert_symbol_from_bybit("BTCUSDT"), "BTC/USDT");
        assert_eq!(convert_symbol_from_bybit("ETHUSDC"), "ETH/USDC");
        assert_eq!(convert_symbol_from_bybit("ETHBTC"), "ETH/BTC");
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
}
