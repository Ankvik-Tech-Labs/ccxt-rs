//! Example demonstrating the market cache feature
//!
//! This example shows how to use the market cache with configurable TTL
//! to reduce API calls when fetching markets repeatedly.
//!
//! Run with: cargo run --example market_cache_demo --features binance

use ccxt::binance::Binance;
use ccxt::base::Exchange;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Market Cache Demo ===\n");

    // Build exchange with 10-second cache TTL
    println!("Building Binance exchange with 10-second cache TTL...");
    let binance = Binance::builder()
        .market_cache_ttl(Duration::from_secs(10))
        .build()?;

    // First fetch - hits the API
    println!("\n1. First fetch_markets() call (hits API)...");
    let start = std::time::Instant::now();
    let markets = binance.fetch_markets().await?;
    let duration = start.elapsed();
    println!(
        "   Fetched {} markets in {:?}",
        markets.len(),
        duration
    );

    // Second fetch - hits the cache
    println!("\n2. Second fetch_markets() call (hits cache)...");
    let start = std::time::Instant::now();
    let cached_markets = binance.fetch_markets().await?;
    let duration = start.elapsed();
    println!(
        "   Fetched {} markets in {:?} (from cache)",
        cached_markets.len(),
        duration
    );

    // Verify cache hit was faster
    println!("\n3. Cache performance:");
    println!("   Cache hit should be < 1ms: {:?}", duration);
    assert!(duration < Duration::from_millis(10));
    assert_eq!(markets.len(), cached_markets.len());

    // Wait for cache to expire
    println!("\n4. Waiting for cache to expire (10 seconds)...");
    tokio::time::sleep(Duration::from_secs(11)).await;

    // Third fetch - cache expired, hits API again
    println!("\n5. Third fetch_markets() call (cache expired, hits API again)...");
    let start = std::time::Instant::now();
    let fresh_markets = binance.fetch_markets().await?;
    let duration = start.elapsed();
    println!(
        "   Fetched {} markets in {:?}",
        fresh_markets.len(),
        duration
    );

    println!("\n=== Demo Complete ===\n");
    println!("Key takeaways:");
    println!("- First API call: Fetches from exchange (slow)");
    println!("- Cached calls: Return instantly from memory (fast)");
    println!("- After TTL expires: Automatically refreshes from API");
    println!("- Default TTL: 1 hour (configurable)");
    println!("- Disable caching: Use Duration::ZERO\n");

    Ok(())
}
