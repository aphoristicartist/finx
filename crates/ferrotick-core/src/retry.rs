//! Retry logic with exponential backoff and jitter.

use std::time::Duration;

/// Backoff strategy for retrying failed requests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backoff {
    /// Uses a fixed delay between retries.
    Fixed {
        /// Delay between retries.
        delay: Duration,
    },
    /// Uses an exponential delay between retries.
    ///
    /// The delay is calculated as `base * (factor ^ attempt)`.
    Exponential {
        /// The initial backoff duration.
        base: Duration,
        /// The multiplicative factor for each subsequent retry.
        factor: f64,
        /// The maximum duration to wait between retries.
        max: Duration,
        /// Whether to apply random jitter (+/- 50%) to the delay.
        jitter: bool,
    },
}

impl Default for Backoff {
    fn default() -> Self {
        Self::Exponential {
            base: Duration::from_millis(200),
            factor: 2.0,
            max: Duration::from_secs(3),
            jitter: true,
        }
    }
}

impl Backoff {
    /// Calculate the delay for a given retry attempt.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The retry attempt number (0-based)
    ///
    /// # Returns
    ///
    /// The calculated delay duration
    pub fn delay(self, attempt: u32) -> Duration {
        match self {
            Self::Fixed { delay } => delay,
            Self::Exponential { base, factor, max, jitter } => {
                let scale = factor.powi(attempt as i32);
                let seconds = base.as_secs_f64() * scale;
                let capped_seconds = seconds.min(max.as_secs_f64());

                let mut delay = Duration::from_secs_f64(capped_seconds);

                // Apply jitter: +/- 50% of the delay
                if jitter {
                    let jitter_ms = (delay.as_millis() as f64 * 0.5) as u64;
                    let random_offset = fastrand::u64(0..=(jitter_ms * 2));
                    let total_ms = delay.as_millis() as i64 + (random_offset as i64 - jitter_ms as i64);
                    delay = Duration::from_millis(total_ms.max(0) as u64);
                }

                delay
            }
        }
    }
}

/// Configuration for the automatic retry mechanism.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Enables or disables the retry mechanism.
    pub enabled: bool,
    /// The maximum number of retries to attempt.
    /// Total attempts = `max_retries + 1`.
    pub max_retries: u32,
    /// The backoff strategy to use between retries.
    pub backoff: Backoff,
    /// A list of HTTP status codes that should trigger a retry.
    pub retry_on_status: Vec<u16>,
    /// Whether to retry on request timeouts.
    pub retry_on_timeout: bool,
    /// Whether to retry on connection errors.
    pub retry_on_connect: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 4,
            backoff: Backoff::default(),
            retry_on_status: vec![408, 429, 500, 502, 503, 504],
            retry_on_timeout: true,
            retry_on_connect: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration with exponential backoff.
    pub fn exponential(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Self::default()
        }
    }

    /// Create a new retry configuration with fixed backoff.
    pub fn fixed(delay: Duration, max_retries: u32) -> Self {
        Self {
            max_retries,
            backoff: Backoff::Fixed { delay },
            ..Self::default()
        }
    }

    /// Disable retries.
    pub fn no_retry() -> Self {
        Self {
            enabled: false,
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Check if a given HTTP status code should trigger a retry.
    pub fn should_retry_status(&self, status: u16) -> bool {
        self.retry_on_status.contains(&status)
    }

    /// Calculate the delay for a given retry attempt.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        self.backoff.delay(attempt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_backoff() {
        let backoff = Backoff::Fixed {
            delay: Duration::from_millis(100),
        };

        assert_eq!(backoff.delay(0), Duration::from_millis(100));
        assert_eq!(backoff.delay(1), Duration::from_millis(100));
        assert_eq!(backoff.delay(10), Duration::from_millis(100));
    }

    #[test]
    fn test_exponential_backoff() {
        let backoff = Backoff::Exponential {
            base: Duration::from_millis(100),
            factor: 2.0,
            max: Duration::from_secs(1),
            jitter: false,
        };

        assert_eq!(backoff.delay(0), Duration::from_millis(100));
        assert_eq!(backoff.delay(1), Duration::from_millis(200));
        assert_eq!(backoff.delay(2), Duration::from_millis(400));
        assert_eq!(backoff.delay(3), Duration::from_millis(800));
        assert_eq!(backoff.delay(4), Duration::from_secs(1)); // capped
    }

    #[test]
    fn test_exponential_backoff_with_jitter() {
        let backoff = Backoff::Exponential {
            base: Duration::from_millis(100),
            factor: 2.0,
            max: Duration::from_secs(1),
            jitter: true,
        };

        // With jitter, delay should be within +/- 50%
        // Run multiple times to account for randomness
        for _ in 0..10 {
            for attempt in 0..5 {
                let delay = backoff.delay(attempt);
                let expected_base = 100.0 * 2_f64.powi(attempt as i32);
                let expected_capped = expected_base.min(1000.0);
                let delay_ms = delay.as_millis() as f64;

                // Allow for jitter: should be within ~50-150% of capped base
                // Use 0.49 and 1.51 to account for integer rounding errors
                assert!(delay_ms >= expected_capped * 0.49, "attempt={}, delay_ms={}, expected_capped={}", attempt, delay_ms, expected_capped);
                assert!(delay_ms <= expected_capped * 1.51, "attempt={}, delay_ms={}, expected_capped={}", attempt, delay_ms, expected_capped);
            }
        }
    }

    #[test]
    fn test_default_retry_config() {
        let config = RetryConfig::default();

        assert!(config.enabled);
        assert_eq!(config.max_retries, 4);
        assert!(config.should_retry_status(408));
        assert!(config.should_retry_status(429));
        assert!(config.should_retry_status(500));
        assert!(config.should_retry_status(502));
        assert!(config.should_retry_status(503));
        assert!(config.should_retry_status(504));
        assert!(!config.should_retry_status(400));
        assert!(!config.should_retry_status(401));
        assert!(config.retry_on_timeout);
        assert!(config.retry_on_connect);
    }

    #[test]
    fn test_retry_config_exponential() {
        let config = RetryConfig {
            backoff: Backoff::Exponential {
                base: Duration::from_millis(200),
                factor: 2.0,
                max: Duration::from_secs(3),
                jitter: false,
            },
            ..RetryConfig::exponential(3)
        };

        assert!(config.enabled);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.delay_for_attempt(0).as_millis(), 200);
        assert_eq!(config.delay_for_attempt(1).as_millis(), 400);
        assert_eq!(config.delay_for_attempt(2).as_millis(), 800);
    }

    #[test]
    fn test_retry_config_fixed() {
        let config = RetryConfig::fixed(Duration::from_millis(500), 2);

        assert!(config.enabled);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(500));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(500));
    }

    #[test]
    fn test_retry_config_no_retry() {
        let config = RetryConfig::no_retry();

        assert!(!config.enabled);
        assert_eq!(config.max_retries, 0);
    }
}
