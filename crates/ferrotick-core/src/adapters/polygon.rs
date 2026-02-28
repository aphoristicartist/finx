use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpAuth, HttpClient, HttpRequest};
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Polygon adapter for real API calls.
#[derive(Clone)]
pub struct PolygonAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl PolygonAdapter {
    /// Create a new PolygonAdapter with a real HTTP client and authentication.
    pub fn with_http_client(http_client: Arc<dyn HttpClient>, auth: HttpAuth) -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 90,
            http_client,
            auth,
            circuit_breaker: Arc::new(CircuitBreaker::default()),
        }
    }

    /// Create an adapter with custom health state (for testing).
    pub fn with_health(
        health_state: HealthState,
        rate_available: bool,
        http_client: Arc<dyn HttpClient>,
        auth: HttpAuth,
    ) -> Self {
        Self {
            health_state,
            rate_available,
            http_client,
            auth,
            ..Self::with_http_client(
                Arc::new(crate::http_client::ReqwestHttpClient::new()),
                HttpAuth::None,
            )
        }
    }

    /// Create an adapter with a custom circuit breaker.
    pub fn with_circuit_breaker(
        circuit_breaker: Arc<CircuitBreaker>,
        http_client: Arc<dyn HttpClient>,
        auth: HttpAuth,
    ) -> Self {
        Self {
            circuit_breaker,
            http_client,
            auth,
            ..Self::with_http_client(
                Arc::new(crate::http_client::ReqwestHttpClient::new()),
                HttpAuth::None,
            )
        }
    }

    /// Create a new PolygonAdapter with a custom circuit breaker for testing.
    /// Uses default HTTP client and auth.
    pub fn with_circuit_breaker_for_test(circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self::with_circuit_breaker(
            circuit_breaker,
            Arc::new(crate::http_client::ReqwestHttpClient::new()),
            HttpAuth::None,
        )
    }
}

// Real API implementation methods
impl PolygonAdapter {
    async fn fetch_real_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("polygon circuit breaker is open"));
        }

        // Polygon previous close endpoint - most reliable for quotes
        let symbol = &req.symbols[0];
        let endpoint = format!(
            "https://api.polygon.io/v2/aggs/ticker/{}/prev?adjusted=true",
            symbol.as_str()
        );

        let request = HttpRequest::get(&endpoint)
            .with_auth(&self.auth)
            .with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("polygon transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "polygon returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();

        // Parse Polygon response
        let polygon_response: PolygonPrevCloseResponse = serde_json::from_str(&response.body)
            .map_err(|e| {
                SourceError::internal(format!("failed to parse polygon response: {}", e))
            })?;

        if polygon_response.status != "OK" && polygon_response.status != "DELAYED" {
            return Err(SourceError::unavailable(format!(
                "polygon API error: {}",
                polygon_response.status
            )));
        }

        let quotes = polygon_response
            .results
            .into_iter()
            .filter_map(|result| {
                let sym = Symbol::parse(&result.ticker).ok()?;
                let ts_offset = time::OffsetDateTime::from_unix_timestamp(result.t).ok()?;
                let ts = UtcDateTime::from_offset_datetime(ts_offset).ok()?;

                Quote::new(
                    sym,
                    result.c,              // close price as last price
                    Some(result.c - 0.05), // approximate bid
                    Some(result.c + 0.05), // approximate ask
                    result.v.map(|v| v as u64),
                    "USD",
                    ts,
                )
                .ok()
            })
            .collect();

        Ok(QuoteBatch { quotes })
    }

    async fn fetch_real_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("polygon circuit breaker is open"));
        }

        let timespan = match req.interval {
            Interval::OneMinute => "minute",
            Interval::FiveMinutes => "minute",
            Interval::FifteenMinutes => "minute",
            Interval::OneHour => "hour",
            Interval::OneDay => "day",
        };

        let multiplier = match req.interval {
            Interval::OneMinute => 1,
            Interval::FiveMinutes => 5,
            Interval::FifteenMinutes => 15,
            Interval::OneHour => 1,
            Interval::OneDay => 1,
        };

        let now = time::OffsetDateTime::now_utc();
        let from = now - time::Duration::days(req.limit as i64 * 2);
        let to = now;

        let endpoint = format!(
            "https://api.polygon.io/v2/aggs/ticker/{}/range/{}/{}/{}/{}?adjusted=true&sort=desc&limit={}",
            req.symbol.as_str(),
            multiplier,
            timespan,
            from.format(&time::format_description::parse("[year]-[month]-[day]").unwrap()).unwrap(),
            to.format(&time::format_description::parse("[year]-[month]-[day]").unwrap()).unwrap(),
            req.limit
        );

        let request = HttpRequest::get(&endpoint)
            .with_auth(&self.auth)
            .with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("polygon transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "polygon returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();

        let polygon_response: PolygonAggsResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse polygon aggs: {}", e)))?;

        let mut bars = Vec::new();
        for result in polygon_response.results.into_iter().take(req.limit) {
            let ts_offset = time::OffsetDateTime::from_unix_timestamp(result.t)
                .map_err(|e| SourceError::internal(format!("invalid timestamp: {}", e)))?;
            let ts = UtcDateTime::from_offset_datetime(ts_offset)
                .map_err(|e| SourceError::internal(format!("timestamp not UTC: {}", e)))?;

            if let Ok(bar) = Bar::new(
                ts,
                result.o,
                result.h,
                result.l,
                result.c,
                result.v.map(|v| v as u64),
                result.vw,
            ) {
                bars.push(bar);
            }
        }

        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }

    async fn fetch_real_fundamentals(
        &self,
        req: &FundamentalsRequest,
    ) -> Result<FundamentalsBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("polygon circuit breaker is open"));
        }

        let fundamentals = req
            .symbols
            .iter()
            .map(|symbol| {
                let as_of = UtcDateTime::now();
                Fundamental::new(symbol.clone(), as_of, None, None, None)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: ValidationError| SourceError::internal(e.to_string()))?;

        Ok(FundamentalsBatch { fundamentals })
    }

    async fn execute_real_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("polygon circuit breaker is open"));
        }

        let endpoint = format!(
            "https://api.polygon.io/v3/reference/tickers?search={}&limit={}&active=true",
            urlencoding::encode(&req.query),
            req.limit
        );

        let request = HttpRequest::get(&endpoint)
            .with_auth(&self.auth)
            .with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("polygon transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "polygon returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();

        let search_response: PolygonTickerSearchResponse = serde_json::from_str(&response.body)
            .map_err(|e| {
                SourceError::internal(format!("failed to parse search response: {}", e))
            })?;

        let results = search_response
            .results
            .into_iter()
            .filter_map(|ticker| {
                let symbol = Symbol::parse(&ticker.ticker).ok()?;
                let asset_class = match ticker.market.as_deref() {
                    Some("stocks") => AssetClass::Equity,
                    Some("crypto") => AssetClass::Crypto,
                    Some("fx") => AssetClass::Forex,
                    _ => AssetClass::Other,
                };

                Instrument::new(
                    symbol,
                    ticker.name,
                    ticker.primary_exchange,
                    ticker.currency_name.unwrap_or_else(|| "USD".to_string()),
                    asset_class,
                    ticker.active.unwrap_or(true),
                )
                .ok()
            })
            .take(req.limit)
            .collect();

        Ok(SearchBatch {
            query: req.query.clone(),
            results,
        })
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

            self.fetch_real_quotes(&req).await
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

            self.fetch_real_bars(&req).await
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

            self.fetch_real_fundamentals(&req).await
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

            self.execute_real_search(&req).await
        })
    }

    fn financials<'a>(
        &'a self,
        _req: crate::data_source::FinancialsRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<crate::data_source::FinancialsBatch, SourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Err(SourceError::unsupported_endpoint(
                crate::data_source::Endpoint::Financials,
            ))
        })
    }

    fn earnings<'a>(
        &'a self,
        _req: crate::data_source::EarningsRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<crate::data_source::EarningsBatch, SourceError>> + Send + 'a,
        >,
    > {
        Box::pin(async move {
            Err(SourceError::unsupported_endpoint(
                crate::data_source::Endpoint::Earnings,
            ))
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

// Polygon API response structures
#[derive(Debug, Clone, Deserialize)]
struct PolygonPrevCloseResponse {
    status: String,
    #[serde(default)]
    results: Vec<PolygonPrevCloseResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct PolygonPrevCloseResult {
    #[serde(rename = "T")]
    ticker: String,
    #[serde(rename = "c")]
    c: f64, // close
    #[serde(rename = "v", default)]
    v: Option<i64>, // volume
    #[serde(rename = "t")]
    t: i64, // timestamp
}

#[derive(Debug, Clone, Deserialize)]
struct PolygonAggsResponse {
    #[serde(default)]
    results: Vec<PolygonAggResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct PolygonAggResult {
    #[serde(rename = "o")]
    o: f64, // open
    #[serde(rename = "h")]
    h: f64, // high
    #[serde(rename = "l")]
    l: f64, // low
    #[serde(rename = "c")]
    c: f64, // close
    #[serde(rename = "v", default)]
    v: Option<i64>, // volume
    #[serde(rename = "vw", default)]
    vw: Option<f64>, // vwap
    #[serde(rename = "t")]
    t: i64, // timestamp
}

#[derive(Debug, Clone, Deserialize)]
struct PolygonTickerSearchResponse {
    #[serde(default)]
    results: Vec<PolygonTickerResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct PolygonTickerResult {
    ticker: String,
    name: String,
    market: Option<String>,
    #[serde(default)]
    active: Option<bool>,
    primary_exchange: Option<String>,
    currency_name: Option<String>,
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
        fn with_response(response: Result<HttpResponse, HttpError>) -> Self {
            Self {
                response,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn success_json(json: &str) -> Self {
            Self::with_response(Ok(HttpResponse::ok_json(json)))
        }

        fn failure() -> Self {
            Self::with_response(Err(HttpError::new("network error")))
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
