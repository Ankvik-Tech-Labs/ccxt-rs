//! HTTP client with rate limiting for CEX exchanges

use crate::base::errors::{CcxtError, Result};
use crate::base::rate_limiter::RateLimiter;
use reqwest::{Client, Method, RequestBuilder, Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for HTTP request retry with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_retries: u32,
    /// Initial delay before first retry (default: 1s)
    pub initial_delay: Duration,
    /// Maximum delay between retries (default: 30s)
    pub max_delay: Duration,
    /// Multiplier applied to delay after each retry (default: 2.0)
    pub backoff_factor: f64,
    /// Longer delay used for rate-limit (429) errors (default: 5s)
    pub rate_limit_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            rate_limit_delay: Duration::from_secs(5),
        }
    }
}

/// Parse `Retry-After` seconds value from a rate-limit error message.
///
/// Looks for the pattern `retry-after: <digits>s` inside the message string.
fn parse_retry_after(msg: &str) -> Option<u64> {
    let marker = "retry-after: ";
    let start = msg.find(marker)? + marker.len();
    let rest = &msg[start..];
    let end = rest.find('s').unwrap_or(rest.len());
    rest[..end].trim().parse::<u64>().ok()
}

/// Simple jitter in the range [0.0, 1.0) derived from system clock nanoseconds.
fn jitter_factor() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

/// HTTP client wrapper with rate limiting
pub struct HttpClient {
    client: Client,
    rate_limiter: Option<Arc<RateLimiter>>,
    #[allow(dead_code)]
    timeout: Duration,
    retry_config: Option<RetryConfig>,
}

impl HttpClient {
    /// Create a new HTTP client
    ///
    /// # Arguments
    /// * `rate_limiter` - Optional rate limiter
    /// * `timeout` - Request timeout duration
    pub fn new(rate_limiter: Option<Arc<RateLimiter>>, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("ccxt-rs/0.1.0")
            .build()
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;

        Ok(Self {
            client,
            rate_limiter,
            timeout,
            retry_config: None,
        })
    }

    /// Enable automatic retry with the given configuration.
    pub fn with_retry(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Execute GET request
    pub async fn get(&self, url: &str, headers: Option<HashMap<String, String>>) -> Result<Response> {
        self.request_with_retry(Method::GET, url, headers, None).await
    }

    /// Execute POST request
    pub async fn post(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        self.request_with_retry(Method::POST, url, headers, body).await
    }

    /// Execute PUT request
    pub async fn put(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        self.request_with_retry(Method::PUT, url, headers, body).await
    }

    /// Execute DELETE request
    pub async fn delete(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Response> {
        self.request_with_retry(Method::DELETE, url, headers, None).await
    }

    /// Execute HTTP request with optional retry logic.
    ///
    /// If `retry_config` is set, retryable and rate-limit errors are retried
    /// with exponential backoff and jitter. Otherwise falls through to a single
    /// attempt via `request()`.
    async fn request_with_retry(
        &self,
        method: Method,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        let config = match &self.retry_config {
            Some(c) => c.clone(),
            None => return self.request(method, url, headers, body).await,
        };

        let mut delay = config.initial_delay;

        for attempt in 0..=config.max_retries {
            let result = self
                .request(
                    method.clone(),
                    url,
                    headers.clone(),
                    body.clone(),
                )
                .await;

            match result {
                Ok(resp) => return Ok(resp),
                Err(ref e) if attempt < config.max_retries && (e.is_retryable() || e.is_rate_limit()) => {
                    let wait = if e.is_rate_limit() {
                        // Check if the error message contains a Retry-After hint
                        let msg = format!("{}", e);
                        parse_retry_after(&msg)
                            .map(Duration::from_secs)
                            .unwrap_or(config.rate_limit_delay)
                    } else {
                        delay
                    };

                    // Add jitter: wait * (0.5 + 0.5 * jitter)
                    let jitter = 0.5 + 0.5 * jitter_factor();
                    let wait_with_jitter = Duration::from_secs_f64(wait.as_secs_f64() * jitter);

                    tracing::warn!(
                        "HTTP {} {} failed (attempt {}/{}): {} — retrying in {:?}",
                        method,
                        url,
                        attempt + 1,
                        config.max_retries,
                        e,
                        wait_with_jitter,
                    );

                    tokio::time::sleep(wait_with_jitter).await;

                    // Exponential backoff for non-rate-limit errors
                    if !e.is_rate_limit() {
                        let next = delay.as_secs_f64() * config.backoff_factor;
                        delay = Duration::from_secs_f64(next.min(config.max_delay.as_secs_f64()));
                    }
                }
                Err(e) => return Err(e),
            }
        }

        unreachable!("All loop iterations return")
    }

    /// Execute HTTP request with rate limiting
    async fn request(
        &self,
        method: Method,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        // Apply rate limiting
        if let Some(rate_limiter) = &self.rate_limiter {
            rate_limiter.wait().await;
        }

        // Build request
        let mut builder: RequestBuilder = self.client.request(method.clone(), url);

        // Add headers
        if let Some(headers) = headers {
            for (key, value) in headers {
                builder = builder.header(key, value);
            }
        }

        // Add body
        if let Some(body) = body {
            builder = builder.body(body);
        }

        // Send request
        tracing::debug!("HTTP {} {}", method, url);
        let response = builder
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    CcxtError::RequestTimeout
                } else if e.is_connect() || e.is_request() {
                    CcxtError::NetworkError(e.to_string())
                } else {
                    CcxtError::NetworkError(e.to_string())
                }
            })?;

        // Check for rate limit response — capture Retry-After header
        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());

            let msg = match retry_after {
                Some(secs) => format!("Rate limit exceeded (retry-after: {}s)", secs),
                None => "Rate limit exceeded".to_string(),
            };
            return Err(CcxtError::RateLimitExceeded(msg));
        }

        // Check for server errors
        if response.status().is_server_error() {
            return Err(CcxtError::ExchangeNotAvailable(format!(
                "Server error: {}",
                response.status()
            )));
        }

        Ok(response)
    }

    /// Execute JSON request and parse response
    pub async fn request_json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<T> {
        let response = self.request_with_retry(method, url, headers, body).await?;

        // Read response text first for better error messages
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| CcxtError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            return Err(CcxtError::BadRequest(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        serde_json::from_str(&text)
            .map_err(|e| CcxtError::ParseError(format!("Failed to parse JSON: {} - Response: {}", e, text)))
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(None, Duration::from_secs(30)).expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert!((config.backoff_factor - 2.0).abs() < f64::EPSILON);
        assert_eq!(config.rate_limit_delay, Duration::from_secs(5));
    }

    #[test]
    fn test_error_retryability() {
        assert!(CcxtError::NetworkError("timeout".into()).is_retryable());
        assert!(CcxtError::RequestTimeout.is_retryable());
        assert!(CcxtError::ExchangeNotAvailable("down".into()).is_retryable());
        assert!(CcxtError::OnMaintenance("upgrading".into()).is_retryable());
        assert!(CcxtError::DDoSProtection("cf".into()).is_retryable());

        assert!(!CcxtError::AuthenticationError("bad".into()).is_retryable());
        assert!(!CcxtError::InvalidOrder("no".into()).is_retryable());

        assert!(CcxtError::RateLimitExceeded("429".into()).is_rate_limit());
        assert!(!CcxtError::NetworkError("err".into()).is_rate_limit());
    }

    #[test]
    fn test_parse_retry_after() {
        assert_eq!(parse_retry_after("Rate limit exceeded (retry-after: 5s)"), Some(5));
        assert_eq!(parse_retry_after("Rate limit exceeded (retry-after: 30s)"), Some(30));
        assert_eq!(parse_retry_after("Rate limit exceeded"), None);
        assert_eq!(parse_retry_after("retry-after: 10s remaining"), Some(10));
    }

    #[test]
    fn test_jitter_factor() {
        let j = jitter_factor();
        assert!((0.0..1.0).contains(&j), "Jitter {} out of range [0, 1)", j);
    }

    #[test]
    fn test_http_client_with_retry() {
        let client = HttpClient::default().with_retry(RetryConfig::default());
        assert!(client.retry_config.is_some());
        let rc = client.retry_config.unwrap();
        assert_eq!(rc.max_retries, 3);
    }
}
