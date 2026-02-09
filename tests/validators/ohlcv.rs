use ccxt::types::OHLCV;
use rust_decimal::Decimal;

/// Validate an OHLCV struct for structural and semantic correctness.
///
/// Returns a list of error messages (empty = valid).
pub fn validate_ohlcv(ohlcv: &OHLCV) -> Vec<String> {
    let mut errors = Vec::new();

    // Timestamp must be positive
    if ohlcv.timestamp <= 0 {
        errors.push(format!("timestamp {} is not positive", ohlcv.timestamp));
    }

    // high >= open, close, low
    if ohlcv.high < ohlcv.open {
        errors.push(format!("high ({}) < open ({})", ohlcv.high, ohlcv.open));
    }
    if ohlcv.high < ohlcv.close {
        errors.push(format!("high ({}) < close ({})", ohlcv.high, ohlcv.close));
    }
    if ohlcv.high < ohlcv.low {
        errors.push(format!("high ({}) < low ({})", ohlcv.high, ohlcv.low));
    }

    // low <= open, close, high
    if ohlcv.low > ohlcv.open {
        errors.push(format!("low ({}) > open ({})", ohlcv.low, ohlcv.open));
    }
    if ohlcv.low > ohlcv.close {
        errors.push(format!("low ({}) > close ({})", ohlcv.low, ohlcv.close));
    }

    // All OHLC values must be > 0
    for (name, val) in [
        ("open", ohlcv.open),
        ("high", ohlcv.high),
        ("low", ohlcv.low),
        ("close", ohlcv.close),
    ] {
        if val <= Decimal::ZERO {
            errors.push(format!("{} ({}) must be > 0", name, val));
        }
    }

    // Volume must be >= 0
    if ohlcv.volume < Decimal::ZERO {
        errors.push(format!("volume ({}) must be >= 0", ohlcv.volume));
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_ohlcv() -> OHLCV {
        OHLCV {
            timestamp: 1700000000000,
            open: Decimal::new(50000, 0),
            high: Decimal::new(51000, 0),
            low: Decimal::new(49000, 0),
            close: Decimal::new(50500, 0),
            volume: Decimal::new(1000, 0),
            info: None,
        }
    }

    #[test]
    fn test_valid_ohlcv_passes() {
        let errors = validate_ohlcv(&valid_ohlcv());
        assert!(errors.is_empty(), "Errors: {:?}", errors);
    }

    #[test]
    fn test_high_below_low() {
        let mut o = valid_ohlcv();
        o.high = Decimal::new(48000, 0);
        let errors = validate_ohlcv(&o);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_negative_volume() {
        let mut o = valid_ohlcv();
        o.volume = Decimal::new(-1, 0);
        let errors = validate_ohlcv(&o);
        assert!(errors.iter().any(|e| e.contains("volume")));
    }
}
