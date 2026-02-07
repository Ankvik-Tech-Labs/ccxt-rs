//! OHLCV (candlestick) data structure

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// OHLCV - Open, High, Low, Close, Volume candlestick data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OHLCV {
    /// Timestamp (opening time) in milliseconds
    pub timestamp: i64,

    /// Opening price
    pub open: Decimal,

    /// Highest price during period
    pub high: Decimal,

    /// Lowest price during period
    pub low: Decimal,

    /// Closing price
    pub close: Decimal,

    /// Volume in base currency
    pub volume: Decimal,

    /// Raw exchange response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

impl OHLCV {
    /// Create from array [timestamp, open, high, low, close, volume]
    pub fn from_array(arr: &[serde_json::Value]) -> Result<Self, crate::base::errors::CcxtError> {
        if arr.len() < 6 {
            return Err(crate::base::errors::CcxtError::ParseError(
                "OHLCV array must have at least 6 elements".to_string(),
            ));
        }

        Ok(Self {
            timestamp: arr[0]
                .as_i64()
                .ok_or_else(|| crate::base::errors::CcxtError::ParseError("Invalid timestamp".to_string()))?,
            open: serde_json::from_value(arr[1].clone())?,
            high: serde_json::from_value(arr[2].clone())?,
            low: serde_json::from_value(arr[3].clone())?,
            close: serde_json::from_value(arr[4].clone())?,
            volume: serde_json::from_value(arr[5].clone())?,
            info: None,
        })
    }
}
