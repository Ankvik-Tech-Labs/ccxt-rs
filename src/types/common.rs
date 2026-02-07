//! Common types and enums used across all data structures

use serde::{Deserialize, Serialize};

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLossLimit,
    TakeProfit,
    TakeProfitLimit,
    TrailingStop,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Open,
    Closed,
    Canceled,
    Expired,
    Rejected,
    PartiallyFilled,
}

/// Order time in force
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Canceled
    #[serde(rename = "GTC")]
    Gtc,
    /// Immediate Or Cancel
    #[serde(rename = "IOC")]
    Ioc,
    /// Fill Or Kill
    #[serde(rename = "FOK")]
    Fok,
    /// Post Only
    #[serde(rename = "PO")]
    PostOnly,
}

/// Margin mode for derivatives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    Cross,
    Isolated,
}

/// Position side for derivatives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionSide {
    Long,
    Short,
    Both,
}

/// Timeframe for OHLCV data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "3m")]
    ThreeMinutes,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "30m")]
    ThirtyMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "2h")]
    TwoHours,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "8h")]
    EightHours,
    #[serde(rename = "12h")]
    TwelveHours,
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "3d")]
    ThreeDays,
    #[serde(rename = "1w")]
    OneWeek,
    #[serde(rename = "1M")]
    OneMonth,
}

impl Timeframe {
    /// Convert timeframe to milliseconds
    pub fn to_milliseconds(&self) -> i64 {
        match self {
            Timeframe::OneMinute => 60_000,
            Timeframe::ThreeMinutes => 180_000,
            Timeframe::FiveMinutes => 300_000,
            Timeframe::FifteenMinutes => 900_000,
            Timeframe::ThirtyMinutes => 1_800_000,
            Timeframe::OneHour => 3_600_000,
            Timeframe::TwoHours => 7_200_000,
            Timeframe::FourHours => 14_400_000,
            Timeframe::SixHours => 21_600_000,
            Timeframe::EightHours => 28_800_000,
            Timeframe::TwelveHours => 43_200_000,
            Timeframe::OneDay => 86_400_000,
            Timeframe::ThreeDays => 259_200_000,
            Timeframe::OneWeek => 604_800_000,
            Timeframe::OneMonth => 2_592_000_000, // Approximate (30 days)
        }
    }
}

/// Exchange status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeStatus {
    pub status: String,
    pub updated: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Ok,
    Failed,
    Canceled,
}
