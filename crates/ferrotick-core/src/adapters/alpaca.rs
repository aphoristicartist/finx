use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpClient, HttpRequest};
use crate::{Bar, BarSeries, Interval, ProviderId, Quote, Symbol, UtcDateTime, ValidationError};

/// Alpaca adapter for real API calls.
#[derive(Clone)]
pub struct AlpacaAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    api_key: String,
    secret_key: String,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl AlpacaAdapter {
    /// Create a new AlpacaAdapter with a real HTTP client and credentials.
    pub fn with_http_client(
        http_client: Arc<dyn HttpClient>,
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 85,
            http_client,
            api_key: api_key.into(),
            secret_key: secret_key.into(),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
        }
    }

    /// Create a new AlpacaAdapter with a custom circuit breaker.
    pub fn with_circuit_breaker(
        circuit_breaker: Arc<CircuitBreaker>,
        http_client: Arc<dyn HttpClient>,
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            circuit_breaker,
            ..Self::with_http_client(http_client, api_key, secret_key)
        }
    }

    /// Create a new AlpacaAdapter with a custom circuit breaker for testing.
    /// Uses default HTTP client.
    pub fn with_circuit_breaker_for_test(
        circuit_breaker: Arc<CircuitBreaker>,
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self::with_circuit_breaker(
            circuit_breaker,
            Arc::new(crate::http_client::ReqwestHttpClient::new()),
            api_key,
            secret_key,
        )
    }
}

// Real API implementation methods
impl AlpacaAdapter {
    async fn fetch_real_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("alpaca circuit breaker is open"));
        }

        // Alpaca latest quotes endpoint
        let symbols_param = req
            .symbols
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let endpoint = format!(
            "https://data.alpaca.markets/v2/stocks/quotes/latest?symbols={}",
            symbols_param
        );

        let request = HttpRequest::get(&endpoint)
            .with_header("APCA-API-KEY-ID", &self.api_key)
            .with_header("APCA-API-SECRET-KEY", &self.secret_key)
            .with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("alpaca transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alpaca returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();

        // Parse Alpaca response
        let alpaca_response: AlpacaQuotesResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse alpaca response: {}", e)))?;

        let quotes = alpaca_response
            .quotes
            .into_iter()
            .filter_map(|(symbol_str, quote)| {
                let symbol = Symbol::parse(&symbol_str).ok()?;
                let ts_offset = time::OffsetDateTime::from_unix_timestamp(
                    quote.timestamp.parse().ok()?,
                )
                .ok()?;
                let ts = UtcDateTime::from_offset_datetime(ts_offset).ok()?;

                Quote::new(
                    symbol,
                    quote.last_quote_price(),
                    Some(quote.bid_price),
                    Some(quote.ask_price),
                    None,
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
            return Err(SourceError::unavailable("alpaca circuit breaker is open"));
        }

        let timeframe = match req.interval {
            Interval::OneMinute => "1Min",
            Interval::FiveMinutes => "5Min",
            Interval::FifteenMinutes => "15Min",
            Interval::OneHour => "1Hour",
            Interval::OneDay => "1Day",
        };

        let now = time::OffsetDateTime::now_utc();
        let start = now - time::Duration::days(req.limit as i64 * 2);

        let endpoint = format!(
            "https://data.alpaca.markets/v2/stocks/{}/bars?timeframe={}&start={}&limit={}",
            req.symbol.as_str(),
            timeframe,
            start.format(&time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z").unwrap()).unwrap(),
            req.limit
        );

        let request = HttpRequest::get(&endpoint)
            .with_header("APCA-API-KEY-ID", &self.api_key)
            .with_header("APCA-API-SECRET-KEY", &self.secret_key)
            .with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("alpaca transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alpaca returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();

        let bars_response: AlpacaBarsResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse alpaca bars: {}", e)))?;

        let mut bars = Vec::new();
        for bar_data in bars_response.bars.into_iter().take(req.limit) {
            let ts_offset = time::OffsetDateTime::parse(
                &bar_data.t,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(|e| SourceError::internal(format!("invalid timestamp: {}", e)))?;
            let ts = UtcDateTime::from_offset_datetime(ts_offset)
                .map_err(|e| SourceError::internal(format!("timestamp not UTC: {}", e)))?;

            if let Ok(bar) = Bar::new(
                ts,
                bar_data.o,
                bar_data.h,
                bar_data.l,
                bar_data.c,
                Some(bar_data.v as u64),
                bar_data.vw,
            ) {
                bars.push(bar);
            }
        }

        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }
}

impl DataSource for AlpacaAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Alpaca
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new(true, true, false, false, false, false)
    }

    fn quote<'a>(
        &'a self,
        req: QuoteRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QuoteBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            if req.symbols.is_empty() {
                return Err(SourceError::invalid_request(
                    "alpaca quote request requires at least one symbol",
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
                    "alpaca bars request limit must be greater than zero",
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
            let _ = req;
            Err(SourceError::unsupported_endpoint(Endpoint::Fundamentals))
        })
    }

    fn search<'a>(
        &'a self,
        req: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            let _ = req;
            Err(SourceError::unsupported_endpoint(Endpoint::Search))
        })
    }

    fn financials<'a>(
        &'a self,
        _req: crate::data_source::FinancialsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<crate::data_source::FinancialsBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            Err(SourceError::unsupported_endpoint(crate::data_source::Endpoint::Financials))
        })
    }

    fn earnings<'a>(
        &'a self,
        _req: crate::data_source::EarningsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<crate::data_source::EarningsBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            Err(SourceError::unsupported_endpoint(crate::data_source::Endpoint::Earnings))
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

// Alpaca API response structures
#[derive(Debug, Clone, Deserialize)]
struct AlpacaQuotesResponse {
    quotes: std::collections::HashMap<String, AlpacaQuoteData>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlpacaQuoteData {
    #[serde(rename = "bp")]
    bid_price: f64,
    #[serde(rename = "ap")]
    ask_price: f64,
    #[serde(rename = "t")]
    timestamp: String,
}

impl AlpacaQuoteData {
    fn last_quote_price(&self) -> f64 {
        (self.bid_price + self.ask_price) / 2.0
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AlpacaBarsResponse {
    #[serde(default)]
    bars: Vec<AlpacaBarData>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlpacaBarData {
    t: String, // timestamp
    o: f64,    // open
    h: f64,    // high
    l: f64,    // low
    c: f64,    // close
    v: i64,    // volume
    #[serde(default)]
    vw: Option<f64>, // vwap
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
        let adapter = AlpacaAdapter::with_http_client(client, "demo-key", "demo-secret");
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
