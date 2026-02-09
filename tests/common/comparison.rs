use serde_json::Value;
use std::collections::HashSet;

/// Deep JSON comparison result
#[derive(Debug)]
pub struct ComparisonError {
    pub path: String,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mismatch at '{}': {} (expected: {}, actual: {})",
            self.path, self.message, self.expected, self.actual
        )
    }
}

/// Configuration for JSON comparison
pub struct ComparisonConfig {
    /// Keys to skip entirely during comparison
    pub skip_keys: HashSet<String>,
    /// Relative tolerance for numeric comparisons (e.g., 0.01 = 1%)
    pub numeric_tolerance: f64,
    /// Whether to allow extra keys in the actual value that aren't in expected
    pub allow_extra_keys: bool,
    /// Whether to allow None/null in actual where expected has a value
    pub allow_missing_optional: bool,
}

impl Default for ComparisonConfig {
    fn default() -> Self {
        Self {
            skip_keys: ["timestamp", "datetime", "info"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            numeric_tolerance: 0.001,
            allow_extra_keys: true,
            allow_missing_optional: true,
        }
    }
}

impl ComparisonConfig {
    pub fn with_skip_keys(mut self, keys: &[&str]) -> Self {
        self.skip_keys = keys.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.numeric_tolerance = tolerance;
        self
    }

    pub fn strict() -> Self {
        Self {
            skip_keys: HashSet::new(),
            numeric_tolerance: 0.0,
            allow_extra_keys: false,
            allow_missing_optional: false,
        }
    }
}

/// Compare two JSON values recursively, collecting all differences
pub fn compare_json(
    actual: &Value,
    expected: &Value,
    config: &ComparisonConfig,
) -> Vec<ComparisonError> {
    let mut errors = Vec::new();
    compare_recursive(actual, expected, config, "", &mut errors);
    errors
}

/// Assert two JSON values are equal (with configuration), panicking on failure
pub fn assert_json_eq_with_skip(
    actual: &Value,
    expected: &Value,
    skip_keys: &[&str],
) {
    let config = ComparisonConfig::default().with_skip_keys(skip_keys);
    let errors = compare_json(actual, expected, &config);
    if !errors.is_empty() {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        panic!(
            "JSON comparison failed with {} error(s):\n{}",
            errors.len(),
            error_msgs.join("\n")
        );
    }
}

/// Assert two JSON values are equal using default config
pub fn assert_json_eq(actual: &Value, expected: &Value) {
    let config = ComparisonConfig::default();
    let errors = compare_json(actual, expected, &config);
    if !errors.is_empty() {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        panic!(
            "JSON comparison failed with {} error(s):\n{}",
            errors.len(),
            error_msgs.join("\n")
        );
    }
}

fn compare_recursive(
    actual: &Value,
    expected: &Value,
    config: &ComparisonConfig,
    path: &str,
    errors: &mut Vec<ComparisonError>,
) {
    match (actual, expected) {
        // Both are objects
        (Value::Object(actual_map), Value::Object(expected_map)) => {
            for (key, expected_val) in expected_map {
                if config.skip_keys.contains(key) {
                    continue;
                }
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                match actual_map.get(key) {
                    Some(actual_val) => {
                        compare_recursive(actual_val, expected_val, config, &child_path, errors);
                    }
                    None => {
                        // actual is missing a key that expected has
                        if !config.allow_missing_optional || !expected_val.is_null() {
                            errors.push(ComparisonError {
                                path: child_path,
                                expected: format!("{}", expected_val),
                                actual: "missing".to_string(),
                                message: "Key missing in actual".to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Both are arrays
        (Value::Array(actual_arr), Value::Array(expected_arr)) => {
            if actual_arr.len() != expected_arr.len() {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: format!("array of length {}", expected_arr.len()),
                    actual: format!("array of length {}", actual_arr.len()),
                    message: "Array length mismatch".to_string(),
                });
                return;
            }
            for (i, (actual_elem, expected_elem)) in
                actual_arr.iter().zip(expected_arr.iter()).enumerate()
            {
                let child_path = format!("{}[{}]", path, i);
                compare_recursive(actual_elem, expected_elem, config, &child_path, errors);
            }
        }

        // Both are numbers — compare with tolerance
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(0.0);
            let b_f = b.as_f64().unwrap_or(0.0);

            if !numbers_equal(a_f, b_f, config.numeric_tolerance) {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: format!("{}", b_f),
                    actual: format!("{}", a_f),
                    message: format!(
                        "Numeric mismatch (tolerance: {})",
                        config.numeric_tolerance
                    ),
                });
            }
        }

        // String actual vs Number expected (Rust Decimal serializes to string)
        (Value::String(a_str), Value::Number(b)) => {
            if let Ok(a_f) = a_str.parse::<f64>() {
                let b_f = b.as_f64().unwrap_or(0.0);
                if !numbers_equal(a_f, b_f, config.numeric_tolerance) {
                    errors.push(ComparisonError {
                        path: path.to_string(),
                        expected: format!("{}", b_f),
                        actual: a_str.clone(),
                        message: "Numeric string vs number mismatch".to_string(),
                    });
                }
            } else {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: format!("{}", b),
                    actual: a_str.clone(),
                    message: "Type mismatch (string vs number)".to_string(),
                });
            }
        }

        // Number actual vs String expected
        (Value::Number(a), Value::String(b_str)) => {
            if let Ok(b_f) = b_str.parse::<f64>() {
                let a_f = a.as_f64().unwrap_or(0.0);
                if !numbers_equal(a_f, b_f, config.numeric_tolerance) {
                    errors.push(ComparisonError {
                        path: path.to_string(),
                        expected: b_str.clone(),
                        actual: format!("{}", a_f),
                        message: "Number vs numeric string mismatch".to_string(),
                    });
                }
            } else {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: b_str.clone(),
                    actual: format!("{}", a),
                    message: "Type mismatch (number vs string)".to_string(),
                });
            }
        }

        // Both are strings
        (Value::String(a), Value::String(b)) => {
            // Try numeric comparison first (Decimal might format differently)
            if let (Ok(a_f), Ok(b_f)) = (a.parse::<f64>(), b.parse::<f64>()) {
                if !numbers_equal(a_f, b_f, config.numeric_tolerance) {
                    errors.push(ComparisonError {
                        path: path.to_string(),
                        expected: b.clone(),
                        actual: a.clone(),
                        message: "Numeric string mismatch".to_string(),
                    });
                }
            } else if a != b {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: b.clone(),
                    actual: a.clone(),
                    message: "String mismatch".to_string(),
                });
            }
        }

        // Null handling
        (Value::Null, Value::Null) => {}
        (_, Value::Null) => {
            // actual has value, expected is null — OK if we allow extra
        }
        (Value::Null, _) => {
            if !config.allow_missing_optional {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: format!("{}", expected),
                    actual: "null".to_string(),
                    message: "Unexpected null".to_string(),
                });
            }
        }

        // Bool comparison
        (Value::Bool(a), Value::Bool(b)) => {
            if a != b {
                errors.push(ComparisonError {
                    path: path.to_string(),
                    expected: format!("{}", b),
                    actual: format!("{}", a),
                    message: "Boolean mismatch".to_string(),
                });
            }
        }

        // Type mismatch
        _ => {
            errors.push(ComparisonError {
                path: path.to_string(),
                expected: format!("{}", expected),
                actual: format!("{}", actual),
                message: "Type mismatch".to_string(),
            });
        }
    }
}

/// Compare two floating-point numbers with relative tolerance
fn numbers_equal(a: f64, b: f64, tolerance: f64) -> bool {
    if a == b {
        return true;
    }
    if a == 0.0 || b == 0.0 {
        return (a - b).abs() < tolerance;
    }
    let relative_diff = ((a - b) / b).abs();
    relative_diff <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_identical_objects() {
        let a = json!({"symbol": "BTC/USDT", "price": 50000.0});
        let b = json!({"symbol": "BTC/USDT", "price": 50000.0});
        let config = ComparisonConfig::default();
        let errors = compare_json(&a, &b, &config);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skip_keys() {
        let a = json!({"symbol": "BTC/USDT", "timestamp": 1000});
        let b = json!({"symbol": "BTC/USDT", "timestamp": 2000});
        let config = ComparisonConfig::default();
        let errors = compare_json(&a, &b, &config);
        assert!(errors.is_empty(), "timestamp should be skipped");
    }

    #[test]
    fn test_numeric_tolerance() {
        let a = json!({"price": 50000.05});
        let b = json!({"price": 50000.0});
        let config = ComparisonConfig::default().with_tolerance(0.001);
        let errors = compare_json(&a, &b, &config);
        assert!(errors.is_empty(), "Should be within tolerance");
    }

    #[test]
    fn test_string_vs_number() {
        let a = json!({"price": "50000.0"});
        let b = json!({"price": 50000.0});
        let config = ComparisonConfig::default();
        let errors = compare_json(&a, &b, &config);
        assert!(errors.is_empty(), "String '50000.0' should match number 50000.0");
    }

    #[test]
    fn test_mismatch_detected() {
        let a = json!({"symbol": "BTC/USDT"});
        let b = json!({"symbol": "ETH/USDT"});
        let config = ComparisonConfig::strict();
        let errors = compare_json(&a, &b, &config);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].path.contains("symbol"));
    }

    #[test]
    fn test_missing_key() {
        let a = json!({"symbol": "BTC/USDT"});
        let b = json!({"symbol": "BTC/USDT", "price": 50000.0});
        let config = ComparisonConfig {
            allow_missing_optional: false,
            ..ComparisonConfig::strict()
        };
        let errors = compare_json(&a, &b, &config);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_array_comparison() {
        let a = json!([[50000.0, 1.5], [49999.0, 2.0]]);
        let b = json!([[50000.0, 1.5], [49999.0, 2.0]]);
        let config = ComparisonConfig::default();
        let errors = compare_json(&a, &b, &config);
        assert!(errors.is_empty());
    }
}
