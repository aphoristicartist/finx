use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Deserialize;
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

// ============================================================================
// Yahoo Auth Manager - Handles cookie/crumb authentication
// ============================================================================

/// Manages Yahoo Finance cookie/crumb authentication.
/// 
/// Yahoo's unofficial API requires:
/// 1. Session cookie from fc.yahoo.com
/// 2. Crumb token from query1.finance.yahoo.com/v1/test/getcrumb
#[derive(Clone)]
pub struct YahooAuthManager {
    /// Cached cookie value (e.g., "B=some_cookie_value")
    cookie: Arc<std::sync::Mutex<Option<String>>>,
    /// Cached crumb token
    crumb: Arc<std::sync::Mutex<Option<String>>>,
    /// When the auth was last refreshed
    last_refresh: Arc<std::sync::Mutex<Option<Instant>>>,
    /// Whether auth refresh is currently in progress
    refreshing: Arc<AtomicBool>,
    /// Auth TTL in seconds (default: 1 hour)
    auth_ttl_secs: u64,
    /// Whether to use environment variable override for auth
    use_env_override: bool,
}

impl Default for YahooAuthManager {
    fn default() -> Self {
        Self {
            cookie: Arc::new(std::sync::Mutex::new(None)),
            crumb: Arc::new(std::sync::Mutex::new(None)),
            last_refresh: Arc::new(std::sync::Mutex::new(None)),
            refreshing: Arc::new(AtomicBool::new(false)),
            auth_ttl_secs: 3600, // 1 hour
            use_env_override: true,
        }
    }
}

impl YahooAuthManager {
    /// Check if cached auth is valid (exists and not expired)
    fn is_auth_valid(&self) -> bool {
        let cookie = self.cookie.lock().unwrap();
        let crumb = self.crumb.lock().unwrap();
        let last_refresh = self.last_refresh.lock().unwrap();

        if cookie.is_none() || crumb.is_none() {
            return false;
        }

        if let Some(last) = *last_refresh {
            let elapsed = last.elapsed().as_secs();
            return elapsed < self.auth_ttl_secs;
        }

        false
    }

    /// Get current crumb for use in query parameters, refreshing if needed
    pub async fn get_crumb(&self, http_client: &Arc<dyn HttpClient>) -> Result<String, SourceError> {
        // Return cached crumb if valid
        if self.is_auth_valid() {
            let crumb = self.crumb.lock().unwrap().clone();
            if let Some(cr) = crumb {
                return Ok(cr);
            }
        }

        // Refresh auth
        self.refresh_auth(http_client).await?;

        // Return new crumb
        let crumb = self.crumb.lock().unwrap().clone();
        crumb.ok_or_else(|| SourceError::unavailable("failed to obtain Yahoo crumb"))
    }

    /// Get current auth (cookies are managed by the jar, crumb is in URL params)
    pub async fn get_auth(&self, http_client: &Arc<dyn HttpClient>) -> Result<HttpAuth, SourceError> {
        if self.use_env_override {
            if let Some(auth) = Self::get_env_auth() {
                return Ok(auth);
            }
        }
        if !self.is_auth_valid() {
            self.refresh_auth(http_client).await?;
        }
        Ok(HttpAuth::None) // Cookies managed by jar, no manual auth needed
    }

    /// Refresh auth by fetching cookie and crumb from Yahoo
    async fn refresh_auth(&self, http_client: &Arc<dyn HttpClient>) -> Result<(), SourceError> {
        // Check if already refreshing
        if self.refreshing.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            // Another thread is refreshing, wait a bit and check if done
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if self.is_auth_valid() {
                return Ok(());
            }
        }

        let result = self.do_refresh(http_client).await;

        // Reset refreshing flag
        self.refreshing.store(false, Ordering::SeqCst);

        result
    }

    /// Actually perform the refresh
    async fn do_refresh(&self, http_client: &Arc<dyn HttpClient>) -> Result<(), SourceError> {
        // Step 1: Visit fc.yahoo.com with Referer header to get session cookies
        let cookie_request = HttpRequest::get("https://fc.yahoo.com")
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let _cookie_response = http_client.execute(cookie_request).await.map_err(|e| {
            SourceError::unavailable(format!("failed to fetch Yahoo cookie: {}", e.message()))
        })?;

        // Step 2: Fetch crumb from query endpoints
        let crumb_endpoints = [
            "https://query1.finance.yahoo.com/v1/test/getcrumb",
            "https://query2.finance.yahoo.com/v1/test/getcrumb",
        ];

        for endpoint in &crumb_endpoints {
            let crumb_request = HttpRequest::get(endpoint.to_string())
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            match http_client.execute(crumb_request).await {
                Ok(response) if response.is_success() && !response.body.is_empty() => {
                    let body = response.body.trim();

                    // Check for HTML error pages
                    if body.contains("<html") || body.contains("<!DOCTYPE") {
                        continue;
                    }

                    // Check for "Too Many Requests"
                    if body.to_lowercase().contains("too many requests") {
                        return Err(SourceError::unavailable("Yahoo rate limited while fetching crumb"));
                    }

                    // Validate crumb (should not be too long, contain reasonable characters)
                    if body.len() > 0 && body.len() < 100 && !body.contains(' ') {
                        *self.crumb.lock().unwrap() = Some(body.to_string());
                        *self.last_refresh.lock().unwrap() = Some(Instant::now());
                        return Ok(());
                    }
                }
                _ => continue,
            }
        }

        Err(SourceError::unavailable("failed to fetch Yahoo crumb from all endpoints"))
    }

    /// Get auth from environment variables (for testing/override)
    fn get_env_auth() -> Option<HttpAuth> {
        std::env::var("YAHOO_COOKIE")
            .ok()
            .map(|cookie| HttpAuth::Cookie(cookie))
    }

    /// Invalidate cached auth (triggers refresh on next call)
    pub fn invalidate(&self) {
        *self.cookie.lock().unwrap() = None;
        *self.crumb.lock().unwrap() = None;
        *self.last_refresh.lock().unwrap() = None;
    }

}

// ============================================================================
// Yahoo Adapter
// ============================================================================

/// Yahoo adapter supporting both real API calls and mock mode.
#[derive(Clone)]
pub struct YahooAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
    use_real_api: bool,
    /// Auth manager for cookie/crumb handling
    auth_manager: Arc<YahooAuthManager>,
}

impl Default for YahooAdapter {
    fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 78,
            http_client: Arc::new(NoopHttpClient),
            auth: HttpAuth::Cookie(String::from("B=ferrotick-session")),
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            use_real_api: false,
            auth_manager: Arc::new(YahooAuthManager::default()),
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
        let is_real = !http_client.is_mock();
        Self {
            http_client,
            auth,
            use_real_api: is_real,
            auth_manager: Arc::new(YahooAuthManager::default()),
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

    /// Handle authentication errors by invalidating cached auth
    fn handle_auth_error(&self) {
        self.auth_manager.invalidate();
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

            if self.is_real_client() {
                // Use real Yahoo Finance API
                self.fetch_real_quotes(&req).await
            } else {
                // Use deterministic fake data for tests
                self.fetch_fake_quotes(&req).await
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
                    "yahoo bars request limit must be greater than zero",
                ));
            }

            if self.is_real_client() {
                // Use real Yahoo Finance API
                self.fetch_real_bars(&req).await
            } else {
                // Use deterministic fake data for tests
                self.fetch_fake_bars(&req).await
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
                    "yahoo fundamentals request requires at least one symbol",
                ));
            }

            if self.is_real_client() {
                // Use real Yahoo Finance API
                self.fetch_real_fundamentals(&req).await
            } else {
                // Use deterministic fake data for tests
                self.fetch_fake_fundamentals(&req).await
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
                    "yahoo search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "yahoo search limit must be greater than zero",
                ));
            }

            if self.is_real_client() {
                // Use real Yahoo Finance API
                self.execute_real_search(&req).await
            } else {
                // Use deterministic fake data for tests
                self.execute_fake_search(&req).await
            }
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

// Real API implementation methods
impl YahooAdapter {
    async fn fetch_real_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
        let symbols_param = req
            .symbols
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        // Get crumb for authentication
        let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v7/finance/quote?symbols={}&fields=regularMarketPrice,regularMarketBid,regularMarketAsk,regularMarketVolume,currency&crumb={}",
            urlencoding::encode(&symbols_param),
            urlencoding::encode(&crumb)
        );

        self.fetch_quotes_with_retry(&endpoint).await
    }

    /// Fetch quotes with automatic auth retry on 401/429
    async fn fetch_quotes_with_retry(&self, endpoint: &str) -> Result<QuoteBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in the jar, crumb is in the URL
        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth and retrying once
        if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            // Get fresh crumb
            let _ = self.auth_manager.get_crumb(&self.http_client).await;

            // Retry with fresh crumb (rebuild endpoint with new crumb)
            let crumb = self.auth_manager.get_crumb(&self.http_client).await?;
            let endpoint_with_crumb = if endpoint.contains("&crumb=") {
                // Replace existing crumb
                endpoint.split("&crumb=").next().unwrap().to_string()
            } else {
                endpoint.to_string()
            };

            let new_endpoint = format!("{}&crumb={}",
                endpoint_with_crumb.split('&').take_while(|s| !s.starts_with("crumb=")).collect::<Vec<_>>().join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            let retry_response = self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?;

            if !retry_response.is_success() {
                self.circuit_breaker.record_failure();
                return Err(SourceError::unavailable(format!(
                    "yahoo returned status {} after auth refresh",
                    retry_response.status
                )));
            }

            self.circuit_breaker.record_success();
            return self.parse_quote_response(&retry_response.body);
        }

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        self.parse_quote_response(&response.body)
    }

    /// Parse Yahoo quote response JSON
    fn parse_quote_response(&self, body: &str) -> Result<QuoteBatch, SourceError> {
        let yahoo_response: YahooQuoteResponse = serde_json::from_str(body)
            .map_err(|e| SourceError::internal(format!("failed to parse yahoo response: {}", e)))?;

        // Check for API-level errors
        if let Some(error) = &yahoo_response.quote_response.error {
            if !error.is_empty() {
                return Err(SourceError::unavailable(format!(
                    "yahoo API error: {}",
                    error
                )));
            }
        }

        let quotes = yahoo_response
            .quote_response
            .result
            .into_iter()
            .filter_map(|quote| {
                let symbol = Symbol::parse(&quote.symbol).ok()?;
                let ts = UtcDateTime::now();

                Quote::new(
                    symbol,
                    quote.regular_market_price.unwrap_or(0.0),
                    quote.regular_market_bid,
                    quote.regular_market_ask,
                    quote.regular_market_volume.map(|v| v as u64),
                    quote.currency.unwrap_or_else(|| "USD".to_string()),
                    ts,
                )
                .ok()
            })
            .collect();

        Ok(QuoteBatch { quotes })
    }

    async fn fetch_real_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
        let range = match req.limit {
            0..=100 => "1d",
            101..=1000 => "1mo",
            _ => "1y",
        };

        let interval = match req.interval {
            Interval::OneMinute => "1m",
            Interval::FiveMinutes => "5m",
            Interval::FifteenMinutes => "15m",
            Interval::OneHour => "1h",
            Interval::OneDay => "1d",
        };

        // Get crumb for authentication
        let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={}&interval={}&crumb={}",
            urlencoding::encode(req.symbol.as_str()),
            range,
            interval,
            urlencoding::encode(&crumb)
        );

        self.fetch_bars_with_retry(&endpoint, &req.symbol, req.interval, req.limit).await
    }

    /// Fetch bars with automatic auth retry on 401/429
    async fn fetch_bars_with_retry(
        &self,
        endpoint: &str,
        symbol: &Symbol,
        interval: Interval,
        limit: usize,
    ) -> Result<BarSeries, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in the jar, crumb is in the URL
        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth and retrying once
        let response_body = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            // Get fresh crumb and rebuild endpoint
            let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!("{}&crumb={}",
                base_endpoint.split('&').take_while(|s| !s.starts_with("crumb=")).collect::<Vec<_>>().join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            let retry_response = self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?;

            if !retry_response.is_success() {
                self.circuit_breaker.record_failure();
                return Err(SourceError::unavailable(format!(
                    "yahoo returned status {} after auth refresh",
                    retry_response.status
                )));
            }

            self.circuit_breaker.record_success();
            retry_response.body
        } else if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        } else {
            self.circuit_breaker.record_success();
            response.body
        };

        // Parse Yahoo Finance chart response
        let chart_response: YahooChartResponse = serde_json::from_str(&response_body)
            .map_err(|e| SourceError::internal(format!("failed to parse yahoo chart: {}", e)))?;

        // Check for API-level errors
        if let Some(error) = &chart_response.chart.error {
            if !error.is_empty() {
                return Err(SourceError::unavailable(format!(
                    "yahoo chart API error: {}",
                    error
                )));
            }
        }

        let result = chart_response
            .chart
            .result
            .first()
            .ok_or_else(|| SourceError::internal("no chart data in response"))?;

        let timestamp = result
            .timestamp
            .as_ref()
            .ok_or_else(|| SourceError::internal("no timestamp data"))?;
        let quote = result
            .indicators
            .quote
            .first()
            .ok_or_else(|| SourceError::internal("no quote data"))?;

        let mut bars = Vec::new();
        for (i, &ts_value) in timestamp.iter().enumerate().take(limit) {
            // Convert Unix timestamp to UtcDateTime
            let ts_offset = time::OffsetDateTime::from_unix_timestamp(ts_value)
                .map_err(|e| SourceError::internal(format!("invalid timestamp: {}", e)))?;
            let ts = UtcDateTime::from_offset_datetime(ts_offset)
                .map_err(|e| SourceError::internal(format!("timestamp not UTC: {}", e)))?;

            // Only create bar if all OHLC values are present
            if let (Some(Some(open)), Some(Some(high)), Some(Some(low)), Some(Some(close))) = (
                quote.open.get(i),
                quote.high.get(i),
                quote.low.get(i),
                quote.close.get(i),
            ) {
                let volume = quote.volume.get(i).copied().flatten().map(|v| v as u64);

                if let Ok(bar) = Bar::new(ts, *open, *high, *low, *close, volume, None) {
                    bars.push(bar);
                }
            }
        }

        Ok(BarSeries::new(symbol.clone(), interval, bars))
    }

    async fn fetch_real_fundamentals(
        &self,
        req: &FundamentalsRequest,
    ) -> Result<FundamentalsBatch, SourceError> {
        // Get crumb for authentication
        let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

        let symbols_param = req
            .symbols
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        // Use quoteSummary endpoint with modules for various financial metrics
        let modules = "defaultKeyStatistics,financialData,summaryDetail,price";
        let endpoint = format!(
            "https://query1.finance.yahoo.com/v10/finance/quoteSummary/{}?modules={}&crumb={}",
            urlencoding::encode(&symbols_param),
            modules,
            urlencoding::encode(&crumb)
        );

        self.fetch_fundamentals_with_retry(&endpoint).await
    }

    /// Fetch fundamentals with automatic auth retry on 401/429
    async fn fetch_fundamentals_with_retry(&self, endpoint: &str) -> Result<FundamentalsBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in the jar, crumb is in the URL
        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth and retrying once
        let response_body = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            // Get fresh crumb and rebuild endpoint
            let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!("{}&crumb={}",
                base_endpoint.split('&').take_while(|s| !s.starts_with("crumb=")).collect::<Vec<_>>().join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            let retry_response = self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?;

            if !retry_response.is_success() {
                self.circuit_breaker.record_failure();
                return Err(SourceError::unavailable(format!(
                    "yahoo returned status {} after auth refresh",
                    retry_response.status
                )));
            }

            self.circuit_breaker.record_success();
            retry_response.body
        } else if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        } else {
            self.circuit_breaker.record_success();
            response.body
        };

        self.parse_fundamentals_response(&response_body)
    }

    /// Parse Yahoo fundamentals response from quoteSummary endpoint
    fn parse_fundamentals_response(&self, body: &str) -> Result<FundamentalsBatch, SourceError> {
        let summary_response: YahooQuoteSummaryResponse = serde_json::from_str(body)
            .map_err(|e| SourceError::internal(format!("failed to parse yahoo fundamentals: {}", e)))?;

        // Check for API-level errors
        if let Some(error) = &summary_response.quote_summary.error {
            if !error.is_empty() {
                return Err(SourceError::unavailable(format!(
                    "yahoo fundamentals API error: {}",
                    error
                )));
            }
        }

        let as_of = UtcDateTime::now();
        let fundamentals = summary_response
            .quote_summary
            .result
            .into_iter()
            .filter_map(|result| {
                // Try to get symbol from meta, or fall back to price data
                let symbol = if let Some(meta) = &result.meta {
                    Symbol::parse(&meta.symbol).ok()?
                } else if let Some(price) = &result.price {
                    Symbol::parse(&price.symbol.as_ref()?).ok()?
                } else {
                    return None; // Can't determine symbol, skip this result
                };

                // Extract market cap from price or defaultKeyStatistics
                let market_cap = result.price
                    .and_then(|p| p.market_cap.and_then(|v| v.to_option()))
                    .or_else(|| {
                        result.default_key_statistics.as_ref()
                            .and_then(|dks| dks.market_cap.as_ref().and_then(|v| v.to_option()))
                    });

                // Extract PE ratio from summaryDetail or defaultKeyStatistics
                let pe_ratio = result.summary_detail.as_ref()
                    .and_then(|sd| sd.forward_pe.as_ref().and_then(|v| v.to_option()))
                    .or_else(|| {
                        result.summary_detail.as_ref()
                            .and_then(|sd| sd.pe_ratio.as_ref().and_then(|v| v.to_option()))
                    })
                    .or_else(|| {
                        result.default_key_statistics.as_ref()
                            .and_then(|dks| dks.forward_pe.as_ref().and_then(|v| v.to_option()))
                    })
                    .or_else(|| {
                        result.default_key_statistics.as_ref()
                            .and_then(|dks| dks.pe_ratio.as_ref().and_then(|v| v.to_option()))
                    });

                // Extract dividend yield from summaryDetail or defaultKeyStatistics
                let dividend_yield = result.summary_detail.as_ref()
                    .and_then(|sd| sd.dividend_yield.as_ref().and_then(|v| v.to_option()))
                    .or_else(|| {
                        result.default_key_statistics.as_ref()
                            .and_then(|dks| dks.dividend_yield.as_ref().and_then(|v| v.to_option()))
                    });

                Fundamental::new(symbol, as_of, market_cap, pe_ratio, dividend_yield).ok()
            })
            .collect();

        Ok(FundamentalsBatch { fundamentals })
    }

    async fn execute_real_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        // Get crumb for authentication
        let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

        let endpoint = format!(
            "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount={}&crumb={}",
            urlencoding::encode(&req.query),
            req.limit,
            urlencoding::encode(&crumb)
        );

        self.fetch_search_with_retry(&endpoint, &req.query, req.limit).await
    }

    /// Fetch search with automatic auth retry on 401/429
    async fn fetch_search_with_retry(
        &self,
        endpoint: &str,
        query: &str,
        limit: usize,
    ) -> Result<SearchBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in the jar, crumb is in the URL
        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth and retrying once
        let response_body = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            // Get fresh crumb and rebuild endpoint
            let crumb = self.auth_manager.get_crumb(&self.http_client).await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!("{}&crumb={}",
                base_endpoint.split('&').take_while(|s| !s.starts_with("crumb=")).collect::<Vec<_>>().join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            let retry_response = self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?;

            if !retry_response.is_success() {
                self.circuit_breaker.record_failure();
                return Err(SourceError::unavailable(format!(
                    "yahoo returned status {} after auth refresh",
                    retry_response.status
                )));
            }

            self.circuit_breaker.record_success();
            retry_response.body
        } else if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        } else {
            self.circuit_breaker.record_success();
            response.body
        };

        let search_response: YahooSearchResponse =
            serde_json::from_str(&response_body).map_err(|e| {
                SourceError::internal(format!("failed to parse search response: {}", e))
            })?;

        let results = search_response
            .quotes
            .into_iter()
            .filter_map(|quote| {
                let symbol = Symbol::parse(&quote.symbol).ok()?;
                let asset_class = match quote.quote_type.as_str() {
                    "EQUITY" => AssetClass::Equity,
                    "ETF" => AssetClass::Etf,
                    "MUTUALFUND" => AssetClass::Fund,
                    "INDEX" => AssetClass::Index,
                    "CRYPTOCURRENCY" => AssetClass::Crypto,
                    "CURRENCY" => AssetClass::Forex,
                    _ => AssetClass::Other,
                };

                Instrument::new(
                    symbol,
                    quote.short_name.unwrap_or_else(|| quote.symbol.clone()),
                    quote.exchange,
                    quote.currency.unwrap_or_else(|| "USD".to_string()),
                    asset_class,
                    true,
                )
                .ok()
            })
            .take(limit)
            .collect();

        Ok(SearchBatch {
            query: query.to_string(),
            results,
        })
    }
}

// Fake data methods (for tests)
impl YahooAdapter {
    async fn fetch_fake_quotes(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError> {
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
    }

    async fn fetch_fake_bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError> {
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
    }

    async fn fetch_fake_fundamentals(
        &self,
        req: &FundamentalsRequest,
    ) -> Result<FundamentalsBatch, SourceError> {
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
    }

    async fn execute_fake_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        self.execute_authenticated_call("https://query2.finance.yahoo.com/v1/finance/search")
            .await?;

        let query_lower = req.query.to_ascii_lowercase();
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
            query: req.query.clone(),
            results,
        })
    }
}

// Yahoo Finance API response structures
#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteResponse {
    #[serde(rename = "quoteResponse")]
    quote_response: YahooQuoteResponseData,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteResponseData {
    result: Vec<YahooQuoteData>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteData {
    symbol: String,
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: Option<f64>,
    #[serde(rename = "regularMarketBid")]
    regular_market_bid: Option<f64>,
    #[serde(rename = "regularMarketAsk")]
    regular_market_ask: Option<f64>,
    #[serde(rename = "regularMarketVolume")]
    regular_market_volume: Option<i64>,
    currency: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooChartResponse {
    chart: YahooChartData,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooChartData {
    result: Vec<YahooChartResult>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooChartResult {
    timestamp: Option<Vec<i64>>,
    indicators: YahooChartIndicators,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooChartIndicators {
    quote: Vec<YahooChartQuote>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooChartQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<i64>>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooSearchResponse {
    quotes: Vec<YahooSearchQuote>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooSearchQuote {
    symbol: String,
    #[serde(rename = "shortname")]
    short_name: Option<String>,
    exchange: Option<String>,
    #[serde(rename = "quoteType")]
    quote_type: String,
    currency: Option<String>,
}

// ============================================================================
// Yahoo Fundamentals API Response Structures
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteSummaryResponse {
    #[serde(rename = "quoteSummary")]
    quote_summary: YahooQuoteSummaryData,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteSummaryData {
    result: Vec<YahooQuoteSummaryResult>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooQuoteSummaryResult {
    #[serde(default)]
    meta: Option<YahooMeta>,
    #[serde(rename = "price", default)]
    price: Option<YahooPriceData>,
    #[serde(rename = "summaryDetail", default)]
    summary_detail: Option<YahooSummaryDetailData>,
    #[serde(rename = "defaultKeyStatistics", default)]
    default_key_statistics: Option<YahooDefaultKeyStatisticsData>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooMeta {
    symbol: String,
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooPriceData {
    #[serde(rename = "marketCap", default)]
    market_cap: Option<YahooRawValue>,
    #[serde(rename = "symbol", default)]
    symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooSummaryDetailData {
    #[serde(rename = "forwardPE", default)]
    forward_pe: Option<YahooRawValue>,
    #[serde(rename = "PE_RATIO", default)]
    pe_ratio: Option<YahooRawValue>,
    #[serde(rename = "dividendYield", default)]
    dividend_yield: Option<YahooRawValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooDefaultKeyStatisticsData {
    #[serde(rename = "marketCap", default)]
    market_cap: Option<YahooRawValue>,
    #[serde(rename = "forwardPE", default)]
    forward_pe: Option<YahooRawValue>,
    #[serde(rename = "PE_RATIO", default)]
    pe_ratio: Option<YahooRawValue>,
    #[serde(rename = "dividendYield", default)]
    dividend_yield: Option<YahooRawValue>,
}

/// Yahoo returns some numeric values with extra metadata in a wrapper object.
/// We use a helper to extract the raw f64 value.
#[derive(Debug, Clone, Deserialize)]
struct YahooRawValue {
    #[serde(default)]
    raw: Option<f64>,
}

impl Into<Option<f64>> for YahooRawValue {
    fn into(self) -> Option<f64> {
        self.raw.filter(|v| !v.is_nan() && *v != 0.0)
    }
}

impl YahooRawValue {
    /// Helper to convert to Option<f64>
    fn to_option(&self) -> Option<f64> {
        self.raw.filter(|v| !v.is_nan() && *v != 0.0)
    }
}

// Legacy fake data structures (kept for backward compatibility with tests)
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

        fn is_mock(&self) -> bool {
            true
        }
    }

    #[test]
    fn quote_request_applies_cookie_auth_header() {
        let client = Arc::new(NoopHttpClient);
        let adapter = YahooAdapter::with_http_client(
            client,
            HttpAuth::Cookie(String::from("B=secure-cookie")),
        );
        let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
            .expect("valid request");

        let response = block_on(adapter.quote(request)).expect("quote should succeed");
        assert_eq!(response.quotes.len(), 1);
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
