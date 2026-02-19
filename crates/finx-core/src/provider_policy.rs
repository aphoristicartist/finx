use std::time::Duration;

use crate::ProviderId;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderPolicy {
    pub provider_id: ProviderId,
    pub max_concurrency: usize,
    pub quota_window: Duration,
    pub quota_limit: u32,
    pub retry_backoff: BackoffPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackoffPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub max_retries: u32,
}

impl ProviderPolicy {
    pub fn alphavantage_default() -> Self {
        Self {
            provider_id: ProviderId::Alphavantage,
            max_concurrency: 1,
            quota_window: Duration::from_secs(60),
            quota_limit: 5,
            retry_backoff: BackoffPolicy {
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(60),
                multiplier: 2.0,
                max_retries: 3,
            },
        }
    }

    pub fn alpaca_default() -> Self {
        Self {
            provider_id: ProviderId::Alpaca,
            max_concurrency: 10,
            quota_window: Duration::from_secs(60),
            quota_limit: 100,
            retry_backoff: BackoffPolicy {
                initial_delay: Duration::from_millis(500),
                max_delay: Duration::from_secs(30),
                multiplier: 2.0,
                max_retries: 3,
            },
        }
    }

    pub fn default_for(provider_id: ProviderId) -> Option<Self> {
        match provider_id {
            ProviderId::Alphavantage => Some(Self::alphavantage_default()),
            ProviderId::Alpaca => Some(Self::alpaca_default()),
            ProviderId::Yahoo | ProviderId::Polygon => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphavantage_policy_matches_free_tier() {
        let policy = ProviderPolicy::alphavantage_default();

        assert_eq!(policy.provider_id, ProviderId::Alphavantage);
        assert_eq!(policy.max_concurrency, 1);
        assert_eq!(policy.quota_window, Duration::from_secs(60));
        assert_eq!(policy.quota_limit, 5);
    }

    #[test]
    fn alpaca_policy_matches_default_limits() {
        let policy = ProviderPolicy::alpaca_default();

        assert_eq!(policy.provider_id, ProviderId::Alpaca);
        assert_eq!(policy.max_concurrency, 10);
        assert_eq!(policy.quota_window, Duration::from_secs(60));
        assert_eq!(policy.quota_limit, 100);
    }
}
