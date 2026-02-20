use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Runtime circuit state for source adapter upstream calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker thresholds and timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub open_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            open_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
struct CircuitInner {
    state: CircuitState,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
}

impl Default for CircuitInner {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            opened_at: None,
        }
    }
}

/// Thread-safe circuit breaker for adapter network requests.
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Mutex<CircuitInner>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(CircuitInner::default()),
        }
    }

    pub fn allow_request(&self) -> bool {
        let mut inner = self
            .inner
            .lock()
            .expect("circuit breaker lock is not poisoned");
        match inner.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                let can_probe = inner
                    .opened_at
                    .map(|opened_at| opened_at.elapsed() >= self.config.open_timeout)
                    .unwrap_or(false);

                if can_probe {
                    inner.state = CircuitState::HalfOpen;
                    inner.opened_at = None;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn record_success(&self) {
        let mut inner = self
            .inner
            .lock()
            .expect("circuit breaker lock is not poisoned");
        inner.state = CircuitState::Closed;
        inner.consecutive_failures = 0;
        inner.opened_at = None;
    }

    pub fn record_failure(&self) {
        let mut inner = self
            .inner
            .lock()
            .expect("circuit breaker lock is not poisoned");
        inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);

        if inner.state == CircuitState::HalfOpen
            || inner.consecutive_failures >= self.config.failure_threshold
        {
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
        }
    }

    pub fn state(&self) -> CircuitState {
        let inner = self
            .inner
            .lock()
            .expect("circuit breaker lock is not poisoned");
        inner.state
    }

    pub fn consecutive_failures(&self) -> u32 {
        let inner = self
            .inner
            .lock()
            .expect("circuit breaker lock is not poisoned");
        inner.consecutive_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_failures() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            open_timeout: Duration::from_millis(10),
        });

        assert_eq!(breaker.state(), CircuitState::Closed);
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.allow_request());
    }

    #[test]
    fn transitions_to_half_open_after_timeout_then_closes_on_success() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            open_timeout: Duration::from_millis(1),
        });

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(2));
        assert!(breaker.allow_request());
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.consecutive_failures(), 0);
    }
}
