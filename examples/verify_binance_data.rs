//! Verify Binance data format
//!
//! This example fetches real data from Binance public API to verify
//! the response format before implementing the full exchange.

use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Binance Data Format Verification ===\n");

    let client = reqwest::Client::new();

    // Test 1: Fetch ticker for BTC/USDT
    println!("1. Fetching ticker for BTCUSDT...");
    let ticker_url = "https://api.binance.com/api/v3/ticker/24hr?symbol=BTCUSDT";
    let ticker_response = client.get(ticker_url).send().await?;
    let ticker_json: Value = ticker_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response:\n{}\n", serde_json::to_string_pretty(&ticker_json)?);

    // Test 2: Fetch order book
    println!("2. Fetching order book for BTCUSDT (limit=5)...");
    let orderbook_url = "https://api.binance.com/api/v3/depth?symbol=BTCUSDT&limit=5";
    let orderbook_response = client.get(orderbook_url).send().await?;
    let orderbook_json: Value = orderbook_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response:\n{}\n", serde_json::to_string_pretty(&orderbook_json)?);

    // Test 3: Fetch recent trades
    println!("3. Fetching recent trades for BTCUSDT (limit=5)...");
    let trades_url = "https://api.binance.com/api/v3/trades?symbol=BTCUSDT&limit=5";
    let trades_response = client.get(trades_url).send().await?;
    let trades_json: Value = trades_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response:\n{}\n", serde_json::to_string_pretty(&trades_json)?);

    // Test 4: Fetch OHLCV (klines)
    println!("4. Fetching OHLCV/Klines for BTCUSDT (1h, limit=3)...");
    let klines_url = "https://api.binance.com/api/v3/klines?symbol=BTCUSDT&interval=1h&limit=3";
    let klines_response = client.get(klines_url).send().await?;
    let klines_json: Value = klines_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response:\n{}\n", serde_json::to_string_pretty(&klines_json)?);

    // Test 5: Fetch exchange info (markets)
    println!("5. Fetching exchange info for BTCUSDT...");
    let exchange_info_url = "https://api.binance.com/api/v3/exchangeInfo?symbol=BTCUSDT";
    let exchange_info_response = client.get(exchange_info_url).send().await?;
    let exchange_info_json: Value = exchange_info_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response (symbols only):\n{}\n",
        serde_json::to_string_pretty(
            exchange_info_json.get("symbols").unwrap_or(&Value::Null)
        )?
    );

    // Test 6: Server time
    println!("6. Fetching server time...");
    let time_url = "https://api.binance.com/api/v3/time";
    let time_response = client.get(time_url).send().await?;
    let time_json: Value = time_response.json().await?;
    println!("   Status: Success ✓");
    println!("   Response:\n{}\n", serde_json::to_string_pretty(&time_json)?);

    println!("=== All data fetching tests completed successfully! ===");

    Ok(())
}
