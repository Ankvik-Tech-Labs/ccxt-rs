use ccxt::types::OrderBook;
use rust_decimal::Decimal;

/// Validate an OrderBook struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_order_book(ob: &OrderBook) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty and contain '/'
    if ob.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }
    if !ob.symbol.contains('/') {
        errors.push(format!("symbol '{}' does not contain '/'", ob.symbol));
    }

    // Timestamp must be positive
    if ob.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", ob.timestamp));
    }

    // Bids must be sorted descending by price
    for i in 1..ob.bids.len() {
        if ob.bids[i].0 > ob.bids[i - 1].0 {
            errors.push(format!(
                "bids not sorted descending: bid[{}] ({}) > bid[{}] ({})",
                i, ob.bids[i].0, i - 1, ob.bids[i - 1].0
            ));
            break;
        }
    }

    // Asks must be sorted ascending by price
    for i in 1..ob.asks.len() {
        if ob.asks[i].0 < ob.asks[i - 1].0 {
            errors.push(format!(
                "asks not sorted ascending: ask[{}] ({}) < ask[{}] ({})",
                i, ob.asks[i].0, i - 1, ob.asks[i - 1].0
            ));
            break;
        }
    }

    // All prices and amounts must be > 0
    for (i, (price, amount)) in ob.bids.iter().enumerate() {
        if *price <= Decimal::ZERO {
            errors.push(format!("bids[{}].price ({}) must be > 0", i, price));
        }
        if *amount <= Decimal::ZERO {
            errors.push(format!("bids[{}].amount ({}) must be > 0", i, amount));
        }
    }

    for (i, (price, amount)) in ob.asks.iter().enumerate() {
        if *price <= Decimal::ZERO {
            errors.push(format!("asks[{}].price ({}) must be > 0", i, price));
        }
        if *amount <= Decimal::ZERO {
            errors.push(format!("asks[{}].amount ({}) must be > 0", i, amount));
        }
    }

    // best_ask > best_bid (positive spread)
    if let (Some(best_bid), Some(best_ask)) = (ob.bids.first(), ob.asks.first()) {
        if best_ask.0 <= best_bid.0 {
            errors.push(format!(
                "negative or zero spread: best_ask ({}) <= best_bid ({})",
                best_ask.0, best_bid.0
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_order_book() -> OrderBook {
        OrderBook {
            symbol: "BTC/USDT".to_string(),
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            nonce: Some(12345),
            bids: vec![
                (Decimal::new(50000, 0), Decimal::new(1, 0)),
                (Decimal::new(49990, 0), Decimal::new(2, 0)),
                (Decimal::new(49980, 0), Decimal::new(3, 0)),
            ],
            asks: vec![
                (Decimal::new(50010, 0), Decimal::new(1, 0)),
                (Decimal::new(50020, 0), Decimal::new(2, 0)),
                (Decimal::new(50030, 0), Decimal::new(3, 0)),
            ],
            info: None,
        }
    }

    #[test]
    fn test_valid_order_book_passes() {
        let errors = validate_order_book(&valid_order_book());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_bids_not_sorted() {
        let mut ob = valid_order_book();
        ob.bids = vec![
            (Decimal::new(49990, 0), Decimal::new(2, 0)),
            (Decimal::new(50000, 0), Decimal::new(1, 0)),
        ];
        let errors = validate_order_book(&ob);
        assert!(errors.iter().any(|e| e.contains("bids not sorted")));
    }

    #[test]
    fn test_negative_spread() {
        let mut ob = valid_order_book();
        ob.asks = vec![(Decimal::new(49999, 0), Decimal::new(1, 0))];
        let errors = validate_order_book(&ob);
        assert!(errors.iter().any(|e| e.contains("negative or zero spread")));
    }
}
