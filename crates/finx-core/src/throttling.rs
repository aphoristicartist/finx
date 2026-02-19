use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::state::direct::NotKeyed;
use governor::state::InMemoryState;
use governor::{Quota, RateLimiter};

use crate::provider_policy::{BackoffPolicy, ProviderPolicy};

/// In-memory throttling queue that tracks pending requests and computes retry delays.
#[derive(Clone)]
pub struct ThrottlingQueue {
    limiter: Arc<DirectRateLimiter>,
    pending: Arc<Mutex<VecDeque<PendingRequest>>>,
    retry_backoff: BackoffPolicy,
}

type DirectRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Debug, Clone, Copy)]
struct PendingRequest {
    retry_count: u32,
}

impl ThrottlingQueue {
    pub fn new(quota_window: Duration, quota_limit: u32, retry_backoff: BackoffPolicy) -> Self {
        let quota = quota_from_window(quota_window, quota_limit);
        Self {
            limiter: Arc::new(RateLimiter::direct(quota)),
            pending: Arc::new(Mutex::new(VecDeque::new())),
            retry_backoff,
        }
    }

    pub fn from_policy(policy: &ProviderPolicy) -> Self {
        Self::new(
            policy.quota_window,
            policy.quota_limit,
            policy.retry_backoff.clone(),
        )
    }

    /// Tries to acquire rate budget. When budget is unavailable the request is buffered
    /// and the recommended backoff delay is returned.
    pub fn acquire(&self) -> Result<(), Duration> {
        if self.limiter.check().is_ok() {
            return Ok(());
        }

        let mut pending = self
            .pending
            .lock()
            .expect("throttling pending queue should not be poisoned");
        pending.push_back(PendingRequest { retry_count: 0 });

        Err(self.retry_delay(0).unwrap_or(self.retry_backoff.max_delay))
    }

    /// Increments retry count for the oldest buffered request and returns its next delay.
    pub fn register_retry(&self) -> Option<Duration> {
        let mut pending = self
            .pending
            .lock()
            .expect("throttling pending queue should not be poisoned");
        let request = pending.front_mut()?;
        request.retry_count = request.retry_count.saturating_add(1);
        self.retry_delay(request.retry_count)
    }

    /// Removes a request from the pending queue when it was successfully retried.
    pub fn complete_one(&self) {
        let mut pending = self
            .pending
            .lock()
            .expect("throttling pending queue should not be poisoned");
        let _ = pending.pop_front();
    }

    pub fn pending_len(&self) -> usize {
        self.pending
            .lock()
            .expect("throttling pending queue should not be poisoned")
            .len()
    }

    pub fn retry_delay(&self, retry_count: u32) -> Option<Duration> {
        if retry_count > self.retry_backoff.max_retries {
            return None;
        }

        let scale = self.retry_backoff.multiplier.powf(f64::from(retry_count));
        let seconds = self.retry_backoff.initial_delay.as_secs_f64() * scale;
        let capped_seconds = seconds.min(self.retry_backoff.max_delay.as_secs_f64());
        Some(Duration::from_secs_f64(capped_seconds))
    }
}

fn quota_from_window(quota_window: Duration, quota_limit: u32) -> Quota {
    let safe_limit = quota_limit.max(1);
    let burst = NonZeroU32::new(safe_limit).expect("safe limit must be non-zero");

    let seconds_per_cell = (quota_window.as_secs_f64() / f64::from(safe_limit)).max(0.001);
    let period = Duration::from_secs_f64(seconds_per_cell);

    Quota::with_period(period)
        .expect("period is always greater than zero")
        .allow_burst(burst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_when_rate_limit_is_exceeded() {
        let queue = ThrottlingQueue::new(
            Duration::from_secs(60),
            2,
            BackoffPolicy {
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(10),
                multiplier: 2.0,
                max_retries: 3,
            },
        );

        assert!(queue.acquire().is_ok());
        assert!(queue.acquire().is_ok());

        let retry_delay = queue.acquire().expect_err("third request should be queued");
        assert_eq!(retry_delay, Duration::from_secs(1));
        assert_eq!(queue.pending_len(), 1);
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        let queue = ThrottlingQueue::new(
            Duration::from_secs(60),
            1,
            BackoffPolicy {
                initial_delay: Duration::from_secs(2),
                max_delay: Duration::from_secs(10),
                multiplier: 2.0,
                max_retries: 3,
            },
        );

        assert_eq!(queue.retry_delay(0), Some(Duration::from_secs(2)));
        assert_eq!(queue.retry_delay(1), Some(Duration::from_secs(4)));
        assert_eq!(queue.retry_delay(2), Some(Duration::from_secs(8)));
        assert_eq!(queue.retry_delay(3), Some(Duration::from_secs(10)));
        assert_eq!(queue.retry_delay(4), None);
    }
}
