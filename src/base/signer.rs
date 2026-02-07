//! Authentication and signing utilities for CEX exchanges

use crate::base::errors::{CcxtError, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha512};

type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

/// HMAC-SHA256 signing
pub fn hmac_sha256(secret: &str, message: &str) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| CcxtError::AuthenticationError(format!("Invalid secret key: {}", e)))?;

    mac.update(message.as_bytes());

    let result = mac.finalize();
    Ok(hex::encode(result.into_bytes()))
}

/// HMAC-SHA256 signing with hex output
pub fn hmac_sha256_hex(secret: &str, message: &str) -> Result<String> {
    hmac_sha256(secret, message)
}

/// HMAC-SHA256 signing with base64 output
pub fn hmac_sha256_base64(secret: &str, message: &str) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| CcxtError::AuthenticationError(format!("Invalid secret key: {}", e)))?;

    mac.update(message.as_bytes());

    let result = mac.finalize();
    Ok(BASE64.encode(result.into_bytes()))
}

/// HMAC-SHA512 signing
pub fn hmac_sha512(secret: &str, message: &str) -> Result<String> {
    let mut mac = HmacSha512::new_from_slice(secret.as_bytes())
        .map_err(|e| CcxtError::AuthenticationError(format!("Invalid secret key: {}", e)))?;

    mac.update(message.as_bytes());

    let result = mac.finalize();
    Ok(hex::encode(result.into_bytes()))
}

/// Get current timestamp in milliseconds
pub fn timestamp_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Get current timestamp in seconds
pub fn timestamp_s() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Get ISO 8601 datetime string
pub fn iso8601_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Convert timestamp (milliseconds) to ISO 8601 string
pub fn timestamp_to_iso8601(timestamp_ms: i64) -> String {
    let datetime = chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .unwrap_or_else(chrono::Utc::now);
    datetime.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256() {
        let secret = "test_secret";
        let message = "test_message";

        let signature = hmac_sha256(secret, message).unwrap();

        // Should be consistent
        let signature2 = hmac_sha256(secret, message).unwrap();
        assert_eq!(signature, signature2);

        // Should be hex string (64 chars for SHA256)
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_hmac_sha256_base64() {
        let secret = "test_secret";
        let message = "test_message";

        let signature = hmac_sha256_base64(secret, message).unwrap();

        // Should be valid base64
        assert!(BASE64.decode(&signature).is_ok());
    }

    #[test]
    fn test_timestamps() {
        let ts_ms = timestamp_ms();
        let ts_s = timestamp_s();

        // Milliseconds should be ~1000x seconds
        assert!(ts_ms / 1000 >= ts_s - 1);
        assert!(ts_ms / 1000 <= ts_s + 1);
    }

    #[test]
    fn test_iso8601() {
        let iso = iso8601_now();

        // Should be valid ISO 8601 format
        assert!(iso.contains('T'));
        assert!(iso.contains('Z'));

        // Should be parseable
        assert!(chrono::DateTime::parse_from_rfc3339(&iso).is_ok());
    }

    #[test]
    fn test_timestamp_to_iso8601() {
        let ts = 1704067200000i64; // 2024-01-01 00:00:00 UTC
        let iso = timestamp_to_iso8601(ts);

        assert!(iso.starts_with("2024-01-01"));
    }
}
