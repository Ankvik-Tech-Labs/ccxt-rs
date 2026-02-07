//! Symbol parsing and data transformation utilities for Uniswap V3
//!
//! This module handles:
//! - Parsing extended symbol format: `BASE/QUOTE:V3:FEE_TIER`
//! - Parsing short symbol format: `BASE/QUOTE`
//! - Converting subgraph data to unified types (Market, Trade, OHLCV)
//! - Safe decimal conversions and timestamp formatting

use crate::base::errors::{CcxtError, Result};
use crate::types::*;
use crate::uniswap::constants::FeeTier;
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

/// Parsed symbol components
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSymbol {
    pub base: String,
    pub quote: String,
    pub fee_tier: Option<FeeTier>,
}

/// Parse Uniswap symbol format
///
/// Supports two formats:
/// - Extended: `WETH/USDC:V3:3000` (specific fee tier)
/// - Short: `WETH/USDC` (defaults to highest TVL pool)
///
/// # Examples
///
/// ```
/// let parsed = parse_uniswap_symbol("WETH/USDC:V3:3000")?;
/// assert_eq!(parsed.base, "WETH");
/// assert_eq!(parsed.quote, "USDC");
/// assert_eq!(parsed.fee_tier, Some(FeeTier::Medium));
/// ```
pub fn parse_uniswap_symbol(symbol: &str) -> Result<ParsedSymbol> {
    let parts: Vec<&str> = symbol.split(':').collect();

    match parts.len() {
        // Short format: BASE/QUOTE
        1 => {
            let pair_parts: Vec<&str> = parts[0].split('/').collect();
            if pair_parts.len() != 2 {
                return Err(CcxtError::BadSymbol(format!(
                    "Invalid symbol format: {}. Expected BASE/QUOTE or BASE/QUOTE:V3:FEE_TIER",
                    symbol
                )));
            }
            Ok(ParsedSymbol {
                base: pair_parts[0].to_string(),
                quote: pair_parts[1].to_string(),
                fee_tier: None,
            })
        }
        // Extended format: BASE/QUOTE:V3:FEE_TIER
        3 => {
            let pair_parts: Vec<&str> = parts[0].split('/').collect();
            if pair_parts.len() != 2 {
                return Err(CcxtError::BadSymbol(format!(
                    "Invalid symbol format: {}. Expected BASE/QUOTE:V3:FEE_TIER",
                    symbol
                )));
            }

            if parts[1] != "V3" {
                return Err(CcxtError::BadSymbol(format!(
                    "Invalid protocol version: {}. Only V3 is supported",
                    parts[1]
                )));
            }

            let fee_tier_value = parts[2].parse::<u32>().map_err(|_| {
                CcxtError::BadSymbol(format!("Invalid fee tier: {}", parts[2]))
            })?;

            let fee_tier = FeeTier::from_basis_points(fee_tier_value)?;

            Ok(ParsedSymbol {
                base: pair_parts[0].to_string(),
                quote: pair_parts[1].to_string(),
                fee_tier: Some(fee_tier),
            })
        }
        _ => Err(CcxtError::BadSymbol(format!(
            "Invalid symbol format: {}. Expected BASE/QUOTE or BASE/QUOTE:V3:FEE_TIER",
            symbol
        ))),
    }
}

/// Format symbol from components
///
/// # Examples
///
/// ```
/// let symbol = format_uniswap_symbol("WETH", "USDC", Some(FeeTier::Medium));
/// assert_eq!(symbol, "WETH/USDC:V3:3000");
/// ```
pub fn format_uniswap_symbol(base: &str, quote: &str, fee_tier: Option<FeeTier>) -> String {
    match fee_tier {
        Some(tier) => format!("{}/{}:V3:{}", base, quote, tier.as_basis_points()),
        None => format!("{}/{}", base, quote),
    }
}

/// Convert JSON value to Decimal safely
pub fn json_to_decimal(value: &Value, field_name: &str) -> Result<Decimal> {
    match value {
        Value::String(s) => Decimal::from_str(s).map_err(|_| {
            CcxtError::ParseError(format!("Invalid decimal in {}: {}", field_name, s))
        }),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Decimal::try_from(f).map_err(|_| {
                    CcxtError::ParseError(format!("Invalid decimal in {}: {}", field_name, f))
                })
            } else {
                Err(CcxtError::ParseError(format!(
                    "Invalid number in {}: {}",
                    field_name, n
                )))
            }
        }
        _ => Err(CcxtError::ParseError(format!(
            "Expected string or number for {}, got: {:?}",
            field_name, value
        ))),
    }
}

/// Convert Unix timestamp to ISO8601 string
pub fn timestamp_to_iso8601(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_default()
        .to_rfc3339()
}

/// Convert subgraph pool data to Market
pub fn parse_pool_to_market(
    pool: &Value,
    base_symbol: &str,
    quote_symbol: &str,
    fee_tier: FeeTier,
) -> Result<Market> {
    let pool_id = pool["id"]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Missing pool id".to_string()))?;

    let token0 = &pool["token0"];
    let token1 = &pool["token1"];

    let token0_symbol = token0["symbol"]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Missing token0 symbol".to_string()))?;
    let token1_symbol = token1["symbol"]
        .as_str()
        .ok_or_else(|| CcxtError::ParseError("Missing token1 symbol".to_string()))?;

    let token0_decimals = token0["decimals"]
        .as_str()
        .and_then(|s| s.parse::<u8>().ok())
        .ok_or_else(|| CcxtError::ParseError("Missing token0 decimals".to_string()))?;

    let token1_decimals = token1["decimals"]
        .as_str()
        .and_then(|s| s.parse::<u8>().ok())
        .ok_or_else(|| CcxtError::ParseError("Missing token1 decimals".to_string()))?;

    // Determine if token ordering matches base/quote
    let (base_token, quote_token, inverted) = if token0_symbol == base_symbol
        && token1_symbol == quote_symbol
    {
        (token0, token1, false)
    } else if token0_symbol == quote_symbol && token1_symbol == base_symbol {
        (token1, token0, true)
    } else {
        return Err(CcxtError::ParseError(format!(
            "Token mismatch: expected {}/{}, got {}/{}",
            base_symbol, quote_symbol, token0_symbol, token1_symbol
        )));
    };

    let symbol = format_uniswap_symbol(base_symbol, quote_symbol, Some(fee_tier));

    let fee_percentage = fee_tier.as_percentage();

    Ok(Market {
        symbol: symbol.clone(),
        base: base_symbol.to_string(),
        quote: quote_symbol.to_string(),
        settle: None,
        base_id: base_token["id"].as_str().unwrap_or("").to_string(),
        quote_id: quote_token["id"].as_str().unwrap_or("").to_string(),
        settle_id: None,
        market_type: "spot".to_string(),
        spot: true,
        margin: false,
        swap: false,
        future: false,
        option: false,
        active: true,
        contract: None,
        linear: None,
        inverse: None,
        taker: Some(fee_percentage),
        maker: Some(fee_percentage),
        contract_size: None,
        expiry: None,
        expiry_datetime: None,
        strike: None,
        option_type: None,
        precision: MarketPrecision {
            price: Some(if inverted { token0_decimals as i32 } else { token1_decimals as i32 }),
            amount: Some(if inverted { token1_decimals as i32 } else { token0_decimals as i32 }),
            cost: None,
            base: Some(if inverted { token1_decimals as i32 } else { token0_decimals as i32 }),
            quote: Some(if inverted { token0_decimals as i32 } else { token1_decimals as i32 }),
        },
        limits: MarketLimits {
            amount: Some(MinMax {
                min: None,
                max: None,
            }),
            price: Some(MinMax {
                min: None,
                max: None,
            }),
            cost: Some(MinMax {
                min: None,
                max: None,
            }),
            leverage: Some(MinMax {
                min: None,
                max: None,
            }),
        },
        info: Some(pool.clone()),
    })
}

/// Convert subgraph swaps to Trade vector
pub fn parse_swaps_to_trades(
    swaps: &Value,
    symbol: &str,
    _base_symbol: &str,
    quote_symbol: &str,
    _base_decimals: u8,
    _quote_decimals: u8,
    fee_tier: FeeTier,
    inverted: bool,
) -> Result<Vec<Trade>> {
    let swap_array = swaps
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("Swaps is not an array".to_string()))?;

    let mut trades = Vec::new();

    for swap in swap_array {
        let id = swap["id"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing swap id".to_string()))?;

        let timestamp = swap["timestamp"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| CcxtError::ParseError("Invalid timestamp".to_string()))?;

        let amount0_str = swap["amount0"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing amount0".to_string()))?;
        let amount1_str = swap["amount1"]
            .as_str()
            .ok_or_else(|| CcxtError::ParseError("Missing amount1".to_string()))?;

        let amount0 = Decimal::from_str(amount0_str).map_err(|_| {
            CcxtError::ParseError(format!("Invalid amount0: {}", amount0_str))
        })?;
        let amount1 = Decimal::from_str(amount1_str).map_err(|_| {
            CcxtError::ParseError(format!("Invalid amount1: {}", amount1_str))
        })?;

        // Determine side: if amount0 > 0, buying token0 (selling token1)
        let (side, amount, price) = if !inverted {
            if amount0 > Decimal::ZERO {
                // Buying base (token0), selling quote (token1)
                let price = amount1.abs() / amount0.abs();
                (OrderSide::Buy, amount0, price)
            } else {
                // Selling base (token0), buying quote (token1)
                let price = amount1.abs() / amount0.abs();
                (OrderSide::Sell, amount0.abs(), price)
            }
        } else {
            // Inverted: token1 is base, token0 is quote
            if amount1 > Decimal::ZERO {
                let price = amount0.abs() / amount1.abs();
                (OrderSide::Buy, amount1, price)
            } else {
                let price = amount0.abs() / amount1.abs();
                (OrderSide::Sell, amount1.abs(), price)
            }
        };

        let cost = amount * price;
        let fee_amount = cost * fee_tier.as_percentage();

        trades.push(Trade {
            id: id.to_string(),
            order: None,
            timestamp: timestamp * 1000, // Convert to milliseconds
            datetime: timestamp_to_iso8601(timestamp),
            symbol: symbol.to_string(),
            side,
            taker_or_maker: None,
            price,
            amount,
            cost,
            fee: Some(TradeFee {
                cost: fee_amount,
                currency: quote_symbol.to_string(),
                rate: Some(fee_tier.as_percentage()),
            }),
            info: Some(swap.clone()),
        });
    }

    Ok(trades)
}

/// Convert subgraph candle data to OHLCV vector
pub fn parse_candles_to_ohlcv(candles: &Value, _symbol: &str) -> Result<Vec<OHLCV>> {
    let candle_array = candles
        .as_array()
        .ok_or_else(|| CcxtError::ParseError("Candles is not an array".to_string()))?;

    let mut ohlcv_data = Vec::new();

    for candle in candle_array {
        let timestamp = candle["periodStartUnix"]
            .as_str()
            .or_else(|| candle["date"].as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| CcxtError::ParseError("Invalid timestamp".to_string()))?;

        let open = json_to_decimal(&candle["open"], "open")?;
        let high = json_to_decimal(&candle["high"], "high")?;
        let low = json_to_decimal(&candle["low"], "low")?;
        let close = json_to_decimal(&candle["close"], "close")?;

        let volume = if let Some(v) = candle.get("volumeUSD") {
            json_to_decimal(v, "volumeUSD").unwrap_or(Decimal::ZERO)
        } else {
            Decimal::ZERO
        };

        ohlcv_data.push(OHLCV {
            timestamp: timestamp * 1000, // Convert to milliseconds
            open,
            high,
            low,
            close,
            volume,
            info: None,
        });
    }

    Ok(ohlcv_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extended_format() {
        let parsed = parse_uniswap_symbol("WETH/USDC:V3:3000").unwrap();
        assert_eq!(parsed.base, "WETH");
        assert_eq!(parsed.quote, "USDC");
        assert_eq!(parsed.fee_tier, Some(FeeTier::Medium));
    }

    #[test]
    fn test_parse_short_format() {
        let parsed = parse_uniswap_symbol("WETH/USDC").unwrap();
        assert_eq!(parsed.base, "WETH");
        assert_eq!(parsed.quote, "USDC");
        assert_eq!(parsed.fee_tier, None);
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(parse_uniswap_symbol("WETH-USDC").is_err());
        assert!(parse_uniswap_symbol("WETH/USDC:V2").is_err());
        assert!(parse_uniswap_symbol("WETH/USDC:V3:999").is_err());
    }

    #[test]
    fn test_format_symbol() {
        let symbol = format_uniswap_symbol("WETH", "USDC", Some(FeeTier::Medium));
        assert_eq!(symbol, "WETH/USDC:V3:3000");

        let symbol = format_uniswap_symbol("WETH", "USDC", None);
        assert_eq!(symbol, "WETH/USDC");
    }

    #[test]
    fn test_json_to_decimal() {
        let value = serde_json::json!("123.456");
        let decimal = json_to_decimal(&value, "test").unwrap();
        assert_eq!(decimal, Decimal::from_str("123.456").unwrap());

        let value = serde_json::json!(123.456);
        let decimal = json_to_decimal(&value, "test").unwrap();
        assert!(decimal > Decimal::ZERO);
    }

    #[test]
    fn test_timestamp_to_iso8601() {
        let iso = timestamp_to_iso8601(1609459200); // 2021-01-01 00:00:00 UTC
        assert!(iso.starts_with("2021-01-01"));
    }
}
