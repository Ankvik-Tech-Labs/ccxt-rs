// Static response tests - Tier 1
// Feed raw exchange JSON through Rust parsers and validate output

// Helper to extract inner data from exchange API envelopes
pub fn bybit_extract_ticker(http_response: &serde_json::Value) -> &serde_json::Value {
    &http_response["result"]["list"][0]
}

pub fn bybit_extract_list(http_response: &serde_json::Value) -> &serde_json::Value {
    &http_response["result"]["list"]
}

pub fn okx_extract_first(http_response: &serde_json::Value) -> &serde_json::Value {
    &http_response["data"][0]
}

pub fn okx_extract_list(http_response: &serde_json::Value) -> &serde_json::Value {
    &http_response["data"]
}
