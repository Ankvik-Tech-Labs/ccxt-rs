use ccxt::types::LeverageTier;
use rust_decimal::Decimal;

/// Validate a LeverageTier struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_leverage_tier(tier: &LeverageTier) -> Vec<String> {
    let mut errors = Vec::new();

    // Tier must be > 0
    if tier.tier == 0 {
        errors.push("tier must be > 0".to_string());
    }

    // Max leverage when present must be > 0 and <= 200
    if let Some(max_leverage) = tier.max_leverage {
        if max_leverage <= Decimal::ZERO {
            errors.push(format!(
                "max_leverage ({}) must be > 0",
                max_leverage
            ));
        }
        if max_leverage > Decimal::new(200, 0) {
            errors.push(format!(
                "max_leverage ({}) must be <= 200",
                max_leverage
            ));
        }
    }

    // Maintenance margin rate when present must be >= 0 and <= 1
    if let Some(mmr) = tier.maintenance_margin_rate {
        if mmr < Decimal::ZERO {
            errors.push(format!(
                "maintenance_margin_rate ({}) must be >= 0",
                mmr
            ));
        }
        if mmr > Decimal::ONE {
            errors.push(format!(
                "maintenance_margin_rate ({}) must be <= 1",
                mmr
            ));
        }
    }

    // Max notional must be >= min notional when both are present
    if let (Some(min_notional), Some(max_notional)) = (tier.min_notional, tier.max_notional) {
        if max_notional < min_notional {
            errors.push(format!(
                "max_notional ({}) must be >= min_notional ({})",
                max_notional, min_notional
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_leverage_tier() -> LeverageTier {
        LeverageTier {
            tier: 1,
            currency: Some("USDT".to_string()),
            min_notional: Some(Decimal::new(0, 0)),
            max_notional: Some(Decimal::new(50000, 0)),
            maintenance_margin_rate: Some(Decimal::new(5, 3)), // 0.005
            max_leverage: Some(Decimal::new(125, 0)),
            info: None,
        }
    }

    #[test]
    fn test_valid_leverage_tier_passes() {
        let errors = validate_leverage_tier(&valid_leverage_tier());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_invalid_max_leverage() {
        let mut t = valid_leverage_tier();
        t.max_leverage = Some(Decimal::new(300, 0));
        let errors = validate_leverage_tier(&t);
        assert!(errors.iter().any(|e| e.contains("max_leverage")));
    }
}
