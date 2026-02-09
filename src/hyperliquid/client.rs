//! REST client for Hyperliquid API
//!
//! Hyperliquid has exactly 2 endpoints:
//! - POST /info — all read operations
//! - POST /exchange — all write operations (requires EIP-712 signature)

use crate::base::errors::{CcxtError, Result};
use crate::base::http_client::HttpClient;
use crate::base::rate_limiter::RateLimiter;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// REST client for Hyperliquid's /info and /exchange endpoints
pub struct HyperliquidClient {
    http_client: HttpClient,
    base_url: String,
}

impl HyperliquidClient {
    /// Create a new Hyperliquid REST client.
    ///
    /// :param base_url: API base URL (mainnet or testnet).
    /// :param rate_limiter: Optional rate limiter.
    /// :param timeout: Request timeout.
    pub fn new(
        base_url: &str,
        rate_limiter: Option<Arc<RateLimiter>>,
        timeout: Duration,
    ) -> Result<Self> {
        let http_client = HttpClient::new(rate_limiter, timeout)?;
        Ok(Self {
            http_client,
            base_url: base_url.to_string(),
        })
    }

    /// Send a read request to POST /info.
    ///
    /// :param request_type: The `type` field (e.g., "meta", "allMids", "l2Book").
    /// :param extra_fields: Additional fields to merge into the request body.
    pub async fn info_request(
        &self,
        request_type: &str,
        extra_fields: Option<Value>,
    ) -> Result<Value> {
        let mut body = serde_json::json!({ "type": request_type });

        if let Some(extra) = extra_fields {
            if let (Some(body_obj), Some(extra_obj)) = (body.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    body_obj.insert(k.clone(), v.clone());
                }
            }
        }

        let url = format!("{}/info", self.base_url);
        let body_str = serde_json::to_string(&body)?;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = self
            .http_client
            .post(&url, Some(headers), Some(body_str))
            .await?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(Self::map_http_error(status.as_u16(), &text));
        }

        serde_json::from_str(&text).map_err(|e| {
            CcxtError::ParseError(format!("Failed to parse /info response: {} - Body: {}", e, text))
        })
    }

    /// Send a write request to POST /exchange.
    ///
    /// :param action: The action payload.
    /// :param nonce: Timestamp nonce in milliseconds.
    /// :param signature: EIP-712 signature as JSON object {r, s, v}.
    /// :param vault_address: Optional vault address.
    pub async fn exchange_request(
        &self,
        action: Value,
        nonce: u64,
        signature: Value,
        vault_address: Option<&str>,
    ) -> Result<Value> {
        let body = serde_json::json!({
            "action": action,
            "nonce": nonce,
            "signature": signature,
            "vaultAddress": vault_address,
        });

        let url = format!("{}/exchange", self.base_url);
        let body_str = serde_json::to_string(&body)?;

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = self
            .http_client
            .post(&url, Some(headers), Some(body_str))
            .await?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(Self::map_http_error(status.as_u16(), &text));
        }

        let json: Value = serde_json::from_str(&text).map_err(|e| {
            CcxtError::ParseError(format!(
                "Failed to parse /exchange response: {} - Body: {}",
                e, text
            ))
        })?;

        // Check for Hyperliquid API-level errors: {"status": "err", "response": "..."}
        if json.get("status").and_then(|s| s.as_str()) == Some("err") {
            let msg = json
                .get("response")
                .and_then(|r| r.as_str())
                .unwrap_or("Unknown error");
            return Err(Self::map_exchange_error(msg));
        }

        Ok(json)
    }

    /// Map Hyperliquid error strings to CcxtError.
    fn map_exchange_error(msg: &str) -> CcxtError {
        let lower = msg.to_lowercase();
        if lower.contains("insufficient margin") || lower.contains("insufficient balance") {
            CcxtError::InsufficientFunds(msg.to_string())
        } else if lower.contains("order not found")
            || lower.contains("already canceled")
            || lower.contains("already filled")
            || lower.contains("was never placed")
        {
            CcxtError::OrderNotFound(msg.to_string())
        } else if lower.contains("invalid signature") || lower.contains("unauthorized") {
            CcxtError::AuthenticationError(msg.to_string())
        } else if lower.contains("rate limit") {
            CcxtError::RateLimitExceeded(msg.to_string())
        } else if lower.contains("asset not found") || lower.contains("unknown asset") {
            CcxtError::BadSymbol(msg.to_string())
        } else {
            CcxtError::ExchangeError(msg.to_string())
        }
    }

    /// Map HTTP status codes to CcxtError.
    fn map_http_error(status: u16, body: &str) -> CcxtError {
        match status {
            429 => CcxtError::RateLimitExceeded(format!("HTTP 429: {}", body)),
            401 | 403 => CcxtError::AuthenticationError(format!("HTTP {}: {}", status, body)),
            500..=599 => {
                CcxtError::ExchangeNotAvailable(format!("HTTP {}: {}", status, body))
            }
            _ => CcxtError::BadRequest(format!("HTTP {}: {}", status, body)),
        }
    }
}
