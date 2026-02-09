use ccxt::types::FundingRate;
use rust_decimal::Decimal;

/// Validate a FundingRate struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_funding_rate(fr: &FundingRate) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty
    if fr.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }

    // Timestamp must be positive
    if fr.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", fr.timestamp));
    }

    // datetime must be non-empty
    if fr.datetime.is_empty() {
        errors.push("datetime is empty".to_string());
    }

    // Funding rate should be within reasonable bounds (-1% to +1% per interval)
    let min_rate = Decimal::new(-1, 2); // -0.01
    let max_rate = Decimal::new(1, 2); // 0.01
    if fr.funding_rate < min_rate || fr.funding_rate > max_rate {
        errors.push(format!(
            "funding_rate ({}) is outside reasonable range [-0.01, 0.01]",
            fr.funding_rate
        ));
    }

    // Mark price must be > 0 when present
    if let Some(mark_price) = fr.mark_price {
        if mark_price <= Decimal::ZERO {
            errors.push(format!("mark_price ({}) must be > 0", mark_price));
        }
    }

    // Index price must be > 0 when present
    if let Some(index_price) = fr.index_price {
        if index_price <= Decimal::ZERO {
            errors.push(format!("index_price ({}) must be > 0", index_price));
        }
    }

    // Funding timestamp must be > current timestamp when present (it's next funding)
    if let Some(ft) = fr.funding_timestamp {
        if ft <= 0 {
            errors.push(format!(
                "funding_timestamp ({}) must be positive",
                ft
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_funding_rate() -> FundingRate {
        FundingRate {
            symbol: "BTC/USDT:USDT".to_string(),
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            funding_rate: Decimal::new(1, 4), // 0.0001
            funding_timestamp: Some(1700028800000),
            funding_datetime: Some("2023-11-15T06:13:20.000Z".to_string()),
            mark_price: Some(Decimal::new(50000, 0)),
            index_price: Some(Decimal::new(50010, 0)),
            interest_rate: Some(Decimal::new(1, 4)),
            estimated_settle_price: None,
            interval: Some("8h".to_string()),
            previous_funding_rate: None,
            previous_funding_timestamp: None,
            previous_funding_datetime: None,
            next_funding_rate: None,
            next_funding_timestamp: None,
            next_funding_datetime: None,
            info: None,
        }
    }

    #[test]
    fn test_valid_funding_rate_passes() {
        let errors = validate_funding_rate(&valid_funding_rate());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_extreme_funding_rate() {
        let mut fr = valid_funding_rate();
        fr.funding_rate = Decimal::new(5, 2); // 0.05 = 5% — unreasonable
        let errors = validate_funding_rate(&fr);
        assert!(errors.iter().any(|e| e.contains("funding_rate")));
    }
}
