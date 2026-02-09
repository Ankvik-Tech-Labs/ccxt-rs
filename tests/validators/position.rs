use ccxt::types::Position;
use rust_decimal::Decimal;

/// Validate a Position struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_position(position: &Position) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty and contain '/'
    if position.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }
    if !position.symbol.contains('/') {
        errors.push(format!(
            "symbol '{}' does not contain '/'",
            position.symbol
        ));
    }

    // Timestamp must be positive
    if position.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", position.timestamp));
    }

    // Contracts must be >= 0
    if position.contracts < Decimal::ZERO {
        errors.push(format!(
            "contracts ({}) must be >= 0",
            position.contracts
        ));
    }

    // Leverage must be in reasonable range when present (0 to 200)
    if let Some(leverage) = position.leverage {
        if leverage < Decimal::ZERO || leverage > Decimal::new(200, 0) {
            errors.push(format!(
                "leverage ({}) is outside reasonable range [0, 200]",
                leverage
            ));
        }
    }

    // Entry price must be > 0 when present
    if let Some(entry_price) = position.entry_price {
        if entry_price <= Decimal::ZERO {
            errors.push(format!(
                "entry_price ({}) must be > 0",
                entry_price
            ));
        }
    }

    // Mark price must be > 0 when present
    if let Some(mark_price) = position.mark_price {
        if mark_price <= Decimal::ZERO {
            errors.push(format!(
                "mark_price ({}) must be > 0",
                mark_price
            ));
        }
    }

    // Liquidation price must be > 0 when present
    if let Some(liq_price) = position.liquidation_price {
        if liq_price <= Decimal::ZERO {
            errors.push(format!(
                "liquidation_price ({}) must be > 0",
                liq_price
            ));
        }
    }

    // Initial margin must be >= 0 when present
    if let Some(initial_margin) = position.initial_margin {
        if initial_margin < Decimal::ZERO {
            errors.push(format!(
                "initial_margin ({}) must be >= 0",
                initial_margin
            ));
        }
    }

    // Maintenance margin must be >= 0 when present
    if let Some(maint_margin) = position.maintenance_margin {
        if maint_margin < Decimal::ZERO {
            errors.push(format!(
                "maintenance_margin ({}) must be >= 0",
                maint_margin
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccxt::types::common::{MarginMode, PositionSide};

    fn valid_position() -> Position {
        Position {
            symbol: "BTC/USDT:USDT".to_string(),
            id: None,
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            side: PositionSide::Long,
            margin_mode: MarginMode::Cross,
            contracts: Decimal::new(1, 0),
            contract_size: Some(Decimal::ONE),
            notional: Some(Decimal::new(50000, 0)),
            leverage: Some(Decimal::new(10, 0)),
            entry_price: Some(Decimal::new(50000, 0)),
            mark_price: Some(Decimal::new(50100, 0)),
            unrealized_pnl: Some(Decimal::new(100, 0)),
            realized_pnl: None,
            collateral: None,
            initial_margin: Some(Decimal::new(5000, 0)),
            maintenance_margin: Some(Decimal::new(250, 0)),
            liquidation_price: Some(Decimal::new(45000, 0)),
            margin_ratio: None,
            percentage: None,
            stop_loss_price: None,
            take_profit_price: None,
            hedged: None,
            maintenance_margin_percentage: None,
            initial_margin_percentage: None,
            last_update_timestamp: None,
            last_price: None,
            info: None,
        }
    }

    #[test]
    fn test_valid_position_passes() {
        let errors = validate_position(&valid_position());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_invalid_leverage() {
        let mut p = valid_position();
        p.leverage = Some(Decimal::new(300, 0));
        let errors = validate_position(&p);
        assert!(errors.iter().any(|e| e.contains("leverage")));
    }
}
