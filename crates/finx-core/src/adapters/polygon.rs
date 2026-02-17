use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use time::Duration;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpAuth, HttpClient, HttpRequest, NoopHttpClient};
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Deterministic Polygon adapter used by the Phase 2 routing pipeline.
#[derive(Clone)]
pub struct PolygonAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl Default for PolygonAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 90,
            http_client: Arc::new(NoopHttpClient),
            auth: HttpAuth::Header {
                name: String::from("x-api-key"),
                value: std::env::var("FINX_POLYGON_API_KEY")
                    .unwrap_or_else(|_| String::from("demo")),
            },
            circuit_breaker: Arc::new(CircuitBreaker::default()),
        }
    }
}

impl PolygonAdapter {
    pub fn with_health(health_state: HealthState, rate_available: bool) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::default()
        }
    }

    pub fn with_http_client(http_client: Arc<dyn HttpClient>, auth: HttpAuth) -> Self {
        Self {
            http_client,
            auth,
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
                "polygon circuit breaker is open; skipping upstream call",
            ));
        }

        let request = HttpRequest::get(endpoint).with_auth(&self.auth);
        let response = self.http_client.execute(request).await.map_err(|error| {
            self.circuit_breaker.record_failure();
            if error.retryable() {
                SourceError::unavailable(format!("polygon transport error: {}", error.message()))
            } else {
                SourceError::internal(format!("polygon transport error: {}", error.message()))
            }
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "polygon upstream returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(())
    }
}

impl DataSource for PolygonAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Polygon
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
                    "polygon quote request requires at least one symbol",
                ));
            }
            if req.symbols.len() > 3 {
                return Err(SourceError::rate_limited(
                    "polygon quote batch limit exceeded (max 3 symbols)",
                ));
            }

            self.execute_authenticated_call("https://api.polygon.io/v2/last/trade")
                .await?;

            let as_of = UtcDateTime::now();
            let quotes = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = PolygonQuotePayload::from_symbol(symbol, as_of);
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
                    "polygon bars request limit must be greater than zero",
                ));
            }
            if req.interval == Interval::OneMinute && req.limit > 120 {
                return Err(SourceError::rate_limited(
                    "polygon free-tier minute bars limit exceeded (max 120)",
                ));
            }

            self.execute_authenticated_call("https://api.polygon.io/v2/aggs/ticker")
                .await?;

            let step = interval_duration(req.interval);
            let now = UtcDateTime::now().into_inner();
            let seed = symbol_seed(&req.symbol);
            let mut bars = Vec::with_capacity(req.limit);

            for index in 0..req.limit {
                let offset = step * (req.limit.saturating_sub(index + 1) as i32);
                let ts =
                    UtcDateTime::from_offset_datetime(now - offset).map_err(validation_to_error)?;
                let base = 95.0 + ((seed + index as u64 * 3) % 420) as f64 / 10.0;

                let raw = PolygonAggregatePayload {
                    ts,
                    open: base + 0.05,
                    high: base + 1.35,
                    low: base - 0.75,
                    close: base + 0.42,
                    volume: Some(35_000 + (index as u64) * 40),
                    vwap: Some(base + 0.20),
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
                    "polygon fundamentals request requires at least one symbol",
                ));
            }
            if req.symbols.len() > 2 {
                return Err(SourceError::rate_limited(
                    "polygon ticker metadata batch limit exceeded (max 2 symbols)",
                ));
            }

            self.execute_authenticated_call("https://api.polygon.io/v3/reference/tickers")
                .await?;

            let as_of = UtcDateTime::now();
            let fundamentals = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = PolygonTickerPayload::from_symbol(symbol, as_of);
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
                    "polygon search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "polygon search limit must be greater than zero",
                ));
            }

            self.execute_authenticated_call("https://api.polygon.io/v3/reference/tickers")
                .await?;

            let query_lower = query.to_ascii_lowercase();
            let results = polygon_catalog()
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
            let mut rate_available = self.rate_available;

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
struct PolygonQuotePayload {
    ticker: String,
    last_trade_price: f64,
    bid_price: f64,
    ask_price: f64,
    session_volume: u64,
    currency: &'static str,
    timestamp: UtcDateTime,
}

impl PolygonQuotePayload {
    fn from_symbol(symbol: &Symbol, timestamp: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let price = 93.5 + (seed % 540) as f64 / 10.0;
        Self {
            ticker: symbol.as_str().to_owned(),
            last_trade_price: price,
            bid_price: price - 0.06,
            ask_price: price + 0.06,
            session_volume: 65_000 + seed % 15_000,
            currency: "USD",
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PolygonAggregatePayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

#[derive(Debug, Clone)]
struct PolygonTickerPayload {
    ticker: String,
    as_of: UtcDateTime,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    dividend_yield: Option<f64>,
}

impl PolygonTickerPayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        Self {
            ticker: symbol.as_str().to_owned(),
            as_of,
            market_cap: Some(700_000_000_000.0 + (seed % 250_000) as f64 * 1_000_000.0),
            pe_ratio: Some(16.0 + (seed % 250) as f64 / 10.0),
            dividend_yield: Some(0.004 + (seed % 40) as f64 / 10_000.0),
        }
    }
}

fn normalize_quote(payload: PolygonQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.last_trade_price,
        Some(payload.bid_price),
        Some(payload.ask_price),
        Some(payload.session_volume),
        payload.currency,
        payload.timestamp,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: PolygonAggregatePayload) -> Result<Bar, SourceError> {
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

fn normalize_fundamentals(payload: PolygonTickerPayload) -> Result<Fundamental, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Fundamental::new(
        symbol,
        payload.as_of,
        payload.market_cap,
        payload.pe_ratio,
        payload.dividend_yield,
    )
    .map_err(validation_to_error)
}

fn polygon_catalog() -> Vec<Instrument> {
    [
        ("AAPL", "Apple Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "MSFT",
            "Microsoft Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        (
            "NVDA",
            "NVIDIA Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        ("TSLA", "Tesla, Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "SPY",
            "SPDR S&P 500 ETF Trust",
            Some("ARCA"),
            AssetClass::Etf,
        ),
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
    symbol.as_str().bytes().fold(7_u64, |acc, byte| {
        acc.wrapping_mul(37).wrapping_add(byte as u64)
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

        fn failure() -> Self {
            Self {
                response: Err(HttpError::new("network error")),
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
    fn quote_request_applies_api_key_header() {
        let client = Arc::new(RecordingHttpClient::success());
        let adapter = PolygonAdapter::with_http_client(
            client.clone(),
            HttpAuth::Header {
                name: String::from("X-API-Key"),
                value: String::from("key-123"),
            },
        );
        let request = QuoteRequest::new(vec![Symbol::parse("NVDA").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);

        let requests = client.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].headers.get("x-api-key").map(String::as_str),
            Some("key-123")
        );
    }

    #[test]
    fn circuit_breaker_marks_health_unhealthy_after_failures() {
        let client = Arc::new(RecordingHttpClient::failure());
        let adapter = PolygonAdapter::with_http_client(
            client,
            HttpAuth::Header {
                name: String::from("x-api-key"),
                value: String::from("demo"),
            },
        );
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        for _ in 0..3 {
            let error = block_on(adapter.quote(request.clone())).expect_err("call should fail");
            assert_eq!(error.kind(), SourceErrorKind::Unavailable);
        }

        let health = block_on(adapter.health());
        assert_eq!(health.state, HealthState::Unhealthy);
        assert!(!health.rate_available);
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
