use ccxt::types::common::{OrderStatus, OrderType};
use ccxt::types::Order;
use rust_decimal::Decimal;

/// Validate an Order struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_order(order: &Order) -> Vec<String> {
    let mut errors = Vec::new();

    // ID must be non-empty
    if order.id.is_empty() {
        errors.push("id is empty".to_string());
    }

    // Symbol must be non-empty and contain '/'
    if order.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }
    if !order.symbol.contains('/') {
        errors.push(format!("symbol '{}' does not contain '/'", order.symbol));
    }

    // Timestamp must be positive
    if order.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", order.timestamp));
    }

    // Amount must be > 0
    if order.amount <= Decimal::ZERO {
        errors.push(format!("amount ({}) must be > 0", order.amount));
    }

    // Price must be > 0 when present (except market orders may have None)
    if let Some(price) = order.price {
        if price < Decimal::ZERO {
            errors.push(format!("price ({}) must be >= 0", price));
        }
    }

    // Filled must be >= 0 and <= amount
    if let Some(filled) = order.filled {
        if filled < Decimal::ZERO {
            errors.push(format!("filled ({}) must be >= 0", filled));
        }
        if filled > order.amount {
            errors.push(format!(
                "filled ({}) > amount ({})",
                filled, order.amount
            ));
        }
    }

    // Remaining must be >= 0
    if let Some(remaining) = order.remaining {
        if remaining < Decimal::ZERO {
            errors.push(format!("remaining ({}) must be >= 0", remaining));
        }
    }

    // amount = filled + remaining (when both present)
    if let (Some(filled), Some(remaining)) = (order.filled, order.remaining) {
        let sum = filled + remaining;
        let diff = (sum - order.amount).abs();
        let tolerance = order.amount * Decimal::new(1, 4); // 0.01%
        if diff > tolerance {
            errors.push(format!(
                "filled ({}) + remaining ({}) = {} does not equal amount ({})",
                filled, remaining, sum, order.amount
            ));
        }
    }

    // If closed: filled ≈ amount
    if order.status == OrderStatus::Closed {
        if let Some(filled) = order.filled {
            let diff = (filled - order.amount).abs();
            let tolerance = order.amount * Decimal::new(1, 4); // 0.01%
            if diff > tolerance {
                errors.push(format!(
                    "closed order: filled ({}) should equal amount ({})",
                    filled, order.amount
                ));
            }
        }
    }

    // If open: filled < amount
    if order.status == OrderStatus::Open {
        if let Some(filled) = order.filled {
            if filled >= order.amount {
                errors.push(format!(
                    "open order: filled ({}) should be < amount ({})",
                    filled, order.amount
                ));
            }
        }
    }

    // cost ≈ filled * average (when all present and filled > 0)
    if let (Some(cost), Some(filled), Some(average)) = (order.cost, order.filled, order.average) {
        if filled > Decimal::ZERO && average > Decimal::ZERO && cost > Decimal::ZERO {
            let calculated = filled * average;
            let diff = (calculated - cost).abs();
            let tolerance = cost * Decimal::new(1, 2); // 1%
            if diff > tolerance {
                errors.push(format!(
                    "cost ({}) does not match filled * average ({}) within 1%",
                    cost, calculated
                ));
            }
        }
    }

    // Average must be > 0 when present
    if let Some(avg) = order.average {
        if avg < Decimal::ZERO {
            errors.push(format!("average ({}) must be >= 0", avg));
        }
    }

    errors
}

/// Validate a market order (filled immediately).
pub fn validate_market_order(order: &Order) -> Vec<String> {
    let mut errors = validate_order(order);

    if order.order_type != OrderType::Market {
        errors.push(format!("expected Market order type, got {:?}", order.order_type));
    }

    // Market orders should typically be filled/closed
    if order.status == OrderStatus::Closed {
        if let Some(filled) = order.filled {
            if filled <= Decimal::ZERO {
                errors.push("closed market order should have filled > 0".to_string());
            }
        }
    }

    // Market orders generally don't have a limit price
    // (some exchanges may set it to the fill price)

    errors
}

/// Validate a stop order (has stop_price or trigger_price set).
pub fn validate_stop_order(order: &Order) -> Vec<String> {
    let mut errors = validate_order(order);

    let has_stop = order.stop_price.is_some()
        || order.trigger_price.is_some()
        || order.stop_loss_price.is_some()
        || order.take_profit_price.is_some();

    if !has_stop {
        errors.push("stop order should have stop_price, trigger_price, stop_loss_price, or take_profit_price set".to_string());
    }

    // Validate stop prices are positive when set
    if let Some(sp) = order.stop_price {
        if sp <= Decimal::ZERO {
            errors.push(format!("stop_price ({}) must be > 0", sp));
        }
    }

    if let Some(tp) = order.trigger_price {
        if tp <= Decimal::ZERO {
            errors.push(format!("trigger_price ({}) must be > 0", tp));
        }
    }

    if let Some(sl) = order.stop_loss_price {
        if sl <= Decimal::ZERO {
            errors.push(format!("stop_loss_price ({}) must be > 0", sl));
        }
    }

    if let Some(tp) = order.take_profit_price {
        if tp <= Decimal::ZERO {
            errors.push(format!("take_profit_price ({}) must be > 0", tp));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccxt::types::common::{OrderSide, OrderType};

    fn valid_order() -> Order {
        Order {
            id: "12345".to_string(),
            client_order_id: None,
            symbol: "BTC/USDT".to_string(),
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            status: OrderStatus::Open,
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            last_trade_timestamp: None,
            price: Some(Decimal::new(50000, 0)),
            average: None,
            amount: Decimal::new(1, 0),
            filled: Some(Decimal::ZERO),
            remaining: Some(Decimal::new(1, 0)),
            cost: Some(Decimal::ZERO),
            fee: None,
            time_in_force: None,
            post_only: None,
            reduce_only: None,
            stop_price: None,
            trigger_price: None,
            stop_loss_price: None,
            take_profit_price: None,
            last_update_timestamp: None,
            trades: None,
            info: None,
        }
    }

    #[test]
    fn test_valid_order_passes() {
        let errors = validate_order(&valid_order());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_filled_plus_remaining_mismatch() {
        let mut o = valid_order();
        o.filled = Some(Decimal::new(3, 1)); // 0.3
        o.remaining = Some(Decimal::new(5, 1)); // 0.5 — doesn't add up to 1.0
        let errors = validate_order(&o);
        assert!(errors.iter().any(|e| e.contains("filled") && e.contains("remaining")));
    }
}
