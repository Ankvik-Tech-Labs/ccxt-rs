use ccxt::types::Market;
use rust_decimal::Decimal;

/// Validate a Market struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_market(market: &Market) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty and contain '/'
    if market.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }
    if !market.symbol.contains('/') {
        errors.push(format!("symbol '{}' does not contain '/'", market.symbol));
    }

    // Base and quote must be non-empty
    if market.base.is_empty() {
        errors.push("base is empty".to_string());
    }
    if market.quote.is_empty() {
        errors.push("quote is empty".to_string());
    }

    // base_id and quote_id must be non-empty
    if market.base_id.is_empty() {
        errors.push("base_id is empty".to_string());
    }
    if market.quote_id.is_empty() {
        errors.push("quote_id is empty".to_string());
    }

    // market_type must be one of the known types
    let valid_types = ["spot", "swap", "future", "option", "margin"];
    if !valid_types.contains(&market.market_type.as_str()) {
        errors.push(format!(
            "market_type '{}' is not a known type (expected one of {:?})",
            market.market_type, valid_types
        ));
    }

    // Type flags should be consistent
    // spot should exclude contract
    if market.spot && market.contract == Some(true) {
        errors.push("spot market should not be a contract".to_string());
    }

    // swap/future/option implies contract
    if (market.swap || market.future || market.option) && market.contract == Some(false) {
        errors.push("derivative market (swap/future/option) should be a contract".to_string());
    }

    // Precision values should be reasonable (0 to 20)
    if let Some(price_prec) = market.precision.price {
        if price_prec < 0 || price_prec > 20 {
            errors.push(format!(
                "precision.price ({}) is outside reasonable range [0, 20]",
                price_prec
            ));
        }
    }
    if let Some(amount_prec) = market.precision.amount {
        if amount_prec < 0 || amount_prec > 20 {
            errors.push(format!(
                "precision.amount ({}) is outside reasonable range [0, 20]",
                amount_prec
            ));
        }
    }

    // Fee rates should be in valid range (0 to 1)
    for (name, val) in [("taker", market.taker), ("maker", market.maker)] {
        if let Some(fee) = val {
            if fee < Decimal::ZERO || fee > Decimal::ONE {
                errors.push(format!(
                    "{} fee ({}) is outside valid range [0, 1]",
                    name, fee
                ));
            }
        }
    }

    // Limits: min should be <= max when both present
    if let Some(ref amount_limits) = market.limits.amount {
        if let (Some(min), Some(max)) = (amount_limits.min, amount_limits.max) {
            if min > max && max > Decimal::ZERO {
                errors.push(format!(
                    "limits.amount.min ({}) > limits.amount.max ({})",
                    min, max
                ));
            }
        }
    }
    if let Some(ref price_limits) = market.limits.price {
        if let (Some(min), Some(max)) = (price_limits.min, price_limits.max) {
            if min > max && max > Decimal::ZERO {
                errors.push(format!(
                    "limits.price.min ({}) > limits.price.max ({})",
                    min, max
                ));
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccxt::types::{MarketLimits, MarketPrecision, MinMax};

    fn valid_market() -> Market {
        Market {
            id: "BTCUSDT".to_string(),
            symbol: "BTC/USDT".to_string(),
            base: "BTC".to_string(),
            quote: "USDT".to_string(),
            settle: None,
            base_id: "BTC".to_string(),
            quote_id: "USDT".to_string(),
            settle_id: None,
            market_type: "spot".to_string(),
            spot: true,
            margin: false,
            swap: false,
            future: false,
            option: false,
            active: true,
            contract: None,
            linear: None,
            inverse: None,
            taker: Some(Decimal::new(1, 3)), // 0.001
            maker: Some(Decimal::new(1, 3)),
            contract_size: None,
            expiry: None,
            expiry_datetime: None,
            strike: None,
            option_type: None,
            created: None,
            margin_modes: None,
            precision: MarketPrecision {
                price: Some(2),
                amount: Some(5),
                cost: None,
                base: None,
                quote: None,
            },
            limits: MarketLimits {
                amount: Some(MinMax {
                    min: Some(Decimal::new(1, 5)),
                    max: Some(Decimal::new(9000, 0)),
                }),
                price: Some(MinMax {
                    min: Some(Decimal::new(1, 2)),
                    max: Some(Decimal::new(1000000, 0)),
                }),
                cost: Some(MinMax {
                    min: Some(Decimal::new(10, 0)),
                    max: None,
                }),
                leverage: None,
            },
            info: None,
        }
    }

    #[test]
    fn test_valid_market_passes() {
        let errors = validate_market(&valid_market());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_spot_with_contract_flag() {
        let mut m = valid_market();
        m.contract = Some(true);
        let errors = validate_market(&m);
        assert!(errors.iter().any(|e| e.contains("spot") && e.contains("contract")));
    }
}
