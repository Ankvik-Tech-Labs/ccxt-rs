//! Integration test for market cache functionality

#[cfg(feature = "binance")]
use ccxt::binance::Binance;
use ccxt::base::Exchange;
use std::time::Duration;

#[cfg(feature = "binance")]
#[tokio::test]
#[ignore] // Ignored by default since it requires network access
async fn test_binance_market_cache() -> Result<(), Box<dyn std::error::Error>> {
    // Build exchange with short TTL for testing
    let binance = Binance::builder()
        .market_cache_ttl(Duration::from_secs(5))
        .build()?;

    // First fetch - should hit the API
    let start = std::time::Instant::now();
    let markets1 = binance.fetch_markets().await?;
    let first_duration = start.elapsed();

    println!("First fetch: {} markets in {:?}", markets1.len(), first_duration);
    assert!(!markets1.is_empty());

    // Second fetch - should hit the cache (much faster)
    let start = std::time::Instant::now();
    let markets2 = binance.fetch_markets().await?;
    let second_duration = start.elapsed();

    println!("Second fetch (cached): {} markets in {:?}", markets2.len(), second_duration);
    assert_eq!(markets1.len(), markets2.len());

    // Cache should be faster than API call
    // Note: This might not always be true due to network variance, but cache should be < 10ms
    assert!(second_duration < Duration::from_millis(10),
        "Cache hit should be fast: {:?}", second_duration);

    Ok(())
}

#[cfg(feature = "binance")]
#[tokio::test]
#[ignore] // Ignored by default since it requires network access
async fn test_market_cache_ttl_expiration() -> Result<(), Box<dyn std::error::Error>> {
    // Build exchange with very short TTL
    let binance = Binance::builder()
        .market_cache_ttl(Duration::from_secs(2))
        .build()?;

    // First fetch
    let markets1 = binance.fetch_markets().await?;
    println!("First fetch: {} markets", markets1.len());

    // Wait for cache to expire
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Second fetch - cache should be expired, hits API again
    let markets2 = binance.fetch_markets().await?;
    println!("Second fetch (after expiry): {} markets", markets2.len());

    assert!(!markets1.is_empty());
    assert!(!markets2.is_empty());

    Ok(())
}

#[cfg(feature = "binance")]
#[tokio::test]
async fn test_market_cache_disabled() -> Result<(), Box<dyn std::error::Error>> {
    // Build exchange with caching disabled
    let _binance = Binance::builder()
        .market_cache_ttl(Duration::ZERO)
        .build()?;

    // Even with cache disabled, fetch_markets should work
    // We can't test this without network, but we can at least verify it builds correctly

    Ok(())
}
