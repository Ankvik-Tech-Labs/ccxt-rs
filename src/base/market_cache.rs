//! Market cache with TTL expiration
//!
//! Provides thread-safe caching of market data to reduce API calls.
//! Markets are cached per exchange with configurable TTL (time-to-live).

use crate::types::Market;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Thread-safe market cache with TTL-based expiration
#[derive(Debug, Clone)]
pub struct MarketCache {
    /// Cached markets per exchange: exchange_id -> (markets, timestamp)
    cache: HashMap<String, (Vec<Market>, Instant)>,
    /// Time-to-live for cached entries
    ttl: Duration,
}

impl MarketCache {
    /// Create a new market cache with the specified TTL
    ///
    /// # Arguments
    ///
    /// * `ttl` - Time-to-live for cached entries. Use `Duration::ZERO` to disable caching.
    ///
    /// # Example
    ///
    /// ```
    /// use ccxt::base::market_cache::MarketCache;
    /// use std::time::Duration;
    ///
    /// let cache = MarketCache::new(Duration::from_secs(3600)); // 1 hour TTL
    /// ```
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
        }
    }

    /// Get cached markets if not expired
    ///
    /// Returns `Some(markets)` if the cache entry exists and hasn't expired,
    /// `None` otherwise.
    ///
    /// # Arguments
    ///
    /// * `exchange_id` - The exchange identifier
    pub fn get(&self, exchange_id: &str) -> Option<Vec<Market>> {
        // If TTL is zero, caching is disabled
        if self.ttl.is_zero() {
            return None;
        }

        self.cache.get(exchange_id).and_then(|(markets, timestamp)| {
            let elapsed = timestamp.elapsed();
            if elapsed < self.ttl {
                Some(markets.clone())
            } else {
                None
            }
        })
    }

    /// Insert markets with current timestamp
    ///
    /// # Arguments
    ///
    /// * `exchange_id` - The exchange identifier
    /// * `markets` - The market data to cache
    pub fn insert(&mut self, exchange_id: String, markets: Vec<Market>) {
        // Don't cache if TTL is zero
        if self.ttl.is_zero() {
            return;
        }

        self.cache.insert(exchange_id, (markets, Instant::now()));
    }

    /// Remove expired entries from the cache
    ///
    /// This method should be called periodically to clean up expired entries
    /// and free memory. Returns the number of expired entries removed.
    pub fn clear_expired(&mut self) -> usize {
        let ttl = self.ttl;
        let before_len = self.cache.len();

        self.cache.retain(|_, (_, timestamp)| {
            timestamp.elapsed() < ttl
        });

        before_len - self.cache.len()
    }

    /// Clear all entries from the cache
    ///
    /// Useful for forcing a refresh of all market data.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get the current TTL setting
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Set a new TTL for future cache entries
    ///
    /// Note: This does not affect existing cache entries' expiration times.
    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for MarketCache {
    /// Create a market cache with default 1-hour TTL
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn create_test_market(symbol: &str) -> Market {
        Market {
            id: symbol.to_string(),
            symbol: symbol.to_string(),
            base: "BTC".to_string(),
            quote: "USDT".to_string(),
            settle: None,
            base_id: "BTC".to_string(),
            quote_id: "USDT".to_string(),
            settle_id: None,
            market_type: "spot".to_string(),
            spot: true,
            margin: false,
            swap: false,
            future: false,
            option: false,
            active: true,
            contract: None,
            linear: None,
            inverse: None,
            taker: None,
            maker: None,
            contract_size: None,
            expiry: None,
            expiry_datetime: None,
            strike: None,
            option_type: None,
            created: None,
            margin_modes: None,
            precision: MarketPrecision {
                price: Some(2),
                amount: Some(8),
                cost: None,
                base: None,
                quote: None,
            },
            limits: MarketLimits {
                amount: None,
                price: None,
                cost: None,
                leverage: None,
            },
            info: None,
        }
    }

    #[test]
    fn test_market_cache_new() {
        let cache = MarketCache::new(Duration::from_secs(60));
        assert_eq!(cache.ttl(), Duration::from_secs(60));
        assert!(cache.is_empty());
    }

    #[test]
    fn test_market_cache_default() {
        let cache = MarketCache::default();
        assert_eq!(cache.ttl(), Duration::from_secs(3600));
        assert!(cache.is_empty());
    }

    #[test]
    fn test_market_cache_hit() {
        let mut cache = MarketCache::new(Duration::from_secs(60));
        let markets = vec![create_test_market("BTC/USDT")];

        cache.insert("binance".to_string(), markets.clone());

        let cached = cache.get("binance");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_market_cache_miss() {
        let cache = MarketCache::new(Duration::from_secs(60));

        let cached = cache.get("binance");
        assert!(cached.is_none());
    }

    #[test]
    fn test_market_cache_expiration() {
        let mut cache = MarketCache::new(Duration::from_millis(100));
        let markets = vec![create_test_market("BTC/USDT")];

        cache.insert("binance".to_string(), markets.clone());

        // Should be cached immediately
        let cached = cache.get("binance");
        assert!(cached.is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));

        // Should be expired now
        let cached = cache.get("binance");
        assert!(cached.is_none());
    }

    #[test]
    fn test_market_cache_clear() {
        let mut cache = MarketCache::new(Duration::from_secs(60));
        let markets = vec![create_test_market("BTC/USDT")];

        cache.insert("binance".to_string(), markets.clone());
        cache.insert("bybit".to_string(), markets.clone());

        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_market_cache_clear_expired() {
        let mut cache = MarketCache::new(Duration::from_millis(100));
        let markets = vec![create_test_market("BTC/USDT")];

        // Insert first entry
        cache.insert("binance".to_string(), markets.clone());

        // Wait for first entry to be close to expiration
        std::thread::sleep(Duration::from_millis(80));

        // Insert second entry (not expired yet)
        cache.insert("bybit".to_string(), markets.clone());

        // Wait for first entry to expire (but not the second)
        std::thread::sleep(Duration::from_millis(40));

        // Clear expired entries
        let removed = cache.clear_expired();
        assert_eq!(removed, 1); // First entry should be expired
        assert_eq!(cache.len(), 1); // Second entry should remain
        assert!(cache.get("bybit").is_some());
    }

    #[test]
    fn test_market_cache_zero_ttl_disables_caching() {
        let mut cache = MarketCache::new(Duration::ZERO);
        let markets = vec![create_test_market("BTC/USDT")];

        cache.insert("binance".to_string(), markets.clone());

        // Should not cache when TTL is zero
        let cached = cache.get("binance");
        assert!(cached.is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_market_cache_set_ttl() {
        let mut cache = MarketCache::new(Duration::from_secs(60));
        assert_eq!(cache.ttl(), Duration::from_secs(60));

        cache.set_ttl(Duration::from_secs(120));
        assert_eq!(cache.ttl(), Duration::from_secs(120));
    }

    #[test]
    fn test_market_cache_multiple_exchanges() {
        let mut cache = MarketCache::new(Duration::from_secs(60));
        let binance_markets = vec![create_test_market("BTC/USDT")];
        let bybit_markets = vec![
            create_test_market("BTC/USDT"),
            create_test_market("ETH/USDT"),
        ];

        cache.insert("binance".to_string(), binance_markets.clone());
        cache.insert("bybit".to_string(), bybit_markets.clone());

        assert_eq!(cache.len(), 2);

        let binance_cached = cache.get("binance");
        assert!(binance_cached.is_some());
        assert_eq!(binance_cached.unwrap().len(), 1);

        let bybit_cached = cache.get("bybit");
        assert!(bybit_cached.is_some());
        assert_eq!(bybit_cached.unwrap().len(), 2);
    }
}
