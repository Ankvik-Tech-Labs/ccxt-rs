//! Orderbook recovery manager for handling out-of-sync states.
//!
//! This module provides automatic recovery mechanisms for orderbook streams
//! when checksum validation fails, indicating the local state is out of sync
//! with the exchange. Recovery is performed by re-subscribing to the orderbook
//! stream, which forces the exchange to send a fresh snapshot.
//!
//! ## Recovery Strategy
//!
//! When a checksum mismatch is detected:
//! 1. Record the failure and calculate exponential backoff delay
//! 2. Wait for the backoff period (1s, 2s, 4s, 8s, ..., max 30s)
//! 3. Unsubscribe from the orderbook stream
//! 4. Re-subscribe to trigger a new snapshot from the exchange
//! 5. If recovery succeeds (checksum validates), reset the failure counter
//! 6. If max attempts reached, stop recovery and log error
//!
//! ## Configuration
//!
//! Recovery behavior is controlled via `WsConfig`:
//! - `auto_recovery_enabled`: Enable/disable automatic recovery (default: true)
//! - `max_recovery_attempts`: Maximum retry count before giving up (default: 5, 0 = unlimited)
//!
//! ## Example
//!
//! ```rust
//! use ccxt_rs::base::orderbook_recovery::OrderbookRecovery;
//!
//! let mut recovery = OrderbookRecovery::new(5); // max 5 attempts
//!
//! // On checksum failure:
//! if let Some(delay) = recovery.record_failure() {
//!     tokio::time::sleep(delay).await;
//!     // trigger re-subscription...
//! } else {
//!     // max attempts reached, stop recovery
//! }
//!
//! // On successful validation:
//! recovery.reset();
//! ```

use std::time::Duration;
use tokio::time::Instant;

/// Manages recovery state for a single orderbook subscription.
///
/// Tracks failure count, last recovery timestamp, and computes exponential
/// backoff delays for retry attempts.
#[derive(Debug, Clone)]
pub struct OrderbookRecovery {
    /// Number of consecutive checksum failures
    failure_count: u32,
    /// Timestamp of last recovery attempt
    last_recovery: Option<Instant>,
    /// Maximum number of recovery attempts (0 = unlimited)
    max_attempts: u32,
}

impl OrderbookRecovery {
    /// Creates a new recovery manager.
    ///
    /// # Arguments
    /// * `max_attempts` - Maximum number of recovery attempts (0 = unlimited)
    ///
    /// # Returns
    /// A new `OrderbookRecovery` with zero failure count.
    pub fn new(max_attempts: u32) -> Self {
        Self {
            failure_count: 0,
            last_recovery: None,
            max_attempts,
        }
    }

    /// Records a checksum failure and returns the backoff delay if recovery should be attempted.
    ///
    /// Increments the failure counter and calculates exponential backoff delay.
    /// If max attempts is reached, returns `None` to signal recovery should stop.
    ///
    /// # Returns
    /// - `Some(Duration)` - Delay before retrying recovery
    /// - `None` - Max attempts reached, stop recovery
    pub fn record_failure(&mut self) -> Option<Duration> {
        self.failure_count += 1;
        self.last_recovery = Some(Instant::now());

        // Check if max attempts reached (0 = unlimited)
        if self.max_attempts > 0 && self.failure_count > self.max_attempts {
            return None;
        }

        Some(self.next_delay())
    }

    /// Resets the recovery state after a successful checksum validation.
    ///
    /// Clears the failure counter and last recovery timestamp.
    pub fn reset(&mut self) {
        self.failure_count = 0;
        self.last_recovery = None;
    }

    /// Returns the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Returns the timestamp of the last recovery attempt.
    pub fn last_recovery(&self) -> Option<Instant> {
        self.last_recovery
    }

    /// Calculates exponential backoff delay based on failure count.
    ///
    /// Formula: min(2^(failure_count - 1) seconds, 30 seconds)
    /// - Attempt 1: 1s
    /// - Attempt 2: 2s
    /// - Attempt 3: 4s
    /// - Attempt 4: 8s
    /// - Attempt 5: 16s
    /// - Attempt 6+: 30s (capped)
    fn next_delay(&self) -> Duration {
        const MAX_DELAY_SECS: u64 = 30;

        // Calculate 2^(n-1) seconds, capped at 30s
        let delay_secs = if self.failure_count == 0 {
            1
        } else {
            let exponent = self.failure_count.saturating_sub(1);
            // Use bit shift: 1 << n is equivalent to 2^n
            // Cap exponent at 5 to avoid overflow (2^5 = 32 > 30)
            let capped_exp = exponent.min(5);
            let exponential = 1u64 << capped_exp;
            exponential.min(MAX_DELAY_SECS)
        };

        Duration::from_secs(delay_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_recovery_manager() {
        let recovery = OrderbookRecovery::new(5);
        assert_eq!(recovery.failure_count(), 0);
        assert!(recovery.last_recovery().is_none());
    }

    #[test]
    fn test_exponential_backoff_delays() {
        let mut recovery = OrderbookRecovery::new(0); // unlimited

        // First failure: 1s
        let delay1 = recovery.record_failure().unwrap();
        assert_eq!(delay1, Duration::from_secs(1));
        assert_eq!(recovery.failure_count(), 1);

        // Second failure: 2s
        let delay2 = recovery.record_failure().unwrap();
        assert_eq!(delay2, Duration::from_secs(2));
        assert_eq!(recovery.failure_count(), 2);

        // Third failure: 4s
        let delay3 = recovery.record_failure().unwrap();
        assert_eq!(delay3, Duration::from_secs(4));
        assert_eq!(recovery.failure_count(), 3);

        // Fourth failure: 8s
        let delay4 = recovery.record_failure().unwrap();
        assert_eq!(delay4, Duration::from_secs(8));
        assert_eq!(recovery.failure_count(), 4);

        // Fifth failure: 16s
        let delay5 = recovery.record_failure().unwrap();
        assert_eq!(delay5, Duration::from_secs(16));
        assert_eq!(recovery.failure_count(), 5);

        // Sixth failure: 30s (capped)
        let delay6 = recovery.record_failure().unwrap();
        assert_eq!(delay6, Duration::from_secs(30));
        assert_eq!(recovery.failure_count(), 6);

        // Seventh failure: 30s (still capped)
        let delay7 = recovery.record_failure().unwrap();
        assert_eq!(delay7, Duration::from_secs(30));
        assert_eq!(recovery.failure_count(), 7);
    }

    #[test]
    fn test_max_attempts_enforcement() {
        let mut recovery = OrderbookRecovery::new(3);

        // Attempts 1-3: should return delays
        assert!(recovery.record_failure().is_some());
        assert_eq!(recovery.failure_count(), 1);

        assert!(recovery.record_failure().is_some());
        assert_eq!(recovery.failure_count(), 2);

        assert!(recovery.record_failure().is_some());
        assert_eq!(recovery.failure_count(), 3);

        // Attempt 4: should return None (max reached)
        assert!(recovery.record_failure().is_none());
        assert_eq!(recovery.failure_count(), 4);

        // Further attempts: still None
        assert!(recovery.record_failure().is_none());
        assert_eq!(recovery.failure_count(), 5);
    }

    #[test]
    fn test_unlimited_attempts() {
        let mut recovery = OrderbookRecovery::new(0); // unlimited

        // Should never return None
        for i in 1..=100 {
            assert!(recovery.record_failure().is_some());
            assert_eq!(recovery.failure_count(), i);
        }
    }

    #[test]
    fn test_reset_on_success() {
        let mut recovery = OrderbookRecovery::new(5);

        // Record some failures
        recovery.record_failure();
        recovery.record_failure();
        recovery.record_failure();
        assert_eq!(recovery.failure_count(), 3);
        assert!(recovery.last_recovery().is_some());

        // Reset after success
        recovery.reset();
        assert_eq!(recovery.failure_count(), 0);
        assert!(recovery.last_recovery().is_none());

        // Next failure should start from 1s again
        let delay = recovery.record_failure().unwrap();
        assert_eq!(delay, Duration::from_secs(1));
        assert_eq!(recovery.failure_count(), 1);
    }

    #[test]
    fn test_last_recovery_timestamp() {
        let mut recovery = OrderbookRecovery::new(5);

        // No recovery yet
        assert!(recovery.last_recovery().is_none());

        // Record failure
        let before = Instant::now();
        recovery.record_failure();
        let after = Instant::now();

        // Timestamp should be set and within reasonable bounds
        let timestamp = recovery.last_recovery().unwrap();
        assert!(timestamp >= before);
        assert!(timestamp <= after);
    }

    #[test]
    fn test_zero_failure_count_edge_case() {
        let recovery = OrderbookRecovery::new(5);
        // Even with 0 failures, next_delay should return 1s
        assert_eq!(recovery.next_delay(), Duration::from_secs(1));
    }
}
