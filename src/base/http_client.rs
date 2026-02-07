//! HTTP client with rate limiting for CEX exchanges

use crate::base::errors::{CcxtError, Result};
use crate::base::rate_limiter::RateLimiter;
use reqwest::{Client, Method, RequestBuilder, Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// HTTP client wrapper with rate limiting
pub struct HttpClient {
    client: Client,
    rate_limiter: Option<Arc<RateLimiter>>,
    #[allow(dead_code)]
    timeout: Duration,
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
        })
    }

    /// Execute GET request
    pub async fn get(&self, url: &str, headers: Option<HashMap<String, String>>) -> Result<Response> {
        self.request(Method::GET, url, headers, None).await
    }

    /// Execute POST request
    pub async fn post(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        self.request(Method::POST, url, headers, body).await
    }

    /// Execute PUT request
    pub async fn put(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    ) -> Result<Response> {
        self.request(Method::PUT, url, headers, body).await
    }

    /// Execute DELETE request
    pub async fn delete(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Response> {
        self.request(Method::DELETE, url, headers, None).await
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

        // Check for rate limit response
        if response.status().as_u16() == 429 {
            return Err(CcxtError::RateLimitExceeded(
                "Rate limit exceeded".to_string(),
            ));
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
        let response = self.request(method, url, headers, body).await?;

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
