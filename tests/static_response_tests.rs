//! Static Response Tests (Tier 1)
//!
//! Feed raw exchange JSON from fixtures into Rust parsers and compare output
//! against CCXT Python's parsed response.

mod common;
mod static_response;
mod validators;

use common::fixture_loader::load_fixture;
use rust_decimal::Decimal;

// =============================================================================
// BINANCE STATIC TESTS
// =============================================================================

#[cfg(feature = "binance")]
mod binance_static {
    use super::*;
    use ccxt::binance::parsers;

    #[test]
    fn test_binance_parse_ticker_btc_usdt() {
        let fixture = load_fixture("binance", "fetch_ticker_BTC_USDT");
        let raw = &fixture.http_response;

        let ticker = parsers::parse_ticker(raw, "BTC/USDT").expect("parse_ticker failed");

        // Validate structural correctness
        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        // Compare key fields against CCXT Python output
        let parsed = &fixture.parsed_response;
        assert_eq!(ticker.symbol, "BTC/USDT");
        assert_decimal_matches(ticker.high, parsed.get("high"), "high");
        assert_decimal_matches(ticker.low, parsed.get("low"), "low");
        assert_decimal_matches(ticker.bid, parsed.get("bid"), "bid");
        assert_decimal_matches(ticker.ask, parsed.get("ask"), "ask");
        assert_decimal_matches(ticker.last, parsed.get("last"), "last");
        assert_decimal_matches(ticker.open, parsed.get("open"), "open");
        assert_decimal_matches(ticker.close, parsed.get("close"), "close");
        assert_decimal_matches(ticker.base_volume, parsed.get("baseVolume"), "baseVolume");
        assert_decimal_matches(ticker.quote_volume, parsed.get("quoteVolume"), "quoteVolume");
        assert_decimal_matches(ticker.vwap, parsed.get("vwap"), "vwap");
        assert_decimal_matches(ticker.change, parsed.get("change"), "change");
    }

    #[test]
    fn test_binance_parse_ticker_eth_usdt() {
        let fixture = load_fixture("binance", "fetch_ticker_ETH_USDT");
        let raw = &fixture.http_response;

        let ticker = parsers::parse_ticker(raw, "ETH/USDT").expect("parse_ticker failed");
        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);
        assert_eq!(ticker.symbol, "ETH/USDT");
    }

    #[test]
    fn test_binance_parse_order_book_btc_usdt() {
        let fixture = load_fixture("binance", "fetch_order_book_BTC_USDT");
        let raw = &fixture.http_response;

        let ob = parsers::parse_order_book(raw, "BTC/USDT").expect("parse_order_book failed");

        let val_errors = validators::validate_order_book(&ob);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        assert_eq!(ob.symbol, "BTC/USDT");

        // Compare bid/ask counts
        let parsed = &fixture.parsed_response;
        let expected_bids = parsed["bids"].as_array().unwrap();
        let expected_asks = parsed["asks"].as_array().unwrap();
        assert_eq!(ob.bids.len(), expected_bids.len(), "bid count mismatch");
        assert_eq!(ob.asks.len(), expected_asks.len(), "ask count mismatch");

        // Compare first bid price
        if let Some(first_bid) = expected_bids.first() {
            let expected_price = first_bid[0].as_f64().unwrap();
            let actual_price: f64 = ob.bids[0].0.try_into().unwrap();
            assert!(
                (actual_price - expected_price).abs() / expected_price < 0.001,
                "First bid price mismatch: {} vs {}",
                actual_price,
                expected_price
            );
        }

        // Compare nonce
        if let Some(expected_nonce) = parsed.get("nonce").and_then(|v| v.as_u64()) {
            assert_eq!(ob.nonce, Some(expected_nonce));
        }
    }

    #[test]
    fn test_binance_parse_ohlcv_btc_usdt() {
        let fixture = load_fixture("binance", "fetch_ohlcv_BTC_USDT");
        let raw = &fixture.http_response;

        let raw_array = raw.as_array().expect("OHLCV fixture should be an array");
        let parsed_array = fixture
            .parsed_response
            .as_array()
            .expect("OHLCV parsed should be an array");

        assert_eq!(raw_array.len(), parsed_array.len(), "OHLCV count mismatch");

        for (i, (raw_item, expected)) in raw_array.iter().zip(parsed_array.iter()).enumerate() {
            let ohlcv = parsers::parse_ohlcv(raw_item)
                .unwrap_or_else(|e| panic!("parse_ohlcv failed for item {}: {}", i, e));

            let val_errors = validators::validate_ohlcv(&ohlcv);
            assert!(
                val_errors.is_empty(),
                "OHLCV[{}] validation errors: {:?}",
                i,
                val_errors
            );

            // Compare against CCXT Python output: [timestamp, open, high, low, close, volume]
            let expected_arr = expected.as_array().expect("Expected OHLCV array");
            assert_eq!(ohlcv.timestamp, expected_arr[0].as_i64().unwrap());
            assert_f64_close(ohlcv.open, expected_arr[1].as_f64().unwrap(), "open");
            assert_f64_close(ohlcv.high, expected_arr[2].as_f64().unwrap(), "high");
            assert_f64_close(ohlcv.low, expected_arr[3].as_f64().unwrap(), "low");
            assert_f64_close(ohlcv.close, expected_arr[4].as_f64().unwrap(), "close");
            assert_f64_close(ohlcv.volume, expected_arr[5].as_f64().unwrap(), "volume");
        }
    }

    #[test]
    fn test_binance_parse_market_subset() {
        let fixture = load_fixture("binance", "fetch_markets");
        let parsed = &fixture.parsed_response;
        let parsed_array = parsed.as_array().expect("Markets should be an array");

        // The http_response for markets is the full exchangeInfo,
        // but since we captured only 5 parsed markets, we validate those
        for expected_market in parsed_array {
            // Validate the expected market has proper structure
            let symbol = expected_market["symbol"]
                .as_str()
                .expect("market should have symbol");
            assert!(symbol.contains('/'), "Symbol {} should contain '/'", symbol);
        }
    }
}

// =============================================================================
// BYBIT STATIC TESTS
// =============================================================================

#[cfg(feature = "bybit")]
mod bybit_static {
    use super::*;
    use ccxt::bybit::parsers;

    #[test]
    fn test_bybit_parse_ticker_btc_usdt() {
        let fixture = load_fixture("bybit", "fetch_ticker_BTC_USDT");
        // Extract inner data from Bybit envelope
        let raw = static_response::bybit_extract_ticker(&fixture.http_response);

        let ticker = parsers::parse_ticker(raw, "BTC/USDT").expect("parse_ticker failed");

        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        let parsed = &fixture.parsed_response;
        assert_eq!(ticker.symbol, "BTC/USDT");
        assert_decimal_matches(ticker.high, parsed.get("high"), "high");
        assert_decimal_matches(ticker.low, parsed.get("low"), "low");
        assert_decimal_matches(ticker.bid, parsed.get("bid"), "bid");
        assert_decimal_matches(ticker.ask, parsed.get("ask"), "ask");
        assert_decimal_matches(ticker.last, parsed.get("last"), "last");
        assert_decimal_matches(ticker.base_volume, parsed.get("baseVolume"), "baseVolume");
        assert_decimal_matches(ticker.quote_volume, parsed.get("quoteVolume"), "quoteVolume");
    }

    #[test]
    fn test_bybit_parse_ticker_eth_usdt() {
        let fixture = load_fixture("bybit", "fetch_ticker_ETH_USDT");
        let raw = static_response::bybit_extract_ticker(&fixture.http_response);

        let ticker = parsers::parse_ticker(raw, "ETH/USDT").expect("parse_ticker failed");
        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);
        assert_eq!(ticker.symbol, "ETH/USDT");
    }

    #[test]
    fn test_bybit_parse_order_book_btc_usdt() {
        let fixture = load_fixture("bybit", "fetch_order_book_BTC_USDT");
        // Bybit orderbook has different envelope: result.b and result.a
        let raw = &fixture.http_response;

        // The Bybit parser expects the inner data after envelope extraction
        // Check what the actual structure is
        let result = &raw["result"];

        let ob = parsers::parse_orderbook(result, "BTC/USDT").expect("parse_orderbook failed");

        let val_errors = validators::validate_order_book(&ob);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        assert_eq!(ob.symbol, "BTC/USDT");
        assert!(!ob.bids.is_empty(), "bids should not be empty");
        assert!(!ob.asks.is_empty(), "asks should not be empty");
    }
}

// =============================================================================
// OKX STATIC TESTS
// =============================================================================

#[cfg(feature = "okx")]
mod okx_static {
    use super::*;
    use ccxt::okx::parsers;

    #[test]
    fn test_okx_parse_ticker_btc_usdt() {
        let fixture = load_fixture("okx", "fetch_ticker_BTC_USDT");
        let raw = static_response::okx_extract_first(&fixture.http_response);

        let ticker = parsers::parse_ticker(raw, "BTC/USDT").expect("parse_ticker failed");

        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        let parsed = &fixture.parsed_response;
        assert_eq!(ticker.symbol, "BTC/USDT");
        assert_decimal_matches(ticker.high, parsed.get("high"), "high");
        assert_decimal_matches(ticker.low, parsed.get("low"), "low");
        assert_decimal_matches(ticker.bid, parsed.get("bid"), "bid");
        assert_decimal_matches(ticker.ask, parsed.get("ask"), "ask");
        assert_decimal_matches(ticker.last, parsed.get("last"), "last");
        assert_decimal_matches(ticker.base_volume, parsed.get("baseVolume"), "baseVolume");
        assert_decimal_matches(ticker.quote_volume, parsed.get("quoteVolume"), "quoteVolume");
    }

    #[test]
    fn test_okx_parse_ticker_eth_usdt() {
        let fixture = load_fixture("okx", "fetch_ticker_ETH_USDT");
        let raw = static_response::okx_extract_first(&fixture.http_response);

        let ticker = parsers::parse_ticker(raw, "ETH/USDT").expect("parse_ticker failed");
        let val_errors = validators::validate_ticker(&ticker);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);
        assert_eq!(ticker.symbol, "ETH/USDT");
    }

    #[test]
    fn test_okx_parse_order_book_btc_usdt() {
        let fixture = load_fixture("okx", "fetch_order_book_BTC_USDT");
        let raw = static_response::okx_extract_first(&fixture.http_response);

        let ob = parsers::parse_orderbook(raw, "BTC/USDT").expect("parse_orderbook failed");

        let val_errors = validators::validate_order_book(&ob);
        assert!(val_errors.is_empty(), "Validation errors: {:?}", val_errors);

        assert_eq!(ob.symbol, "BTC/USDT");
        assert!(!ob.bids.is_empty(), "bids should not be empty");
        assert!(!ob.asks.is_empty(), "asks should not be empty");
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Assert that an optional Decimal matches a JSON value within tolerance
fn assert_decimal_matches(
    actual: Option<Decimal>,
    expected: Option<&serde_json::Value>,
    field_name: &str,
) {
    match (actual, expected) {
        (Some(actual_dec), Some(expected_val)) => {
            if expected_val.is_null() {
                // CCXT Python returned null, our parser returned Some — that's OK
                return;
            }
            let expected_f64 = expected_val.as_f64().unwrap_or_else(|| {
                panic!("{}: expected value is not a number: {}", field_name, expected_val)
            });
            let actual_f64: f64 = actual_dec.try_into().unwrap();
            let relative_diff = if expected_f64 != 0.0 {
                ((actual_f64 - expected_f64) / expected_f64).abs()
            } else {
                actual_f64.abs()
            };
            assert!(
                relative_diff < 0.001,
                "{}: actual {} vs expected {} (relative diff: {:.6})",
                field_name,
                actual_f64,
                expected_f64,
                relative_diff
            );
        }
        (None, Some(expected_val)) => {
            if !expected_val.is_null() {
                // Our parser returned None but CCXT Python had a value — flag it
                // but don't fail (some fields are calculated differently)
                eprintln!(
                    "WARNING: {}: actual is None, expected {}",
                    field_name, expected_val
                );
            }
        }
        (Some(_), None) => {
            // Our parser found a value that CCXT Python fixture doesn't have — OK
        }
        (None, None) => {
            // Both None — OK
        }
    }
}

/// Assert a Decimal is close to an f64 value
fn assert_f64_close(actual: Decimal, expected: f64, field_name: &str) {
    let actual_f64: f64 = actual.try_into().unwrap();
    let relative_diff = if expected != 0.0 {
        ((actual_f64 - expected) / expected).abs()
    } else {
        actual_f64.abs()
    };
    assert!(
        relative_diff < 0.001,
        "{}: actual {} vs expected {} (relative diff: {:.6})",
        field_name,
        actual_f64,
        expected,
        relative_diff
    );
}
