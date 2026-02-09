use ccxt::types::FundingRateHistory;
use rust_decimal::Decimal;

/// Validate a FundingRateHistory struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_funding_rate_history(frh: &FundingRateHistory) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty
    if frh.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }

    // Timestamp must be positive
    if frh.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", frh.timestamp));
    }

    // datetime must be non-empty
    if frh.datetime.is_empty() {
        errors.push("datetime is empty".to_string());
    }

    // Funding rate should be within reasonable bounds (-1% to +1% per interval)
    let min_rate = Decimal::new(-1, 2); // -0.01
    let max_rate = Decimal::new(1, 2); // 0.01
    if frh.funding_rate < min_rate || frh.funding_rate > max_rate {
        errors.push(format!(
            "funding_rate ({}) is outside reasonable range [-0.01, 0.01]",
            frh.funding_rate
        ));
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_funding_rate_history() -> FundingRateHistory {
        FundingRateHistory {
            symbol: "BTC/USD:USDC".to_string(),
            funding_rate: Decimal::new(1, 4), // 0.0001
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            info: None,
        }
    }

    #[test]
    fn test_valid_funding_rate_history_passes() {
        let errors = validate_funding_rate_history(&valid_funding_rate_history());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_extreme_funding_rate() {
        let mut frh = valid_funding_rate_history();
        frh.funding_rate = Decimal::new(5, 2); // 0.05 = 5% — unreasonable
        let errors = validate_funding_rate_history(&frh);
        assert!(errors.iter().any(|e| e.contains("funding_rate")));
    }
}
