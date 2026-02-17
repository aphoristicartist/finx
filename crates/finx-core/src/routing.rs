use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use crate::adapters::{PolygonAdapter, YahooAdapter};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::{BarSeries, EnvelopeError, ProviderId};

/// Source selection strategy for routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStrategy {
    Auto,
    Priority(Vec<ProviderId>),
    Strict(ProviderId),
}

impl SourceStrategy {
    fn is_strict(&self) -> bool {
        matches!(self, Self::Strict(_))
    }
}

/// Successful routed call.
#[derive(Debug, Clone)]
pub struct RouteSuccess<T> {
    pub data: T,
    pub selected_source: ProviderId,
    pub source_chain: Vec<ProviderId>,
    pub warnings: Vec<String>,
    pub errors: Vec<EnvelopeError>,
    pub latency_ms: u64,
}

/// Failed routed call after exhausting candidates.
#[derive(Debug, Clone)]
pub struct RouteFailure {
    pub source_chain: Vec<ProviderId>,
    pub warnings: Vec<String>,
    pub errors: Vec<EnvelopeError>,
    pub latency_ms: u64,
}

pub type RouteResult<T> = Result<RouteSuccess<T>, RouteFailure>;

/// Source snapshot used by the `sources` CLI command.
#[derive(Debug, Clone, Copy)]
pub struct SourceSnapshot {
    pub id: ProviderId,
    pub capabilities: CapabilitySet,
    pub health: HealthStatus,
}

impl SourceSnapshot {
    pub fn available(self) -> bool {
        self.health.state != HealthState::Unhealthy
    }

    pub fn status_label(self) -> &'static str {
        if !self.health.rate_available {
            return "rate_limited";
        }

        match self.health.state {
            HealthState::Healthy => "healthy",
            HealthState::Degraded => "degraded",
            HealthState::Unhealthy => "unhealthy",
        }
    }
}

/// Adapter registry and routing engine.
pub struct SourceRouter {
    adapters: HashMap<ProviderId, Arc<dyn DataSource>>,
}

type InvokeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, SourceError>> + Send + 'a>>;

impl Default for SourceRouter {
    fn default() -> Self {
        Self::new(vec![
            Arc::new(PolygonAdapter::default()),
            Arc::new(YahooAdapter::default()),
        ])
    }
}

impl SourceRouter {
    pub fn new(adapters: Vec<Arc<dyn DataSource>>) -> Self {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.id(), adapter))
            .collect();
        Self { adapters }
    }

    pub async fn source_chain_for_strategy(
        &self,
        endpoint: Endpoint,
        strategy: &SourceStrategy,
    ) -> Vec<ProviderId> {
        let mut chain = self.plan_sources(endpoint, strategy).await;
        if chain.is_empty() {
            chain = self.sorted_registered_sources();
        }
        chain
    }

    pub async fn snapshot(&self, provider: ProviderId) -> Option<SourceSnapshot> {
        let adapter = self.adapters.get(&provider)?;
        Some(SourceSnapshot {
            id: provider,
            capabilities: adapter.capabilities(),
            health: adapter.health().await,
        })
    }

    pub async fn route_quote(
        &self,
        req: &QuoteRequest,
        strategy: SourceStrategy,
    ) -> RouteResult<QuoteBatch> {
        let req = req.clone();
        self.route_endpoint(Endpoint::Quote, strategy, move |source| {
            source.quote(req.clone())
        })
            .await
    }

    pub async fn route_bars(
        &self,
        req: &BarsRequest,
        strategy: SourceStrategy,
    ) -> RouteResult<BarSeries> {
        let req = req.clone();
        self.route_endpoint(Endpoint::Bars, strategy, move |source| {
            source.bars(req.clone())
        })
            .await
    }

    pub async fn route_fundamentals(
        &self,
        req: &FundamentalsRequest,
        strategy: SourceStrategy,
    ) -> RouteResult<FundamentalsBatch> {
        let req = req.clone();
        self.route_endpoint(Endpoint::Fundamentals, strategy, move |source| {
            source.fundamentals(req.clone())
        })
            .await
    }

    pub async fn route_search(
        &self,
        req: &SearchRequest,
        strategy: SourceStrategy,
    ) -> RouteResult<SearchBatch> {
        let req = req.clone();
        self.route_endpoint(Endpoint::Search, strategy, move |source| {
            source.search(req.clone())
        })
            .await
    }

    async fn route_endpoint<T, F>(
        &self,
        endpoint: Endpoint,
        strategy: SourceStrategy,
        mut invoke: F,
    ) -> RouteResult<T>
    where
        F: for<'a> FnMut(&'a dyn DataSource) -> InvokeFuture<'a, T>,
    {
        let started = Instant::now();
        let planned_chain = self.plan_sources(endpoint, &strategy).await;
        let mut source_chain = Vec::with_capacity(planned_chain.len());
        let mut errors = Vec::new();

        for provider in planned_chain {
            source_chain.push(provider);
            let Some(adapter) = self.adapters.get(&provider) else {
                errors.push(to_envelope_error(
                    provider,
                    SourceError::adapter_not_registered(provider),
                ));
                if strategy.is_strict() {
                    break;
                }
                continue;
            };

            if !adapter.capabilities().supports(endpoint) {
                errors.push(to_envelope_error(
                    provider,
                    SourceError::unsupported_endpoint(endpoint),
                ));
                if strategy.is_strict() {
                    break;
                }
                continue;
            }

            let health = adapter.health().await;
            if health.state == HealthState::Unhealthy {
                errors.push(to_envelope_error(
                    provider,
                    SourceError::unavailable("source health check reported unhealthy"),
                ));
                if strategy.is_strict() {
                    break;
                }
                continue;
            }

            if !health.rate_available {
                errors.push(to_envelope_error(
                    provider,
                    SourceError::rate_limited("source has no rate budget available"),
                ));
                if strategy.is_strict() {
                    break;
                }
                continue;
            }

            match invoke(adapter.as_ref()).await {
                Ok(data) => {
                    let mut warnings = Vec::new();
                    if !errors.is_empty() {
                        warnings.push(format!(
                            "source fallback succeeded with '{}' after {} failed attempt(s)",
                            provider.as_str(),
                            errors.len()
                        ));
                    }

                    return Ok(RouteSuccess {
                        data,
                        selected_source: provider,
                        source_chain,
                        warnings,
                        errors,
                        latency_ms: elapsed_ms(started),
                    });
                }
                Err(error) => {
                    errors.push(to_envelope_error(provider, error));
                    if strategy.is_strict() {
                        break;
                    }
                }
            }
        }

        if source_chain.is_empty() {
            source_chain = self.source_chain_for_strategy(endpoint, &strategy).await;
        }
        if source_chain.is_empty() {
            source_chain = self.sorted_registered_sources();
        }

        if errors.is_empty() {
            errors.push(
                EnvelopeError::new(
                    "source.no_candidate",
                    format!("no source candidates available for endpoint '{endpoint}'"),
                )
                .expect("code/message are non-empty"),
            );
        }

        Err(RouteFailure {
            source_chain,
            warnings: vec![format!("all sources failed for endpoint '{endpoint}'")],
            errors,
            latency_ms: elapsed_ms(started),
        })
    }

    async fn plan_sources(&self, endpoint: Endpoint, strategy: &SourceStrategy) -> Vec<ProviderId> {
        match strategy {
            SourceStrategy::Auto => self.auto_chain(endpoint).await,
            SourceStrategy::Priority(priority) => dedupe_chain(priority),
            SourceStrategy::Strict(provider) => vec![*provider],
        }
    }

    async fn auto_chain(&self, endpoint: Endpoint) -> Vec<ProviderId> {
        let mut scored = Vec::with_capacity(self.adapters.len());
        for (provider, source) in &self.adapters {
            let capabilities = source.capabilities();
            let health = source.health().await;
            let supports_endpoint = capabilities.supports(endpoint);

            let endpoint_score = if supports_endpoint { 1_000 } else { 0 };
            let health_score = match health.state {
                HealthState::Healthy => 250,
                HealthState::Degraded => 100,
                HealthState::Unhealthy => 0,
            };
            let rate_score = if health.rate_available { 150 } else { 0 };
            let total_score = endpoint_score + health_score + rate_score + i32::from(health.score);

            scored.push((*provider, total_score));
        }

        scored.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.0.as_str().cmp(right.0.as_str()))
        });

        scored.into_iter().map(|(provider, _)| provider).collect()
    }

    fn sorted_registered_sources(&self) -> Vec<ProviderId> {
        let mut providers = self.adapters.keys().copied().collect::<Vec<_>>();
        providers.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        providers
    }
}

fn dedupe_chain(chain: &[ProviderId]) -> Vec<ProviderId> {
    let mut seen = HashSet::new();
    let mut output = Vec::with_capacity(chain.len());

    for provider in chain {
        if seen.insert(*provider) {
            output.push(*provider);
        }
    }

    output
}

fn to_envelope_error(provider: ProviderId, error: SourceError) -> EnvelopeError {
    EnvelopeError::new(error.code(), error.message())
        .expect("code/message are non-empty")
        .with_source(provider)
        .with_retryable(error.retryable())
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;
    use std::future::Future;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    #[test]
    fn auto_prefers_polygon_for_quote_when_available() {
        let router = SourceRouter::default();
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let result = block_on(router.route_quote(&request, SourceStrategy::Auto))
            .expect("route should succeed");

        assert_eq!(result.selected_source, ProviderId::Polygon);
        assert_eq!(result.source_chain, vec![ProviderId::Polygon]);
    }

    #[test]
    fn auto_falls_back_to_yahoo_after_polygon_rate_limit() {
        let router = SourceRouter::default();
        let request = QuoteRequest::new(vec![
            Symbol::parse("AAPL").expect("valid symbol"),
            Symbol::parse("MSFT").expect("valid symbol"),
            Symbol::parse("NVDA").expect("valid symbol"),
            Symbol::parse("TSLA").expect("valid symbol"),
        ])
        .expect("valid request");

        let result = block_on(router.route_quote(&request, SourceStrategy::Auto))
            .expect("route should succeed with fallback");

        assert_eq!(result.selected_source, ProviderId::Yahoo);
        assert_eq!(
            result.source_chain,
            vec![ProviderId::Polygon, ProviderId::Yahoo]
        );
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].source, Some(ProviderId::Polygon));
    }

    #[test]
    fn strict_source_does_not_fallback() {
        let router = SourceRouter::default();
        let request = QuoteRequest::new(vec![
            Symbol::parse("AAPL").expect("valid symbol"),
            Symbol::parse("MSFT").expect("valid symbol"),
            Symbol::parse("NVDA").expect("valid symbol"),
            Symbol::parse("TSLA").expect("valid symbol"),
        ])
        .expect("valid request");

        let result = block_on(router.route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon)));

        let failure = result.expect_err("strict route should fail");
        assert_eq!(failure.source_chain, vec![ProviderId::Polygon]);
        assert_eq!(failure.errors.len(), 1);
        assert_eq!(failure.errors[0].source, Some(ProviderId::Polygon));
    }

    fn block_on<F>(future: F) -> F::Output
    where
        F: Future,
    {
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn noop_waker() -> Waker {
        // SAFETY: The vtable functions never dereference the data pointer and are no-op operations.
        unsafe { Waker::from_raw(noop_raw_waker()) }
    }

    fn noop_raw_waker() -> RawWaker {
        RawWaker::new(std::ptr::null(), &NOOP_RAW_WAKER_VTABLE)
    }

    unsafe fn noop_raw_waker_clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
    }

    unsafe fn noop_raw_waker_wake(_: *const ()) {}

    unsafe fn noop_raw_waker_wake_by_ref(_: *const ()) {}

    unsafe fn noop_raw_waker_drop(_: *const ()) {}

    static NOOP_RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        noop_raw_waker_clone,
        noop_raw_waker_wake,
        noop_raw_waker_wake_by_ref,
        noop_raw_waker_drop,
    );
}
