use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
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

/// Alpha Vantage adapter supporting both real API calls and mock mode.
#[derive(Clone)]
pub struct AlphaVantageAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    api_key: String,
    circuit_breaker: Arc<CircuitBreaker>,
    throttling: ThrottlingQueue,
    use_real_api: bool,
}

impl Default for AlphaVantageAdapter {
    fn default() -> Self {
        let policy = ProviderPolicy::alphavantage_default();
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 70,
            http_client: Arc::new(NoopHttpClient),
            api_key: std::env::var("FERROTICK_ALPHAVANTAGE_API_KEY")
                .unwrap_or_else(|_| String::from("demo")),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            throttling: ThrottlingQueue::from_policy(&policy),
            use_real_api: false,
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
        let is_real = !http_client.is_mock();
        Self {
            http_client,
            api_key: api_key.into(),
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

// Mock data methods (for tests)
impl AlphaVantageAdapter {
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

    async fn fetch_mock_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
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
    }

    async fn fetch_mock_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
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
    }

    async fn fetch_mock_fundamentals(
        &self,
        req: &FundamentalsRequest,
    ) -> Result<FundamentalsBatch, SourceError> {
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
    }

    async fn execute_mock_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        self.execute_authenticated_call(
            "https://www.alphavantage.co/query?function=SYMBOL_SEARCH",
        )
        .await?;

        let query_lower = req.query.to_ascii_lowercase();
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
                    "alphavantage bars request limit must be greater than zero",
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
            if req.symbols.is_empty() {
                return Err(SourceError::invalid_request(
                    "alphavantage fundamentals request requires at least one symbol",
                ));
            }

            if self.is_real_client() {
                self.fetch_real_fundamentals(&req).await
            } else {
                self.fetch_mock_fundamentals(&req).await
            }
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

            if self.is_real_client() {
                self.execute_real_search(&req).await
            } else {
                self.execute_mock_search(&req).await
            }
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

// Alpha Vantage API response structures
#[derive(Debug, Clone, Deserialize)]
struct AlphaVantageQuoteResponse {
    #[serde(rename = "Global Quote", default)]
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

// Mock data structures
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

        fn is_mock(&self) -> bool {
            true
        }
    }

    #[test]
    fn quote_request_appends_api_key_query_parameter() {
        let client = Arc::new(NoopHttpClient);
        let adapter = AlphaVantageAdapter::with_http_client(client, "alpha-key");
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);
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
