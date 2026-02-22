use std::collections::{HashMap, HashSet};
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use crate::adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpAuth, ReqwestHttpClient};
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
            Arc::new(AlpacaAdapter::default()),
            Arc::new(AlphaVantageAdapter::default()),
            Arc::new(YahooAdapter::default()),
        ])
    }
}

/// Builder for creating a SourceRouter with real HTTP clients.
///
/// This builder reads API keys from environment variables and creates
/// adapters with real HTTP clients for production use.
///
/// # Environment Variables
///
/// | Provider | Primary Env Var | Fallback Env Var |
/// |----------|----------------|------------------|
/// | Polygon | `FERROTICK_POLYGON_API_KEY` | `POLYGON_API_KEY` |
/// | Alpaca | `FERROTICK_ALPACA_API_KEY` | `ALPACA_API_KEY` |
/// | Alpaca Secret | `FERROTICK_ALPACA_SECRET_KEY` | `ALPACA_SECRET_KEY` |
/// | Alpha Vantage | `FERROTICK_ALPHAVANTAGE_API_KEY` | `ALPHAVANTAGE_API_KEY` |
/// | Yahoo | (no key required) | - |
///
/// # Example
///
/// ```rust,ignore
/// use ferrotick_core::SourceRouterBuilder;
///
/// // Build with real HTTP clients (reads from env vars)
/// let router = SourceRouterBuilder::new()
///     .with_real_clients()
///     .build();
///
/// // Or explicitly use mock mode
/// let mock_router = SourceRouterBuilder::new()
///     .with_mock_mode()
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct SourceRouterBuilder {
    use_mock: bool,
    polygon_api_key: Option<String>,
    alpaca_api_key: Option<String>,
    alpaca_secret_key: Option<String>,
    alphavantage_api_key: Option<String>,
    enable_polygon: bool,
    enable_alpaca: bool,
    enable_alphavantage: bool,
    enable_yahoo: bool,
}

impl SourceRouterBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            use_mock: false,
            polygon_api_key: None,
            alpaca_api_key: None,
            alpaca_secret_key: None,
            alphavantage_api_key: None,
            enable_polygon: true,
            enable_alpaca: true,
            enable_alphavantage: true,
            enable_yahoo: true,
        }
    }

    /// Enable mock mode - all adapters use NoopHttpClient with deterministic data.
    ///
    /// This is useful for testing without making real API calls.
    pub fn with_mock_mode(mut self) -> Self {
        self.use_mock = true;
        self
    }

    /// Configure adapters to use real HTTP clients.
    ///
    /// Reads API keys from environment variables. Providers without API keys
    /// will use mock mode for that provider only (except Yahoo which doesn't need a key).
    pub fn with_real_clients(mut self) -> Self {
        self.use_mock = false;
        self.polygon_api_key = env::var("FERROTICK_POLYGON_API_KEY")
            .or_else(|_| env::var("POLYGON_API_KEY"))
            .ok();
        self.alpaca_api_key = env::var("FERROTICK_ALPACA_API_KEY")
            .or_else(|_| env::var("ALPACA_API_KEY"))
            .ok();
        self.alpaca_secret_key = env::var("FERROTICK_ALPACA_SECRET_KEY")
            .or_else(|_| env::var("ALPACA_SECRET_KEY"))
            .ok();
        self.alphavantage_api_key = env::var("FERROTICK_ALPHAVANTAGE_API_KEY")
            .or_else(|_| env::var("ALPHAVANTAGE_API_KEY"))
            .ok();
        self
    }

    /// Manually set the Polygon API key.
    pub fn with_polygon_key(mut self, key: impl Into<String>) -> Self {
        self.polygon_api_key = Some(key.into());
        self
    }

    /// Manually set the Alpaca API credentials.
    pub fn with_alpaca_keys(mut self, api_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        self.alpaca_api_key = Some(api_key.into());
        self.alpaca_secret_key = Some(secret_key.into());
        self
    }

    /// Manually set the Alpha Vantage API key.
    pub fn with_alphavantage_key(mut self, key: impl Into<String>) -> Self {
        self.alphavantage_api_key = Some(key.into());
        self
    }

    /// Enable or disable the Polygon adapter.
    pub fn with_polygon_enabled(mut self, enabled: bool) -> Self {
        self.enable_polygon = enabled;
        self
    }

    /// Enable or disable the Alpaca adapter.
    pub fn with_alpaca_enabled(mut self, enabled: bool) -> Self {
        self.enable_alpaca = enabled;
        self
    }

    /// Enable or disable the Alpha Vantage adapter.
    pub fn with_alphavantage_enabled(mut self, enabled: bool) -> Self {
        self.enable_alphavantage = enabled;
        self
    }

    /// Enable or disable the Yahoo adapter.
    pub fn with_yahoo_enabled(mut self, enabled: bool) -> Self {
        self.enable_yahoo = enabled;
        self
    }

    /// Build the SourceRouter with the configured adapters.
    pub fn build(self) -> SourceRouter {
        let mut adapters: Vec<Arc<dyn DataSource>> = Vec::new();

        if self.enable_polygon {
            adapters.push(if self.use_mock {
                Arc::new(PolygonAdapter::default())
            } else if let Some(key) = &self.polygon_api_key {
                let http_client = Arc::new(ReqwestHttpClient::new());
                Arc::new(PolygonAdapter::with_http_client(
                    http_client,
                    HttpAuth::Header {
                        name: String::from("X-API-Key"),
                        value: key.clone(),
                    },
                ))
            } else {
                // No API key available, use mock but with a warning capability
                Arc::new(PolygonAdapter::default())
            });
        }

        if self.enable_alpaca {
            adapters.push(if self.use_mock {
                Arc::new(AlpacaAdapter::default())
            } else if let (Some(api_key), Some(secret_key)) = (&self.alpaca_api_key, &self.alpaca_secret_key) {
                let http_client = Arc::new(ReqwestHttpClient::new());
                Arc::new(AlpacaAdapter::with_http_client(
                    http_client,
                    api_key.clone(),
                    secret_key.clone(),
                ))
            } else {
                Arc::new(AlpacaAdapter::default())
            });
        }

        if self.enable_alphavantage {
            adapters.push(if self.use_mock {
                Arc::new(AlphaVantageAdapter::default())
            } else if let Some(key) = &self.alphavantage_api_key {
                let http_client = Arc::new(ReqwestHttpClient::new());
                Arc::new(AlphaVantageAdapter::with_http_client(http_client, key.clone()))
            } else {
                Arc::new(AlphaVantageAdapter::default())
            });
        }

        if self.enable_yahoo {
            adapters.push(if self.use_mock {
                Arc::new(YahooAdapter::default())
            } else {
                let http_client = Arc::new(ReqwestHttpClient::new());
                Arc::new(YahooAdapter::with_http_client(
                    http_client,
                    HttpAuth::Cookie(String::new()), // Yahoo works with anonymous access
                ))
            });
        }

        if adapters.is_empty() {
            // Fallback to all mocks if nothing is enabled
            SourceRouter::default()
        } else {
            SourceRouter::new(adapters)
        }
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
            if !supports_endpoint {
                continue;
            }

            let endpoint_score = 1_000;
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
    fn auto_falls_back_to_alpaca_after_polygon_rate_limit() {
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

        assert_eq!(result.selected_source, ProviderId::Alpaca);
        assert_eq!(
            result.source_chain,
            vec![ProviderId::Polygon, ProviderId::Alpaca]
        );
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].source, Some(ProviderId::Polygon));
    }

    #[test]
    fn auto_chain_for_fundamentals_excludes_alpaca() {
        let router = SourceRouter::default();

        let chain = block_on(
            router.source_chain_for_strategy(Endpoint::Fundamentals, &SourceStrategy::Auto),
        );

        assert!(!chain.contains(&ProviderId::Alpaca));
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

        let result =
            block_on(router.route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon)));

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
