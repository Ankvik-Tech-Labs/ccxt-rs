//! Decimal utilities for financial calculations

use crate::base::errors::{CcxtError, Result};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Parse string to Decimal, handling common exchange formats
pub fn parse_decimal(s: &str) -> Result<Decimal> {
    if s.is_empty() {
        return Ok(Decimal::ZERO);
    }

    Decimal::from_str(s).map_err(|e| {
        CcxtError::ParseError(format!("Failed to parse '{}' as decimal: {}", s, e))
    })
}

/// Parse optional string to Option<Decimal>
pub fn parse_decimal_opt(s: Option<&str>) -> Result<Option<Decimal>> {
    match s {
        Some(s) if !s.is_empty() => Ok(Some(parse_decimal(s)?)),
        _ => Ok(None),
    }
}

/// Parse JSON value to Decimal
pub fn json_to_decimal(value: &serde_json::Value) -> Result<Decimal> {
    match value {
        serde_json::Value::String(s) => parse_decimal(s),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                Ok(Decimal::from(u))
            } else if let Some(f) = n.as_f64() {
                Decimal::try_from(f).map_err(|e| {
                    CcxtError::ParseError(format!("Failed to convert {} to decimal: {}", f, e))
                })
            } else {
                Err(CcxtError::ParseError(format!(
                    "Invalid number format: {}",
                    value
                )))
            }
        }
        _ => Err(CcxtError::ParseError(format!(
            "Expected string or number, got: {}",
            value
        ))),
    }
}

/// Parse JSON value to Option<Decimal>
pub fn json_to_decimal_opt(value: &serde_json::Value) -> Result<Option<Decimal>> {
    match value {
        serde_json::Value::Null => Ok(None),
        _ => Ok(Some(json_to_decimal(value)?)),
    }
}

/// Format decimal to string with specified precision
pub fn format_decimal(value: Decimal, precision: Option<u32>) -> String {
    if let Some(p) = precision {
        // Round first, then format
        let rounded = value.round_dp(p);
        rounded.to_string()
    } else {
        value.to_string()
    }
}

/// Round decimal to specified decimal places
pub fn round_decimal(value: Decimal, decimal_places: u32) -> Decimal {
    value.round_dp(decimal_places)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_decimal("123.45").unwrap(), Decimal::from_str("123.45").unwrap());
        assert_eq!(parse_decimal("0").unwrap(), Decimal::ZERO);
        assert_eq!(parse_decimal("").unwrap(), Decimal::ZERO);
        assert!(parse_decimal("invalid").is_err());
    }

    #[test]
    fn test_json_to_decimal() {
        let json_str = serde_json::Value::String("123.45".to_string());
        let json_num = serde_json::json!(123.45);

        assert_eq!(json_to_decimal(&json_str).unwrap(), Decimal::from_str("123.45").unwrap());
        assert!(json_to_decimal(&json_num).is_ok());
    }

    #[test]
    fn test_format_decimal() {
        let value = Decimal::from_str("123.456789").unwrap();

        assert_eq!(format_decimal(value, Some(2)), "123.46");
        assert_eq!(format_decimal(value, Some(4)), "123.4568");
        assert_eq!(format_decimal(value, None), "123.456789");
    }

    #[test]
    fn test_round_decimal() {
        let value = Decimal::from_str("123.456789").unwrap();

        assert_eq!(round_decimal(value, 2), Decimal::from_str("123.46").unwrap());
        assert_eq!(round_decimal(value, 4), Decimal::from_str("123.4568").unwrap());
    }
}
