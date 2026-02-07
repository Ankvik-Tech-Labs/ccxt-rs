//! Rate limiting utilities

use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::num::NonZeroU32;

/// Rate limiter wrapper using governor
pub struct RateLimiter {
    limiter: GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum requests per second
    pub fn new(requests_per_second: u32) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(requests_per_second).expect("requests_per_second must be > 0"),
        );
        let limiter = GovernorRateLimiter::direct(quota);

        Self { limiter }
    }

    /// Wait until a request can be made (respecting rate limit)
    pub async fn wait(&self) {
        self.limiter.until_ready().await;
    }

    /// Try to make a request immediately (returns false if rate limited)
    pub fn try_acquire(&self) -> bool {
        self.limiter.check().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2); // 2 requests per second

        let start = Instant::now();

        // First two requests should be immediate
        limiter.wait().await;
        limiter.wait().await;

        // Third request should wait ~500ms
        limiter.wait().await;

        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() >= 450); // Allow some timing variance
    }

    #[test]
    fn test_try_acquire() {
        let limiter = RateLimiter::new(2);

        // First two should succeed
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Third should fail (rate limited)
        assert!(!limiter.try_acquire());
    }
}
