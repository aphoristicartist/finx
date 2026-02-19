use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use time::Duration;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpClient, HttpRequest, NoopHttpClient};
use crate::provider_policy::ProviderPolicy;
use crate::throttling::ThrottlingQueue;
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Deterministic Alpha Vantage adapter used by Phase 5 routing.
#[derive(Clone)]
pub struct AlphaVantageAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    api_key: String,
    circuit_breaker: Arc<CircuitBreaker>,
    throttling: ThrottlingQueue,
}

impl Default for AlphaVantageAdapter {
    fn default() -> Self {
        let policy = ProviderPolicy::alphavantage_default();
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 70,
            http_client: Arc::new(NoopHttpClient),
            api_key: std::env::var("FINX_ALPHAVANTAGE_API_KEY")
                .unwrap_or_else(|_| String::from("demo")),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            throttling: ThrottlingQueue::from_policy(&policy),
        }
    }
}

impl AlphaVantageAdapter {
    pub fn with_health(health_state: HealthState, rate_available: bool) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::default()
        }
    }

    pub fn with_http_client(http_client: Arc<dyn HttpClient>, api_key: impl Into<String>) -> Self {
        Self {
            http_client,
            api_key: api_key.into(),
            ..Self::default()
        }
    }

    pub fn with_circuit_breaker(circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self {
            circuit_breaker,
            ..Self::default()
        }
    }

    async fn execute_authenticated_call(&self, endpoint: &str) -> Result<(), SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable(
                "alphavantage circuit breaker is open; skipping upstream call",
            ));
        }

        let retry_delay = self.throttling.acquire().err();
        if let Some(delay) = retry_delay {
            return Err(SourceError::rate_limited(format!(
                "alphavantage free-tier limit exceeded; retry in {:.2}s",
                delay.as_secs_f64()
            )));
        }

        let request = HttpRequest::get(self.with_api_key(endpoint));
        let response = self.http_client.execute(request).await.map_err(|error| {
            self.circuit_breaker.record_failure();
            if error.retryable() {
                SourceError::unavailable(format!(
                    "alphavantage transport error: {}",
                    error.message()
                ))
            } else {
                SourceError::internal(format!("alphavantage transport error: {}", error.message()))
            }
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alphavantage upstream returned status {}",
                response.status
            )));
        }

        self.throttling.complete_one();
        self.circuit_breaker.record_success();
        Ok(())
    }

    fn with_api_key(&self, endpoint: &str) -> String {
        if endpoint.contains('?') {
            format!("{endpoint}&apikey={}", self.api_key)
        } else {
            format!("{endpoint}?apikey={}", self.api_key)
        }
    }
}

impl DataSource for AlphaVantageAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Alphavantage
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::full()
    }

    fn quote<'a>(
        &'a self,
        req: QuoteRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QuoteBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            if req.symbols.is_empty() {
                return Err(SourceError::invalid_request(
                    "alphavantage quote request requires at least one symbol",
                ));
            }

            self.execute_authenticated_call(
                "https://www.alphavantage.co/query?function=GLOBAL_QUOTE",
            )
            .await?;

            let as_of = UtcDateTime::now();
            let quotes = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = AlphaVantageQuotePayload::from_symbol(symbol, as_of);
                    normalize_quote(payload)
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(QuoteBatch { quotes })
        })
    }

    fn bars<'a>(
        &'a self,
        req: BarsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BarSeries, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "alphavantage bars request limit must be greater than zero",
                ));
            }

            self.execute_authenticated_call(
                "https://www.alphavantage.co/query?function=TIME_SERIES_INTRADAY",
            )
            .await?;

            let step = interval_duration(req.interval);
            let now = UtcDateTime::now().into_inner();
            let seed = symbol_seed(&req.symbol);
            let mut bars = Vec::with_capacity(req.limit);

            for index in 0..req.limit {
                let offset = step * (req.limit.saturating_sub(index + 1) as i32);
                let ts =
                    UtcDateTime::from_offset_datetime(now - offset).map_err(validation_to_error)?;
                let base = 88.0 + ((seed + index as u64 * 5) % 500) as f64 / 10.0;

                let raw = AlphaVantageBarPayload {
                    ts,
                    open: base,
                    high: base + 1.10,
                    low: base - 0.70,
                    close: base + 0.33,
                    volume: Some(18_000 + (index as u64) * 20),
                    vwap: Some(base + 0.12),
                };

                bars.push(normalize_bar(raw)?);
            }

            Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
        })
    }

    fn fundamentals<'a>(
        &'a self,
        req: FundamentalsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FundamentalsBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            if req.symbols.is_empty() {
                return Err(SourceError::invalid_request(
                    "alphavantage fundamentals request requires at least one symbol",
                ));
            }

            self.execute_authenticated_call("https://www.alphavantage.co/query?function=OVERVIEW")
                .await?;

            let as_of = UtcDateTime::now();
            let fundamentals = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = AlphaVantageFundamentalsPayload::from_symbol(symbol, as_of);
                    normalize_fundamentals(payload)
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(FundamentalsBatch { fundamentals })
        })
    }

    fn search<'a>(
        &'a self,
        req: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            let query = req.query.trim();
            if query.is_empty() {
                return Err(SourceError::invalid_request(
                    "alphavantage search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "alphavantage search limit must be greater than zero",
                ));
            }

            self.execute_authenticated_call(
                "https://www.alphavantage.co/query?function=SYMBOL_SEARCH",
            )
            .await?;

            let query_lower = query.to_ascii_lowercase();
            let results = alphavantage_catalog()
                .into_iter()
                .filter(|instrument| {
                    instrument
                        .symbol
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(&query_lower)
                        || instrument.name.to_ascii_lowercase().contains(&query_lower)
                })
                .take(req.limit)
                .collect::<Vec<_>>();

            Ok(SearchBatch {
                query: query.to_owned(),
                results,
            })
        })
    }

    fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = HealthStatus> + Send + 'a>> {
        Box::pin(async move {
            let circuit_state = self.circuit_breaker.state();
            let mut state = self.health_state;
            let mut rate_available = self.rate_available && self.throttling.pending_len() == 0;

            match circuit_state {
                CircuitState::Closed => {}
                CircuitState::HalfOpen => {
                    if state == HealthState::Healthy {
                        state = HealthState::Degraded;
                    }
                }
                CircuitState::Open => {
                    state = HealthState::Unhealthy;
                    rate_available = false;
                }
            }

            HealthStatus::new(state, rate_available, self.score)
        })
    }
}

#[derive(Debug, Clone)]
struct AlphaVantageQuotePayload {
    symbol: String,
    price: f64,
    bid: f64,
    ask: f64,
    volume: u64,
    currency: &'static str,
    as_of: UtcDateTime,
}

impl AlphaVantageQuotePayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let price = 91.0 + (seed % 520) as f64 / 10.0;
        Self {
            symbol: symbol.as_str().to_owned(),
            price,
            bid: price - 0.07,
            ask: price + 0.07,
            volume: 30_000 + seed % 12_000,
            currency: "USD",
            as_of,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AlphaVantageBarPayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

#[derive(Debug, Clone)]
struct AlphaVantageFundamentalsPayload {
    symbol: String,
    as_of: UtcDateTime,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    dividend_yield: Option<f64>,
}

impl AlphaVantageFundamentalsPayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        Self {
            symbol: symbol.as_str().to_owned(),
            as_of,
            market_cap: Some(400_000_000_000.0 + (seed % 280_000) as f64 * 1_000_000.0),
            pe_ratio: Some(12.0 + (seed % 220) as f64 / 10.0),
            dividend_yield: Some(0.003 + (seed % 55) as f64 / 10_000.0),
        }
    }
}

fn normalize_quote(payload: AlphaVantageQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.symbol).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.price,
        Some(payload.bid),
        Some(payload.ask),
        Some(payload.volume),
        payload.currency,
        payload.as_of,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: AlphaVantageBarPayload) -> Result<Bar, SourceError> {
    Bar::new(
        payload.ts,
        payload.open,
        payload.high,
        payload.low,
        payload.close,
        payload.volume,
        payload.vwap,
    )
    .map_err(validation_to_error)
}

fn normalize_fundamentals(
    payload: AlphaVantageFundamentalsPayload,
) -> Result<Fundamental, SourceError> {
    let symbol = Symbol::parse(&payload.symbol).map_err(validation_to_error)?;
    Fundamental::new(
        symbol,
        payload.as_of,
        payload.market_cap,
        payload.pe_ratio,
        payload.dividend_yield,
    )
    .map_err(validation_to_error)
}

fn alphavantage_catalog() -> Vec<Instrument> {
    [
        ("AAPL", "Apple Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "AMZN",
            "Amazon.com, Inc.",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        (
            "META",
            "Meta Platforms, Inc.",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        ("VOO", "Vanguard S&P 500 ETF", Some("ARCA"), AssetClass::Etf),
    ]
    .into_iter()
    .map(|(symbol, name, exchange, asset_class)| {
        Instrument::new(
            Symbol::parse(symbol).expect("catalog symbols are valid"),
            name,
            exchange.map(str::to_owned),
            "USD",
            asset_class,
            true,
        )
        .expect("catalog entries are valid")
    })
    .collect::<Vec<_>>()
}

fn symbol_seed(symbol: &Symbol) -> u64 {
    symbol.as_str().bytes().fold(11_u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u64)
    })
}

fn interval_duration(interval: Interval) -> Duration {
    match interval {
        Interval::OneMinute => Duration::minutes(1),
        Interval::FiveMinutes => Duration::minutes(5),
        Interval::FifteenMinutes => Duration::minutes(15),
        Interval::OneHour => Duration::hours(1),
        Interval::OneDay => Duration::days(1),
    }
}

fn validation_to_error(error: ValidationError) -> SourceError {
    SourceError::internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::SourceErrorKind;
    use crate::http_client::{HttpError, HttpResponse};
    use std::sync::Mutex;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    #[derive(Debug)]
    struct RecordingHttpClient {
        response: Result<HttpResponse, HttpError>,
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl RecordingHttpClient {
        fn success() -> Self {
            Self {
                response: Ok(HttpResponse::ok_json("{}")),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn recorded_requests(&self) -> Vec<HttpRequest> {
            self.requests
                .lock()
                .expect("request store should not be poisoned")
                .clone()
        }
    }

    impl HttpClient for RecordingHttpClient {
        fn execute<'a>(
            &'a self,
            request: HttpRequest,
        ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>> {
            self.requests
                .lock()
                .expect("request store should not be poisoned")
                .push(request);
            let response = self.response.clone();
            Box::pin(async move { response })
        }
    }

    #[test]
    fn quote_request_appends_api_key_query_parameter() {
        let client = Arc::new(RecordingHttpClient::success());
        let adapter = AlphaVantageAdapter::with_http_client(client.clone(), "alpha-key");
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);

        let requests = client.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].url.contains("apikey=alpha-key"));
    }

    #[test]
    fn quote_rate_limits_after_five_calls_per_minute() {
        let adapter = AlphaVantageAdapter::default();
        let request = QuoteRequest::new(vec![Symbol::parse("MSFT").expect("valid symbol")])
            .expect("valid request");

        for _ in 0..5 {
            let response = block_on(adapter.quote(request.clone()));
            assert!(response.is_ok());
        }

        let error = block_on(adapter.quote(request)).expect_err("sixth call should rate limit");
        assert_eq!(error.kind(), SourceErrorKind::RateLimited);
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
