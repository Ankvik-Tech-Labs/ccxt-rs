//! Precise number formatting and comparison utilities
//!
//! This module provides CCXT-style precise number handling for cases where
//! exchanges return numbers as strings to preserve precision.

use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::str::FromStr;

/// Precise number representation
///
/// Used for handling numbers from exchanges that provide them as strings
/// to maintain full precision (avoiding float rounding errors).
#[derive(Debug, Clone)]
pub struct Precise {
    value: Decimal,
}

impl Precise {
    /// Create from string
    pub fn new(s: &str) -> Option<Self> {
        Decimal::from_str(s).ok().map(|value| Self { value })
    }

    /// Create from Decimal
    pub fn from_decimal(value: Decimal) -> Self {
        Self { value }
    }

    /// Get as Decimal
    pub fn as_decimal(&self) -> Decimal {
        self.value
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }

    /// Add two Precise numbers
    pub fn add(&self, other: &Precise) -> Self {
        Self {
            value: self.value + other.value,
        }
    }

    /// Subtract two Precise numbers
    pub fn sub(&self, other: &Precise) -> Self {
        Self {
            value: self.value - other.value,
        }
    }

    /// Multiply two Precise numbers
    pub fn mul(&self, other: &Precise) -> Self {
        Self {
            value: self.value * other.value,
        }
    }

    /// Divide two Precise numbers
    pub fn div(&self, other: &Precise) -> Option<Self> {
        if other.value.is_zero() {
            None
        } else {
            Some(Self {
                value: self.value / other.value,
            })
        }
    }

    /// Compare two Precise numbers
    pub fn cmp(&self, other: &Precise) -> Ordering {
        self.value.cmp(&other.value)
    }

    /// Check if equal to another Precise number
    pub fn eq(&self, other: &Precise) -> bool {
        self.value == other.value
    }

    /// Check if greater than another Precise number
    pub fn gt(&self, other: &Precise) -> bool {
        self.value > other.value
    }

    /// Check if less than another Precise number
    pub fn lt(&self, other: &Precise) -> bool {
        self.value < other.value
    }
}

impl From<Decimal> for Precise {
    fn from(value: Decimal) -> Self {
        Self::from_decimal(value)
    }
}

impl From<Precise> for Decimal {
    fn from(precise: Precise) -> Self {
        precise.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precise_arithmetic() {
        let a = Precise::new("123.456").unwrap();
        let b = Precise::new("78.9").unwrap();

        let sum = a.add(&b);
        assert_eq!(sum.to_string(), "202.356");

        let diff = a.sub(&b);
        assert_eq!(diff.to_string(), "44.556");

        let product = a.mul(&b);
        assert_eq!(product.to_string(), "9740.6784");
    }

    #[test]
    fn test_precise_comparison() {
        let a = Precise::new("123.456").unwrap();
        let b = Precise::new("123.457").unwrap();

        assert!(a.lt(&b));
        assert!(!a.gt(&b));
        assert!(!a.eq(&b));
    }

    #[test]
    fn test_precise_conversion() {
        let decimal = Decimal::from_str("123.456").unwrap();
        let precise = Precise::from_decimal(decimal);

        assert_eq!(precise.as_decimal(), decimal);
        assert_eq!(precise.to_string(), "123.456");
    }
}
