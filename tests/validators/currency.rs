use ccxt::types::Currency;
use rust_decimal::Decimal;

/// Validate a Currency struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_currency(currency: &Currency) -> Vec<String> {
    let mut errors = Vec::new();

    // Code must be non-empty
    if currency.code.is_empty() {
        errors.push("code is empty".to_string());
    }

    // ID must be non-empty
    if currency.id.is_empty() {
        errors.push("id is empty".to_string());
    }

    // Fee when present must be >= 0
    if let Some(fee) = currency.fee {
        if fee < Decimal::ZERO {
            errors.push(format!("fee ({}) must be >= 0", fee));
        }
    }

    // Precision when present must be >= 0
    if let Some(precision) = currency.precision {
        if precision < 0 {
            errors.push(format!("precision ({}) must be >= 0", precision));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_currency() -> Currency {
        Currency {
            code: "BTC".to_string(),
            id: "btc".to_string(),
            name: Some("Bitcoin".to_string()),
            active: true,
            deposit: Some(true),
            withdraw: Some(true),
            fee: Some(Decimal::new(5, 4)), // 0.0005
            precision: Some(8),
            limits: None,
            networks: None,
            info: None,
        }
    }

    #[test]
    fn test_valid_currency_passes() {
        let errors = validate_currency(&valid_currency());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_negative_fee() {
        let mut c = valid_currency();
        c.fee = Some(Decimal::new(-1, 0));
        let errors = validate_currency(&c);
        assert!(errors.iter().any(|e| e.contains("fee")));
    }
}
