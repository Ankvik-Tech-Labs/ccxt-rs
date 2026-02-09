use ccxt::types::Trade;
use rust_decimal::Decimal;

/// Validate a Trade struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_trade(trade: &Trade) -> Vec<String> {
    let mut errors = Vec::new();

    // ID must be non-empty
    if trade.id.is_empty() {
        errors.push("id is empty".to_string());
    }

    // Symbol must be non-empty and contain '/'
    if trade.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }
    if !trade.symbol.contains('/') {
        errors.push(format!("symbol '{}' does not contain '/'", trade.symbol));
    }

    // Timestamp must be positive
    if trade.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", trade.timestamp));
    }

    // datetime must be non-empty
    if trade.datetime.is_empty() {
        errors.push("datetime is empty".to_string());
    }

    // Price must be > 0
    if trade.price <= Decimal::ZERO {
        errors.push(format!("price ({}) must be > 0", trade.price));
    }

    // Amount must be > 0
    if trade.amount <= Decimal::ZERO {
        errors.push(format!("amount ({}) must be > 0", trade.amount));
    }

    // Cost must be >= 0
    if trade.cost < Decimal::ZERO {
        errors.push(format!("cost ({}) must be >= 0", trade.cost));
    }

    // cost ≈ price * amount (5% tolerance for rounding)
    if trade.price > Decimal::ZERO && trade.amount > Decimal::ZERO {
        let calculated_cost = trade.price * trade.amount;
        if trade.cost > Decimal::ZERO {
            let diff = (calculated_cost - trade.cost).abs();
            let tolerance = trade.cost * Decimal::new(5, 2); // 5%
            if diff > tolerance {
                errors.push(format!(
                    "cost ({}) does not match price * amount ({}) within 5%",
                    trade.cost, calculated_cost
                ));
            }
        }
    }

    // Fee validation
    if let Some(ref fee) = trade.fee {
        if fee.cost < Decimal::ZERO {
            errors.push(format!("fee.cost ({}) must be >= 0", fee.cost));
        }
        if fee.currency.is_empty() {
            errors.push("fee.currency is empty".to_string());
        }
    }

    // taker_or_maker must be "taker" or "maker" when present
    if let Some(ref tom) = trade.taker_or_maker {
        if tom != "taker" && tom != "maker" {
            errors.push(format!(
                "taker_or_maker '{}' must be 'taker' or 'maker'",
                tom
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccxt::types::common::OrderSide;

    fn valid_trade() -> Trade {
        Trade {
            id: "12345".to_string(),
            symbol: "BTC/USDT".to_string(),
            order: None,
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            side: OrderSide::Buy,
            price: Decimal::new(50000, 0),
            amount: Decimal::new(1, 0),
            cost: Decimal::new(50000, 0),
            fee: None,
            taker_or_maker: Some("taker".to_string()),
            info: None,
        }
    }

    #[test]
    fn test_valid_trade_passes() {
        let errors = validate_trade(&valid_trade());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_negative_price() {
        let mut t = valid_trade();
        t.price = Decimal::new(-1, 0);
        let errors = validate_trade(&t);
        assert!(errors.iter().any(|e| e.contains("price")));
    }

    #[test]
    fn test_invalid_taker_or_maker() {
        let mut t = valid_trade();
        t.taker_or_maker = Some("unknown".to_string());
        let errors = validate_trade(&t);
        assert!(errors.iter().any(|e| e.contains("taker_or_maker")));
    }
}
