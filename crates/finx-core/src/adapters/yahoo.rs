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

/// Deterministic Yahoo adapter used by the Phase 2 routing pipeline.
#[derive(Clone)]
pub struct YahooAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl Default for YahooAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 78,
            http_client: Arc::new(NoopHttpClient),
            auth: HttpAuth::Cookie(String::from("B=finx-session")),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
        }
    }
}

impl YahooAdapter {
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
                "yahoo circuit breaker is open; skipping upstream call",
            ));
        }

        let request = HttpRequest::get(endpoint).with_auth(&self.auth);
        let response = self.http_client.execute(request).await.map_err(|error| {
            self.circuit_breaker.record_failure();
            if error.retryable() {
                SourceError::unavailable(format!("yahoo transport error: {}", error.message()))
            } else {
                SourceError::internal(format!("yahoo transport error: {}", error.message()))
            }
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo upstream returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(())
    }
}

impl DataSource for YahooAdapter {
    fn id(&self) -> ProviderId {
        ProviderId::Yahoo
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
                    "yahoo quote request requires at least one symbol",
                ));
            }

            self.execute_authenticated_call("https://query1.finance.yahoo.com/v7/finance/quote")
                .await?;

            let as_of = UtcDateTime::now();
            let quotes = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = YahooQuotePayload::from_symbol(symbol, as_of);
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
                    "yahoo bars request limit must be greater than zero",
                ));
            }

            self.execute_authenticated_call("https://query1.finance.yahoo.com/v8/finance/chart")
                .await?;

            let step = interval_duration(req.interval);
            let now = UtcDateTime::now().into_inner();
            let seed = symbol_seed(&req.symbol);
            let mut bars = Vec::with_capacity(req.limit);

            for index in 0..req.limit {
                let offset = step * (req.limit.saturating_sub(index + 1) as i32);
                let ts =
                    UtcDateTime::from_offset_datetime(now - offset).map_err(validation_to_error)?;
                let base = 90.0 + ((seed + index as u64) % 350) as f64 / 10.0;

                let raw = YahooBarPayload {
                    ts,
                    open: base,
                    high: base + 1.20,
                    low: base - 0.80,
                    close: base + 0.30,
                    volume: Some(20_000 + (index as u64) * 25),
                    vwap: Some(base + 0.15),
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
                    "yahoo fundamentals request requires at least one symbol",
                ));
            }

            self.execute_authenticated_call(
                "https://query2.finance.yahoo.com/v10/finance/quoteSummary",
            )
            .await?;

            let as_of = UtcDateTime::now();
            let fundamentals = req
                .symbols
                .iter()
                .map(|symbol| {
                    let payload = YahooFundamentalsPayload::from_symbol(symbol, as_of);
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
                    "yahoo search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "yahoo search limit must be greater than zero",
                ));
            }

            self.execute_authenticated_call("https://query2.finance.yahoo.com/v1/finance/search")
                .await?;

            let query_lower = query.to_ascii_lowercase();
            let results = yahoo_catalog()
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
struct YahooQuotePayload {
    ticker: String,
    regular_market_price: f64,
    regular_market_bid: f64,
    regular_market_ask: f64,
    regular_market_volume: u64,
    currency: &'static str,
    timestamp: UtcDateTime,
}

impl YahooQuotePayload {
    fn from_symbol(symbol: &Symbol, timestamp: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        let price = 92.0 + (seed % 500) as f64 / 10.0;
        Self {
            ticker: symbol.as_str().to_owned(),
            regular_market_price: price,
            regular_market_bid: price - 0.08,
            regular_market_ask: price + 0.08,
            regular_market_volume: 50_000 + seed % 10_000,
            currency: "USD",
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct YahooBarPayload {
    ts: UtcDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: Option<u64>,
    vwap: Option<f64>,
}

#[derive(Debug, Clone)]
struct YahooFundamentalsPayload {
    ticker: String,
    as_of: UtcDateTime,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    dividend_yield: Option<f64>,
}

impl YahooFundamentalsPayload {
    fn from_symbol(symbol: &Symbol, as_of: UtcDateTime) -> Self {
        let seed = symbol_seed(symbol);
        Self {
            ticker: symbol.as_str().to_owned(),
            as_of,
            market_cap: Some(500_000_000_000.0 + (seed % 300_000) as f64 * 1_000_000.0),
            pe_ratio: Some(14.0 + (seed % 200) as f64 / 10.0),
            dividend_yield: Some(0.005 + (seed % 50) as f64 / 10_000.0),
        }
    }
}

fn normalize_quote(payload: YahooQuotePayload) -> Result<Quote, SourceError> {
    let symbol = Symbol::parse(&payload.ticker).map_err(validation_to_error)?;
    Quote::new(
        symbol,
        payload.regular_market_price,
        Some(payload.regular_market_bid),
        Some(payload.regular_market_ask),
        Some(payload.regular_market_volume),
        payload.currency,
        payload.timestamp,
    )
    .map_err(validation_to_error)
}

fn normalize_bar(payload: YahooBarPayload) -> Result<Bar, SourceError> {
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

fn normalize_fundamentals(payload: YahooFundamentalsPayload) -> Result<Fundamental, SourceError> {
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

fn yahoo_catalog() -> Vec<Instrument> {
    [
        ("AAPL", "Apple Inc.", Some("NASDAQ"), AssetClass::Equity),
        (
            "MSFT",
            "Microsoft Corporation",
            Some("NASDAQ"),
            AssetClass::Equity,
        ),
        (
            "SPY",
            "SPDR S&P 500 ETF Trust",
            Some("ARCA"),
            AssetClass::Etf,
        ),
        ("QQQ", "Invesco QQQ Trust", Some("NASDAQ"), AssetClass::Etf),
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
    symbol.as_str().bytes().fold(0_u64, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(byte as u64)
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
                response: Err(HttpError::new("upstream timeout")),
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
    fn quote_request_applies_cookie_auth_header() {
        let client = Arc::new(RecordingHttpClient::success());
        let adapter = YahooAdapter::with_http_client(
            client.clone(),
            HttpAuth::Cookie(String::from("B=secure-cookie")),
        );
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);

        let requests = client.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].headers.get("cookie").map(String::as_str),
            Some("B=secure-cookie")
        );
    }

    #[test]
    fn circuit_breaker_opens_after_repeated_transport_failures() {
        let client = Arc::new(RecordingHttpClient::failure());
        let adapter =
            YahooAdapter::with_http_client(client, HttpAuth::Cookie(String::from("B=session")));
        let request = QuoteRequest::new(vec![Symbol::parse("MSFT").expect("valid symbol")])
            .expect("valid request");

        for _ in 0..3 {
            let error = block_on(adapter.quote(request.clone())).expect_err("call should fail");
            assert_eq!(error.kind(), SourceErrorKind::Unavailable);
        }

        let health = block_on(adapter.health());
        assert_eq!(health.state, HealthState::Unhealthy);
        assert!(!health.rate_available);

        let error = block_on(adapter.quote(request)).expect_err("breaker should block request");
        assert!(error.message().contains("circuit breaker is open"));
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
