mod common;

use common::comparison::{assert_json_eq, assert_json_eq_with_skip, compare_json, ComparisonConfig};
use serde_json::json;

#[test]
fn test_identical_json() {
    let a = json!({"symbol": "BTC/USDT", "last": 50000.0, "bid": 49999.0});
    let b = json!({"symbol": "BTC/USDT", "last": 50000.0, "bid": 49999.0});
    assert_json_eq(&a, &b);
}

#[test]
fn test_skip_timestamp_and_datetime() {
    let a = json!({
        "symbol": "BTC/USDT",
        "timestamp": 1000,
        "datetime": "2023-01-01T00:00:00.000Z"
    });
    let b = json!({
        "symbol": "BTC/USDT",
        "timestamp": 2000,
        "datetime": "2024-01-01T00:00:00.000Z"
    });
    assert_json_eq(&a, &b);
}

#[test]
fn test_decimal_string_vs_number() {
    let a = json!({"price": "50000.12"});
    let b = json!({"price": 50000.12});
    assert_json_eq(&a, &b);
}

#[test]
fn test_numeric_tolerance() {
    let a = json!({"price": 50000.05});
    let b = json!({"price": 50000.00});
    let config = ComparisonConfig::default().with_tolerance(0.01);
    let errors = compare_json(&a, &b, &config);
    assert!(errors.is_empty(), "Errors: {:?}", errors);
}

#[test]
fn test_custom_skip_keys() {
    let a = json!({"symbol": "BTC/USDT", "info": {"raw": "data1"}});
    let b = json!({"symbol": "BTC/USDT", "info": {"raw": "data2"}});
    assert_json_eq_with_skip(&a, &b, &["info"]);
}

#[test]
fn test_nested_comparison() {
    let a = json!({
        "ticker": {
            "symbol": "BTC/USDT",
            "price": "50000.0"
        }
    });
    let b = json!({
        "ticker": {
            "symbol": "BTC/USDT",
            "price": 50000.0
        }
    });
    assert_json_eq(&a, &b);
}

#[test]
fn test_array_comparison() {
    let a = json!([["50000.0", "1.5"], ["49999.0", "2.0"]]);
    let b = json!([["50000.0", "1.5"], ["49999.0", "2.0"]]);
    assert_json_eq(&a, &b);
}

#[test]
#[should_panic(expected = "JSON comparison failed")]
fn test_mismatch_panics() {
    let a = json!({"symbol": "BTC/USDT"});
    let b = json!({"symbol": "ETH/USDT"});
    let config = ComparisonConfig::strict();
    let errors = compare_json(&a, &b, &config);
    if !errors.is_empty() {
        panic!("JSON comparison failed with {} error(s)", errors.len());
    }
}
