use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// A test fixture captured from CCXT Python
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    /// Exchange name (e.g., "binance")
    pub exchange: String,

    /// Method name (e.g., "fetch_ticker")
    pub method: String,

    /// Arguments passed to the method
    pub args: Vec<Value>,

    /// Raw HTTP response from the exchange API
    pub http_response: Value,

    /// CCXT Python's parsed unified output
    pub parsed_response: Value,

    /// Keys to skip during comparison (non-deterministic fields)
    #[serde(default)]
    pub skip_keys: Vec<String>,

    /// Tolerance settings for decimal comparisons
    #[serde(default)]
    pub tolerance: Tolerance,
}

/// Tolerance settings for comparing decimal values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tolerance {
    /// Relative tolerance for percentage-like fields (e.g., 0.001 = 0.1%)
    #[serde(default = "default_percentage_tolerance")]
    pub percentage: f64,

    /// Relative tolerance for average/calculated fields
    #[serde(default = "default_average_tolerance")]
    pub average: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            percentage: default_percentage_tolerance(),
            average: default_average_tolerance(),
        }
    }
}

fn default_percentage_tolerance() -> f64 {
    0.001
}

fn default_average_tolerance() -> f64 {
    0.001
}

/// Get the path to the fixtures directory
pub fn fixtures_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tests").join("fixtures")
}

/// Load a fixture from the fixtures directory
///
/// The fixture file is expected at:
///   `tests/fixtures/{exchange}/{fixture_name}.json`
pub fn load_fixture(exchange: &str, fixture_name: &str) -> Fixture {
    let path = fixtures_dir()
        .join(exchange)
        .join(format!("{}.json", fixture_name));

    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture at {}: {}", path.display(), e));

    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture at {}: {}", path.display(), e))
}

/// List all fixture files for a given exchange
pub fn list_fixtures(exchange: &str) -> Vec<String> {
    let dir = fixtures_dir().join(exchange);
    if !dir.exists() {
        return Vec::new();
    }

    std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("Failed to read fixtures directory {}: {}", dir.display(), e))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") {
                Some(name.trim_end_matches(".json").to_string())
            } else {
                None
            }
        })
        .collect()
}
