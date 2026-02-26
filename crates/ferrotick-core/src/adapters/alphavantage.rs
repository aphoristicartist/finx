use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;

use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpClient, HttpRequest};
use crate::provider_policy::ProviderPolicy;
use crate::throttling::ThrottlingQueue;
use crate::{
    AssetClass, Bar, BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol,
    UtcDateTime, ValidationError,
};

/// Alpha Vantage adapter for real API calls.
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

impl AlphaVantageAdapter {
    /// Create a new AlphaVantageAdapter with a real HTTP client and API key.
    pub fn with_http_client(http_client: Arc<dyn HttpClient>, api_key: impl Into<String>) -> Self {
        let policy = ProviderPolicy::alphavantage_default();
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 70,
            http_client,
            api_key: api_key.into(),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            throttling: ThrottlingQueue::from_policy(&policy),
        }
    }

    /// Create a new AlphaVantageAdapter with a custom circuit breaker.
    pub fn with_circuit_breaker(
        circuit_breaker: Arc<CircuitBreaker>,
        http_client: Arc<dyn HttpClient>,
        api_key: impl Into<String>,
    ) -> Self {
        let policy = ProviderPolicy::alphavantage_default();
        Self {
            circuit_breaker,
            ..Self::with_http_client(http_client, api_key)
        }
    }

    /// Create a new AlphaVantageAdapter with a custom circuit breaker for testing.
    /// Uses default HTTP client.
    pub fn with_circuit_breaker_for_test(
        circuit_breaker: Arc<CircuitBreaker>,
        api_key: impl Into<String>,
    ) -> Self {
        Self::with_circuit_breaker(
            circuit_breaker,
            Arc::new(crate::http_client::ReqwestHttpClient::new()),
            api_key,
        )
    }
}

// Real API implementation methods
impl AlphaVantageAdapter {
    async fn fetch_real_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("alphavantage circuit breaker is open"));
        }

        let retry_delay = self.throttling.acquire().err();
        if let Some(delay) = retry_delay {
            return Err(SourceError::rate_limited(format!(
                "alphavantage free-tier limit exceeded; retry in {:.2}s",
                delay.as_secs_f64()
            )));
        }

        // Alpha Vantage GLOBAL_QUOTE endpoint
        let endpoint = format!(
            "https://www.alphavantage.co/query?function=GLOBAL_QUOTE&symbol={}&apikey={}",
            req.symbols[0].as_str(),
            self.api_key
        );

        let request = HttpRequest::get(&endpoint).with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("alphavantage transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alphavantage returned status {}",
                response.status
            )));
        }

        self.throttling.complete_one();
        self.circuit_breaker.record_success();

        // Parse Alpha Vantage response
        let av_response: AlphaVantageQuoteResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse alphavantage response: {}", e)))?;

        if av_response.quote.is_none() {
            return Err(SourceError::unavailable("no quote data in alphavantage response"));
        }

        let quote_data = av_response.quote.unwrap();
        let as_of = UtcDateTime::now();

        let quote = Quote::new(
            req.symbols[0].clone(),
            quote_data.price,
            None,
            None,
            None,
            "USD",
            as_of,
        )
        .map_err(|e| SourceError::internal(e.to_string()))?;

        Ok(QuoteBatch { quotes: vec![quote] })
    }

    async fn fetch_real_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("alphavantage circuit breaker is open"));
        }

        let retry_delay = self.throttling.acquire().err();
        if let Some(delay) = retry_delay {
            return Err(SourceError::rate_limited(format!(
                "alphavantage free-tier limit exceeded; retry in {:.2}s",
                delay.as_secs_f64()
            )));
        }

        let interval_str = match req.interval {
            Interval::OneMinute => "1min",
            Interval::FiveMinutes => "5min",
            Interval::FifteenMinutes => "15min",
            Interval::OneHour => "60min",
            Interval::OneDay => "daily",
        };

        let endpoint = format!(
            "https://www.alphavantage.co/query?function=TIME_SERIES_INTRADAY&symbol={}&interval={}&apikey={}",
            req.symbol.as_str(),
            interval_str,
            self.api_key
        );

        let request = HttpRequest::get(&endpoint).with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("alphavantage transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alphavantage returned status {}",
                response.status
            )));
        }

        self.throttling.complete_one();
        self.circuit_breaker.record_success();

        let av_response: AlphaVantageTimeSeriesResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse alphavantage bars: {}", e)))?;

        let time_series = av_response.get_time_series()
            .ok_or_else(|| SourceError::internal("no time series data in response"))?;

        let mut bars = Vec::new();
        for (timestamp_str, bar_data) in time_series.into_iter().take(req.limit) {
            // Parse ISO timestamp like "2025-01-14 16:00:00"
            let ts_offset = time::OffsetDateTime::parse(&timestamp_str, &time::format_description::well_known::Iso8601::DEFAULT)
                .or_else(|_| time::OffsetDateTime::parse(&timestamp_str, &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap()))
                .map_err(|e| SourceError::internal(format!("invalid timestamp: {}", e)))?;
            let ts = UtcDateTime::from_offset_datetime(ts_offset)
                .map_err(|e| SourceError::internal(format!("timestamp not UTC: {}", e)))?;

            if let Ok(bar) = Bar::new(
                ts,
                bar_data.open,
                bar_data.high,
                bar_data.low,
                bar_data.close,
                bar_data.volume.map(|v| v as u64),
                None,
            ) {
                bars.push(bar);
            }
        }

        bars.reverse(); // Alpha Vantage returns newest first, we want oldest first
        Ok(BarSeries::new(req.symbol.clone(), req.interval, bars))
    }

    async fn fetch_real_fundamentals(
        &self,
        req: &FundamentalsRequest,
    ) -> Result<FundamentalsBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("alphavantage circuit breaker is open"));
        }

        let retry_delay = self.throttling.acquire().err();
        if let Some(delay) = retry_delay {
            return Err(SourceError::rate_limited(format!(
                "alphavantage free-tier limit exceeded; retry in {:.2}s",
                delay.as_secs_f64()
            )));
        }

        let as_of = UtcDateTime::now();
        let fundamentals = req
            .symbols
            .iter()
            .map(|symbol| Fundamental::new(symbol.clone(), as_of, None, None, None))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e: ValidationError| SourceError::internal(e.to_string()))?;

        Ok(FundamentalsBatch { fundamentals })
    }

    async fn execute_real_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("alphavantage circuit breaker is open"));
        }

        let retry_delay = self.throttling.acquire().err();
        if let Some(delay) = retry_delay {
            return Err(SourceError::rate_limited(format!(
                "alphavantage free-tier limit exceeded; retry in {:.2}s",
                delay.as_secs_f64()
            )));
        }

        let endpoint = format!(
            "https://www.alphavantage.co/query?function=SYMBOL_SEARCH&keywords={}&apikey={}",
            urlencoding::encode(&req.query),
            self.api_key
        );

        let request = HttpRequest::get(&endpoint).with_timeout_ms(5_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("alphavantage transport error: {}", e.message()))
        })?;

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "alphavantage returned status {}",
                response.status
            )));
        }

        self.throttling.complete_one();
        self.circuit_breaker.record_success();

        let search_response: AlphaVantageSearchResponse =
            serde_json::from_str(&response.body).map_err(|e| {
                SourceError::internal(format!("failed to parse search response: {}", e))
            })?;

        let results = search_response
            .best_matches
            .into_iter()
            .filter_map(|match_result| {
                let symbol = Symbol::parse(&match_result.symbol).ok()?;
                let asset_class = match match_result.match_type.as_str() {
                    "Equity" | "Common Stock" => AssetClass::Equity,
                    "ETF" | "Exchange Traded Fund" => AssetClass::Etf,
                    "Fund" => AssetClass::Fund,
                    "Index" => AssetClass::Index,
                    "Crypto" => AssetClass::Crypto,
                    "Currency" | "Forex" => AssetClass::Forex,
                    _ => AssetClass::Other,
                };

                Instrument::new(
                    symbol,
                    match_result.name,
                    None,
                    match_result.currency.unwrap_or_else(|| "USD".to_string()),
                    asset_class,
                    true,
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
                    "alphavantage bars request limit must be greater than zero",
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
                    "alphavantage fundamentals request requires at least one symbol",
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
                    "alphavantage search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "alphavantage search limit must be greater than zero",
                ));
            }

            self.execute_real_search(&req).await
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

// Alpha Vantage API response structures
#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageQuoteResponse {
    #[serde(rename = "Global Quote")]
    quote: Option<AlphaVantageQuoteData>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageQuoteData {
    #[serde(rename = "05. price")]
    price: f64,
}

/// Alpha Vantage returns time series with dynamic field names based on interval
/// We use a flexible JSON approach to handle this
#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageTimeSeriesResponse {
    #[serde(flatten)]
    time_series_data: std::collections::HashMap<String, serde_json::Value>,
}

impl AlphaVantageTimeSeriesResponse {
    /// Extract time series data regardless of the field name
    fn get_time_series(&self) -> Option<std::collections::BTreeMap<String, AlphaVantageTimeSeriesBar>> {
        // Look for any key that starts with "Time Series"
        for (key, value) in &self.time_series_data {
            if key.starts_with("Time Series") {
                if let Ok(series) = serde_json::from_value(value.clone()) {
                    return Some(series);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageTimeSeriesBar {
    #[serde(rename = "1. open")]
    open: f64,
    #[serde(rename = "2. high")]
    high: f64,
    #[serde(rename = "3. low")]
    low: f64,
    #[serde(rename = "4. close")]
    close: f64,
    #[serde(rename = "5. volume")]
    volume: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageSearchResponse {
    #[serde(rename = "bestMatches", default)]
    best_matches: Vec<AlphaVantageSearchMatch>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageSearchMatch {
    #[serde(rename = "1. symbol")]
    symbol: String,
    #[serde(rename = "2. name")]
    name: String,
    #[serde(rename = "3. type")]
    match_type: String,
    #[serde(rename = "8. currency", default)]
    currency: Option<String>,
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
        let adapter = AlphaVantageAdapter::with_http_client(client, "demo-key");
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
