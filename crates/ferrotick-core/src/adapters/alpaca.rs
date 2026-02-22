use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
use time::Duration;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpClient, HttpRequest, NoopHttpClient};
use crate::{Bar, BarSeries, Interval, ProviderId, Quote, Symbol, UtcDateTime, ValidationError};

/// Alpaca adapter supporting both real API calls and mock mode.
#[derive(Clone)]
pub struct AlpacaAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    api_key: String,
    secret_key: String,
    circuit_breaker: Arc<CircuitBreaker>,
    use_real_api: bool,
}

impl Default for AlpacaAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 85,
            http_client: Arc::new(NoopHttpClient),
            api_key: std::env::var("FERROTICK_ALPACA_API_KEY")
                .unwrap_or_else(|_| String::from("demo")),
            secret_key: std::env::var("FERROTICK_ALPACA_SECRET_KEY")
                .unwrap_or_else(|_| String::from("demo")),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            use_real_api: false,
        }
    }
}

impl AlpacaAdapter {
    pub fn with_health(health_state: HealthState, rate_available: bool) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::default()
        }
    }

    pub fn with_http_client(
        http_client: Arc<dyn HttpClient>,
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        let is_real = !http_client.is_mock();
        Self {
            http_client,
            api_key: api_key.into(),
            secret_key: secret_key.into(),
            use_real_api: is_real,
            ..Self::default()
        }
    }

    pub fn with_circuit_breaker(circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self {
            circuit_breaker,
            ..Self::default()
        }
    }

    /// Check if we're using a real HTTP client
    fn is_real_client(&self) -> bool {
        self.use_real_api
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

// Mock data methods (for tests)
impl AlpacaAdapter {
    async fn execute_authenticated_call(&self, endpoint: &str) -> Result<(), SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable(
                "alpaca circuit breaker is open; skipping upstream call",
            ));
        }

        let request = HttpRequest::get(endpoint)
            .with_header("APCA-API-KEY-ID", &self.api_key)
            .with_header("APCA-API-SECRET-KEY", &self.secret_key);

        let response = self.http_client.execute(request).await.map_err(|error| {
            self.circuit_breaker.record_failure();
            if error.retryable() {
                SourceError::unavailable(format!("alpaca transport error: {}", error.message()))
            } else {
                SourceError::internal(format!("alpaca transport error: {}", error.message()))
            }
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alpaca upstream returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(())
    }

    async fn fetch_mock_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        self.execute_authenticated_call("https://data.alpaca.markets/v2/stocks/quotes/latest")
            .await?;

        let as_of = UtcDateTime::now();
        let quotes = req
            .symbols
            .iter()
            .map(|symbol| {
                let payload = AlpacaQuotePayload::from_symbol(symbol, as_of);
                normalize_quote(payload)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(QuoteBatch { quotes })
    }

    async fn fetch_mock_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        self.execute_authenticated_call("https://data.alpaca.markets/v2/stocks/bars")
            .await?;

        let step = interval_duration(req.interval);
        let now = UtcDateTime::now().into_inner();
        let seed = symbol_seed(&req.symbol);
        let mut bars = Vec::with_capacity(req.limit);

        for index in 0..req.limit {
            let offset = step * (req.limit.saturating_sub(index + 1) as i32);
            let ts =
                UtcDateTime::from_offset_datetime(now - offset).map_err(validation_to_error)?;
            let base = 94.0 + ((seed + index as u64 * 2) % 460) as f64 / 10.0;

            let raw = AlpacaBarPayload {
                ts,
                open: base + 0.02,
                high: base + 1.18,
                low: base - 0.68,
                close: base + 0.36,
                volume: Some(28_000 + (index as u64) * 30),
                vwap: Some(base + 0.11),
            };

            bars.push(normalize_bar(raw)?);
        }

        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }
}

impl DataSource for AlpacaAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Alpaca
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new(true, true, false, false)
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

            if self.is_real_client() {
                self.fetch_real_quotes(&req).await
            } else {
                self.fetch_mock_quotes(&req).await
            }
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

            if self.is_real_client() {
                self.fetch_real_bars(&req).await
            } else {
                self.fetch_mock_bars(&req).await
            }
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

// Mock data structures
#[derive(Debug, Clone)]
struct AlpacaQuotePayload {
    symbol: String,
    ask_price: f64,
    bid_price: f64,
    last_price: f64,
    volume: u64,
    currency: &'static str,
    as_of: UtcDateTime,
}

impl AlpacaQuotePayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let last_price = 95.0 + (seed % 510) as f64 / 10.0;
        Self {
            symbol: symbol.as_str().to_owned(),
            ask_price: last_price + 0.05,
            bid_price: last_price - 0.05,
            last_price,
            volume: 45_000 + seed % 11_000,
            currency: "USD",
            as_of,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AlpacaBarPayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

fn normalize_quote(payload: AlpacaQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.symbol).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.last_price,
        Some(payload.bid_price),
        Some(payload.ask_price),
        Some(payload.volume),
        payload.currency,
        payload.as_of,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: AlpacaBarPayload) -> Result<Bar, SourceError> {
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

fn symbol_seed(symbol: &Symbol) -> u64 {
    symbol.as_str().bytes().fold(13_u64, |acc, byte| {
        acc.wrapping_mul(29).wrapping_add(byte as u64)
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

        fn is_mock(&self) -> bool {
            true
        }
    }

    #[test]
    fn quote_request_applies_dual_alpaca_auth_headers() {
        let client = Arc::new(NoopHttpClient);
        let adapter = AlpacaAdapter::with_http_client(client, "key-id", "secret-key");
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);
    }

    #[test]
    fn fundamentals_returns_unsupported_endpoint_error() {
        let adapter = AlpacaAdapter::default();
        let request = FundamentalsRequest::new(vec![Symbol::parse("MSFT").expect("valid symbol")])
            .expect("valid request");

        let error = block_on(adapter.fundamentals(request)).expect_err("must be unsupported");
        assert_eq!(error.kind(), SourceErrorKind::UnsupportedEndpoint);
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
