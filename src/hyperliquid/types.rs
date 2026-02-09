//! Hyperliquid-specific request and response types

use serde::{Deserialize, Serialize};

// ============================================================================
// /info response types
// ============================================================================

/// Meta response from /info type="meta"
#[derive(Debug, Clone, Deserialize)]
pub struct HlMeta {
    pub universe: Vec<HlAssetInfo>,
}

/// Single asset info from meta.universe
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlAssetInfo {
    pub name: String,
    pub sz_decimals: u32,
    pub max_leverage: u32,
}

/// Asset context from metaAndAssetCtxs
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlAssetCtx {
    pub day_ntl_vlm: String,
    pub funding: String,
    #[serde(default)]
    pub impact_pxs: Vec<String>,
    pub mark_px: String,
    #[serde(default)]
    pub mid_px: Option<String>,
    pub open_interest: String,
    pub oracle_px: String,
    pub premium: String,
    pub prev_day_px: String,
}

/// L2 book level
#[derive(Debug, Clone, Deserialize)]
pub struct HlLevel {
    pub px: String,
    pub sz: String,
    pub n: u64,
}

/// L2 book response
#[derive(Debug, Clone, Deserialize)]
pub struct HlL2Book {
    pub coin: String,
    pub levels: Vec<Vec<HlLevel>>,
    pub time: u64,
}

/// Recent trade
#[derive(Debug, Clone, Deserialize)]
pub struct HlRecentTrade {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: u64,
    pub tid: u64,
    #[serde(default)]
    pub hash: Option<String>,
}

/// Candle snapshot entry
#[derive(Debug, Clone, Deserialize)]
pub struct HlCandle {
    pub t: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    pub s: String,
    pub i: String,
    pub o: String,
    pub c: String,
    pub h: String,
    pub l: String,
    pub v: String,
    pub n: u64,
}

/// Clearinghouse state
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlClearinghouseState {
    pub asset_positions: Vec<HlAssetPositionWrapper>,
    pub margin_summary: HlMarginSummary,
    #[serde(default)]
    pub cross_margin_summary: Option<HlMarginSummary>,
    pub withdrawable: String,
    #[serde(default)]
    pub time: Option<u64>,
}

/// Wrapper for asset position
#[derive(Debug, Clone, Deserialize)]
pub struct HlAssetPositionWrapper {
    pub position: HlAssetPosition,
    #[serde(rename = "type")]
    pub position_type: String,
}

/// Asset position data
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlAssetPosition {
    pub coin: String,
    pub entry_px: Option<String>,
    pub leverage: HlLeverage,
    #[serde(default)]
    pub liquidation_px: Option<String>,
    pub margin_used: String,
    pub max_leverage: u32,
    pub position_value: String,
    pub return_on_equity: String,
    pub szi: String,
    pub unrealized_pnl: String,
    #[serde(default)]
    pub cum_funding: Option<HlCumFunding>,
}

/// Leverage info
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlLeverage {
    #[serde(default)]
    pub raw_usd: Option<String>,
    #[serde(rename = "type")]
    pub leverage_type: String,
    pub value: u32,
}

/// Cumulative funding
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlCumFunding {
    pub all_time: String,
    pub since_change: String,
    pub since_open: String,
}

/// Margin summary
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlMarginSummary {
    pub account_value: String,
    pub total_margin_used: String,
    pub total_ntl_pos: String,
    pub total_raw_usd: String,
}

/// Open order (simple)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlOpenOrder {
    pub coin: String,
    pub limit_px: String,
    pub oid: u64,
    pub side: String,
    pub sz: String,
    pub timestamp: u64,
}

/// Frontend open order (rich)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlFrontendOpenOrder {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(default)]
    pub orig_sz: Option<String>,
    #[serde(default)]
    pub cloid: Option<String>,
    #[serde(default)]
    pub order_type: Option<String>,
    #[serde(default)]
    pub tif: Option<String>,
    #[serde(default)]
    pub reduce_only: Option<bool>,
}

/// Order status response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlOrderStatusResponse {
    pub order: HlOrderStatusOrder,
    pub status: String,
    pub status_timestamp: u64,
}

/// Order data inside order status
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlOrderStatusOrder {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(default)]
    pub orig_sz: Option<String>,
    #[serde(default)]
    pub order_type: Option<String>,
    #[serde(default)]
    pub tif: Option<String>,
    #[serde(default)]
    pub reduce_only: Option<bool>,
    #[serde(default)]
    pub cloid: Option<String>,
}

/// User fill
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlUserFill {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: u64,
    pub oid: u64,
    pub tid: u64,
    pub fee: String,
    pub fee_token: String,
    #[serde(default)]
    pub start_position: Option<String>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub closed_pnl: Option<String>,
    #[serde(default)]
    pub crossed: Option<bool>,
    #[serde(default)]
    pub hash: Option<String>,
}

/// Funding history entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlFundingEntry {
    pub coin: String,
    pub funding_rate: String,
    pub premium: String,
    pub time: u64,
}

// ============================================================================
// /exchange action types (for building the action JSON)
// ============================================================================

/// Order wire format for Hyperliquid
#[derive(Debug, Clone, Serialize)]
pub struct HlOrderWire {
    pub a: u32,
    pub b: bool,
    pub p: String,
    pub s: String,
    pub r: bool,
    pub t: HlOrderTypeWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
}

/// Order type wire format
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HlOrderTypeWire {
    Limit { limit: HlLimitOrder },
    Trigger { trigger: HlTriggerOrder },
}

/// Limit order parameters
#[derive(Debug, Clone, Serialize)]
pub struct HlLimitOrder {
    pub tif: String,
}

/// Trigger order parameters
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HlTriggerOrder {
    pub is_market: bool,
    pub trigger_px: String,
    pub tpsl: String,
}

/// Cancel wire format
#[derive(Debug, Clone, Serialize)]
pub struct HlCancelWire {
    pub a: u32,
    pub o: u64,
}

// ============================================================================
// /exchange response types
// ============================================================================

/// Top-level exchange response
#[derive(Debug, Clone, Deserialize)]
pub struct HlExchangeResponse {
    pub status: String,
    pub response: Option<HlExchangeResponseData>,
}

/// Exchange response data (polymorphic)
#[derive(Debug, Clone, Deserialize)]
pub struct HlExchangeResponseData {
    #[serde(rename = "type")]
    pub response_type: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Status entry from order placement
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HlOrderStatusEntry {
    Resting { resting: HlOrderResting },
    Filled { filled: HlOrderFilled },
    Error { error: String },
}

/// Resting order result
#[derive(Debug, Clone, Deserialize)]
pub struct HlOrderResting {
    pub oid: u64,
}

/// Filled order result
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HlOrderFilled {
    pub total_sz: String,
    pub avg_px: String,
    pub oid: u64,
}
