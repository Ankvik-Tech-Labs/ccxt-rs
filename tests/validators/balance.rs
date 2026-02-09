use ccxt::types::Balances;
use rust_decimal::Decimal;

/// Validate a Balances struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_balances(balances: &Balances) -> Vec<String> {
    let mut errors = Vec::new();

    // Timestamp must be positive
    if balances.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", balances.timestamp));
    }

    // datetime must be non-empty
    if balances.datetime.is_empty() {
        errors.push("datetime is empty".to_string());
    }

    // Validate each individual balance
    for (currency, balance) in &balances.balances {
        // Currency code must match the key
        if balance.currency != *currency {
            errors.push(format!(
                "balance currency '{}' does not match key '{}'",
                balance.currency, currency
            ));
        }

        // free + used == total
        let sum = balance.free + balance.used;
        let diff = (sum - balance.total).abs();
        if diff > Decimal::ZERO {
            errors.push(format!(
                "{}: free ({}) + used ({}) = {} does not equal total ({})",
                currency, balance.free, balance.used, sum, balance.total
            ));
        }

        // All values must be >= 0
        if balance.free < Decimal::ZERO {
            errors.push(format!("{}: free ({}) must be >= 0", currency, balance.free));
        }
        if balance.used < Decimal::ZERO {
            errors.push(format!("{}: used ({}) must be >= 0", currency, balance.used));
        }
        if balance.total < Decimal::ZERO {
            errors.push(format!(
                "{}: total ({}) must be >= 0",
                currency, balance.total
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccxt::types::Balance;
    use std::collections::HashMap;

    fn valid_balances() -> Balances {
        let mut bals = HashMap::new();
        bals.insert(
            "BTC".to_string(),
            Balance::new("BTC".to_string(), Decimal::new(1, 0), Decimal::new(5, 1)),
        );
        bals.insert(
            "USDT".to_string(),
            Balance::new(
                "USDT".to_string(),
                Decimal::new(10000, 0),
                Decimal::ZERO,
            ),
        );

        let mut free = HashMap::new();
        free.insert("BTC".to_string(), Decimal::new(1, 0));
        free.insert("USDT".to_string(), Decimal::new(10000, 0));
        let mut used = HashMap::new();
        used.insert("BTC".to_string(), Decimal::new(5, 1));
        used.insert("USDT".to_string(), Decimal::ZERO);
        let mut total = HashMap::new();
        total.insert("BTC".to_string(), Decimal::new(15, 1));
        total.insert("USDT".to_string(), Decimal::new(10000, 0));

        Balances {
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            balances: bals,
            free,
            used,
            total,
            info: None,
        }
    }

    #[test]
    fn test_valid_balances_passes() {
        let errors = validate_balances(&valid_balances());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_negative_free() {
        let mut b = valid_balances();
        b.balances.insert(
            "ETH".to_string(),
            Balance {
                currency: "ETH".to_string(),
                free: Decimal::new(-1, 0),
                used: Decimal::ZERO,
                total: Decimal::new(-1, 0),
                debt: None,
            },
        );
        let errors = validate_balances(&b);
        assert!(errors.iter().any(|e| e.contains("ETH") && e.contains("free")));
    }
}
