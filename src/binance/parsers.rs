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

/// Extract an optional decimal field from a JSON object by key
fn optional_decimal(json: &Value, key: &str) -> Result<Option<Decimal>> {
    json.get(key)
        .and_then(|v| v.as_str())
        .map(parse_decimal)
        .transpose()
}

/// Convert unified symbol to Binance format
///
/// # Example
/// ```
/// use ccxt::binance::parsers::symbol_to_binance;
/// assert_eq!(symbol_to_binance("BTC/USDT"), "BTCUSDT");
/// assert_eq!(symbol_to_binance("ETH/BTC"), "ETHBTC");
/// ```
pub fn symbol_to_binance(symbol: &str) -> String {
    symbol.replace('/', "")
}

/// Convert Binance symbol to unified format
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
    // Common quote currencies in order of priority
    let quote_currencies = ["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB", "EUR", "GBP", "TRY", "DAI"];

    for quote in &quote_currencies {
        if binance_symbol.ends_with(quote) {
            // Calculate byte length correctly
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

    // Fallback: return as-is (no conversion)
    // Don't try to split non-standard symbols
    binance_symbol.to_string()
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

/// Parse Binance ticker to unified Ticker
pub fn parse_ticker(json: &Value, symbol: &str) -> Result<Ticker> {
    let timestamp = json
        .get("closeTime")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CcxtError::ParseError("Missing closeTime in ticker".to_string()))?;

    let high = optional_decimal(json, "highPrice")?;
    let low = optional_decimal(json, "lowPrice")?;

    // Calculate average: (high + low) / 2
    let average = match (high, low) {
        (Some(h), Some(l)) => Some((h + l) / rust_decimal::Decimal::from(2)),
        _ => None,
    };

    // Parse lastPrice once and reuse for both close and last
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

/// Parse Binance trade to unified Trade
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

    // If buyer is maker, then it's a sell order being matched (maker = seller)
    // If buyer is taker, then it's a buy order (taker = buyer)
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

    let open = arr[1]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid open in kline".to_string()))?;

    let high = arr[2]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid high in kline".to_string()))?;

    let low = arr[3]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid low in kline".to_string()))?;

    let close = arr[4]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid close in kline".to_string()))?;

    let volume = arr[5]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Invalid volume in kline".to_string()))?;

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

/// Parse Binance exchangeInfo symbol to unified Market
pub fn parse_market(json: &Value) -> Result<Market> {
    let _symbol_str = json
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

    let is_spot_trading_allowed = json.get("isSpotTradingAllowed").and_then(|v| v.as_bool()).unwrap_or(false);
    let is_margin_trading_allowed = json.get("isMarginTradingAllowed").and_then(|v| v.as_bool()).unwrap_or(false);

    // Parse filters
    let filters = json.get("filters").and_then(|v| v.as_array()).ok_or_else(|| CcxtError::ParseError("Missing filters".to_string()))?;

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
                    min_price = filter.get("minPrice").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
                    max_price = filter.get("maxPrice").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
                }
            }
            Some("LOT_SIZE") => {
                if let Some(step_size) = filter.get("stepSize").and_then(|v| v.as_str()) {
                    amount_precision = Some(count_decimals(step_size));
                    min_amount = filter.get("minQty").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
                    max_amount = filter.get("maxQty").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
                }
            }
            Some("NOTIONAL") | Some("MIN_NOTIONAL") => {
                min_cost = filter.get("minNotional").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
                max_cost = filter.get("maxNotional").and_then(|v| v.as_str()).map(|s| parse_decimal(s)).transpose()?;
            }
            _ => {}
        }
    }

    Ok(Market {
        symbol,
        base: base.to_string(),
        quote: quote.to_string(),
        settle: None,
        base_id: base.to_string(),
        quote_id: quote.to_string(),
        settle_id: None,
        market_type: "spot".to_string(),
        spot: is_spot_trading_allowed,
        margin: is_margin_trading_allowed,
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
            amount: Some(MinMax {
                min: min_amount,
                max: max_amount,
            }),
            price: Some(MinMax {
                min: min_price,
                max: max_price,
            }),
            cost: Some(MinMax {
                min: min_cost,
                max: max_cost,
            }),
            leverage: None,
        },
        info: Some(json.clone()),
    })
}

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
    fn test_count_decimals() {
        assert_eq!(count_decimals("0.01"), 2);
        assert_eq!(count_decimals("0.00001"), 5);
        assert_eq!(count_decimals("1"), 0);
        assert_eq!(count_decimals("0.10"), 1);
    }
}
