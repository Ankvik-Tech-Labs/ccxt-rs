use ccxt::types::Ticker;
use rust_decimal::Decimal;

/// Validate a Ticker struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_ticker(ticker: &Ticker) -> Vec<String> {
    let mut errors = Vec::new();

    // Symbol must be non-empty
    if ticker.symbol.is_empty() {
        errors.push("symbol is empty".to_string());
    }

    // Symbol should contain '/'
    if !ticker.symbol.contains('/') {
        errors.push(format!("symbol '{}' does not contain '/'", ticker.symbol));
    }

    // Timestamp must be positive (milliseconds)
    if ticker.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", ticker.timestamp));
    }

    // datetime must be non-empty
    if ticker.datetime.is_empty() {
        errors.push("datetime is empty".to_string());
    }

    // high >= low
    if let (Some(high), Some(low)) = (ticker.high, ticker.low) {
        if high < low {
            errors.push(format!("high ({}) < low ({})", high, low));
        }
    }

    // ask >= bid (positive spread)
    if let (Some(ask), Some(bid)) = (ticker.ask, ticker.bid) {
        if ask < bid {
            errors.push(format!("ask ({}) < bid ({})", ask, bid));
        }
    }

    // Prices must be positive when present
    for (name, val) in [
        ("bid", ticker.bid),
        ("ask", ticker.ask),
        ("high", ticker.high),
        ("low", ticker.low),
        ("last", ticker.last),
        ("open", ticker.open),
        ("close", ticker.close),
    ] {
        if let Some(v) = val {
            if v <= Decimal::ZERO {
                errors.push(format!("{} ({}) must be > 0", name, v));
            }
        }
    }

    // Volumes must be non-negative when present
    for (name, val) in [
        ("baseVolume", ticker.base_volume),
        ("quoteVolume", ticker.quote_volume),
    ] {
        if let Some(v) = val {
            if v < Decimal::ZERO {
                errors.push(format!("{} ({}) must be >= 0", name, v));
            }
        }
    }

    // baseVolume * vwap ≈ quoteVolume (1% tolerance)
    if let (Some(base_vol), Some(vwap), Some(quote_vol)) =
        (ticker.base_volume, ticker.vwap, ticker.quote_volume)
    {
        if base_vol > Decimal::ZERO && vwap > Decimal::ZERO && quote_vol > Decimal::ZERO {
            let calculated = base_vol * vwap;
            let diff = (calculated - quote_vol).abs();
            let tolerance = quote_vol * Decimal::new(1, 2); // 1%
            if diff > tolerance {
                errors.push(format!(
                    "baseVolume * vwap ({}) does not approximate quoteVolume ({}) within 1%",
                    calculated, quote_vol
                ));
            }
        }
    }

    // percentage should be in reasonable bounds (-100% to +10000%)
    if let Some(pct) = ticker.percentage {
        let min = Decimal::new(-100, 0);
        let max = Decimal::new(10000, 0);
        if pct < min || pct > max {
            errors.push(format!(
                "percentage ({}) is outside reasonable range [-100, 10000]",
                pct
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_ticker() -> Ticker {
        Ticker {
            symbol: "BTC/USDT".to_string(),
            timestamp: 1700000000000,
            datetime: "2023-11-14T22:13:20.000Z".to_string(),
            high: Some(Decimal::new(51000, 0)),
            low: Some(Decimal::new(49000, 0)),
            bid: Some(Decimal::new(50000, 0)),
            bid_volume: Some(Decimal::new(10, 0)),
            ask: Some(Decimal::new(50100, 0)),
            ask_volume: Some(Decimal::new(5, 0)),
            vwap: Some(Decimal::new(50000, 0)),
            open: Some(Decimal::new(49500, 0)),
            close: Some(Decimal::new(50050, 0)),
            last: Some(Decimal::new(50050, 0)),
            previous_close: None,
            change: Some(Decimal::new(550, 0)),
            percentage: Some(Decimal::new(111, 2)), // 1.11%
            average: Some(Decimal::new(50000, 0)),
            base_volume: Some(Decimal::new(1000, 0)),
            quote_volume: Some(Decimal::new(50000000, 0)),
            index_price: None,
            mark_price: None,
            info: None,
        }
    }

    #[test]
    fn test_valid_ticker_passes() {
        let errors = validate_ticker(&valid_ticker());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_invalid_high_low() {
        let mut t = valid_ticker();
        t.high = Some(Decimal::new(49000, 0));
        t.low = Some(Decimal::new(51000, 0));
        let errors = validate_ticker(&t);
        assert!(errors.iter().any(|e| e.contains("high") && e.contains("low")));
    }

    #[test]
    fn test_negative_bid() {
        let mut t = valid_ticker();
        t.bid = Some(Decimal::new(-1, 0));
        let errors = validate_ticker(&t);
        assert!(errors.iter().any(|e| e.contains("bid")));
    }
}
