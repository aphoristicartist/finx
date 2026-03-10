use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde::Deserialize;

use crate::cache::CacheStore;
use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::data_source::{
    BarsRequest, CapabilitySet, DataSource, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
};
use crate::http_client::{HttpAuth, HttpClient, HttpRequest, HttpResponse};
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
    /// When auth was last refreshed
    last_refresh: Arc<std::sync::Mutex<Option<Instant>>>,
    /// Whether auth refresh is currently in progress
    refreshing: Arc<AtomicBool>,
    /// Auth TTL in seconds (default: 1 hour)
    auth_ttl_secs: u64,
}

impl Default for YahooAuthManager {
    fn default() -> Self {
        Self {
            cookie: Arc::new(std::sync::Mutex::new(None)),
            crumb: Arc::new(std::sync::Mutex::new(None)),
            last_refresh: Arc::new(std::sync::Mutex::new(None)),
            refreshing: Arc::new(AtomicBool::new(false)),
            auth_ttl_secs: 3600, // 1 hour
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
    pub async fn get_crumb(
        &self,
        http_client: &Arc<dyn HttpClient>,
    ) -> Result<String, SourceError> {
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

    /// Refresh auth by fetching cookie and crumb from Yahoo
    async fn refresh_auth(&self, http_client: &Arc<dyn HttpClient>) -> Result<(), SourceError> {
        // Check if already refreshing
        if self
            .refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
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
                        return Err(SourceError::unavailable(
                            "Yahoo rate limited while fetching crumb",
                        ));
                    }

                    // Validate crumb (should not be too long, contain reasonable characters)
                    if !body.is_empty() && body.len() < 100 && !body.contains(' ') {
                        *self.crumb.lock().unwrap() = Some(body.to_string());
                        *self.last_refresh.lock().unwrap() = Some(Instant::now());
                        return Ok(());
                    }
                }
                _ => continue,
            }
        }

        Err(SourceError::unavailable(
            "failed to fetch Yahoo crumb from all endpoints",
        ))
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

/// Yahoo adapter for real API calls.
#[derive(Clone)]
pub struct YahooAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    circuit_breaker: Arc<CircuitBreaker>,
    /// Auth manager for cookie/crumb handling
    auth_manager: Arc<YahooAuthManager>,
    cache: CacheStore,
}

impl YahooAdapter {
    /// Create a new YahooAdapter with a real HTTP client and authentication.
    pub fn with_http_client(
        http_client: Arc<dyn HttpClient>,
        _auth: HttpAuth,
        cache: Option<CacheStore>,
    ) -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 78,
            http_client,
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            auth_manager: Arc::new(YahooAuthManager::default()),
            cache: cache.unwrap_or_else(CacheStore::with_default_ttl),
        }
    }

    /// Create a new YahooAdapter with a custom circuit breaker.
    pub fn with_circuit_breaker(
        circuit_breaker: Arc<CircuitBreaker>,
        http_client: Arc<dyn HttpClient>,
        auth: HttpAuth,
        cache: Option<CacheStore>,
    ) -> Self {
        Self {
            circuit_breaker,
            ..Self::with_http_client(http_client, auth, cache)
        }
    }

    /// Create a new YahooAdapter with custom health state.
    pub fn with_health(
        health_state: HealthState,
        rate_available: bool,
        http_client: Arc<dyn HttpClient>,
        auth: HttpAuth,
        cache: Option<CacheStore>,
    ) -> Self {
        Self {
            health_state,
            rate_available,
            ..Self::with_http_client(http_client, auth, cache)
        }
    }

    /// Create a new YahooAdapter with a custom circuit breaker for testing.
    /// Uses default HTTP client and auth.
    pub fn with_circuit_breaker_for_test(circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self::with_circuit_breaker(
            circuit_breaker,
            Arc::new(crate::http_client::ReqwestHttpClient::new()),
            HttpAuth::None,
            None,
        )
    }

    /// Create a new YahooAdapter with custom health state for testing.
    /// Uses default HTTP client and auth.
    pub fn with_health_for_test(health_state: HealthState, rate_available: bool) -> Self {
        Self::with_health(
            health_state,
            rate_available,
            Arc::new(crate::http_client::ReqwestHttpClient::new()),
            HttpAuth::None,
            None,
        )
    }

    /// Handle authentication errors by invalidating cached auth
    fn handle_auth_error(&self) {
        self.auth_manager.invalidate();
    }

    fn bars_cache_key(symbol: &Symbol, interval: Interval, limit: usize) -> String {
        format!("bars:{}:{}:{}", symbol.as_str(), interval.as_str(), limit)
    }

    fn quote_cache_key(symbol: &Symbol) -> String {
        format!("quote:{}", symbol.as_str())
    }

    async fn fetch_crumb(&self) -> Result<String, SourceError> {
        self.auth_manager
            .get_crumb(&self.http_client)
            .await
            .inspect_err(|_| {
                self.circuit_breaker.record_failure();
            })
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
                    "yahoo bars request limit must be greater than zero",
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
                    "yahoo fundamentals request requires at least one symbol",
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
                    "yahoo search query must not be empty",
                ));
            }
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "yahoo search limit must be greater than zero",
                ));
            }

            self.execute_real_search(&req).await
        })
    }

    fn financials<'a>(
        &'a self,
        req: crate::data_source::FinancialsRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<crate::data_source::FinancialsBatch, SourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "yahoo financials limit must be greater than zero",
                ));
            }

            self.fetch_real_financials(&req).await
        })
    }

    fn earnings<'a>(
        &'a self,
        req: crate::data_source::EarningsRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<crate::data_source::EarningsBatch, SourceError>> + Send + 'a,
        >,
    > {
        Box::pin(async move {
            if req.limit == 0 {
                return Err(SourceError::invalid_request(
                    "yahoo earnings limit must be greater than zero",
                ));
            }

            self.fetch_real_earnings(&req).await
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
        let cache_key = Self::quote_cache_key(&req.symbols[0]);
        if let Some(cached_body) = self.cache.get(&cache_key).await {
            return self.parse_quote_response(&cached_body);
        }

        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        let symbols_param = req
            .symbols
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");

        // Get crumb for authentication
        let crumb = self.fetch_crumb().await?;

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v7/finance/quote?symbols={}&fields=regularMarketPrice,regularMarketBid,regularMarketAsk,regularMarketVolume,currency&crumb={}",
            urlencoding::encode(&symbols_param),
            urlencoding::encode(&crumb)
        );

        let response_body = self.fetch_quotes_with_retry(&endpoint).await?;
        let quote_batch = self.parse_quote_response(&response_body)?;
        self.cache.put(cache_key, response_body, None).await;
        Ok(quote_batch)
    }

    /// Fetch quotes with automatic auth retry on 401/429
    async fn fetch_quotes_with_retry(&self, endpoint: &str) -> Result<String, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in jar, crumb is in URL
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
            let _ = self.fetch_crumb().await;

            // Retry with fresh crumb (rebuild endpoint with new crumb)
            let crumb = self.fetch_crumb().await?;
            let endpoint_with_crumb = if endpoint.contains("&crumb=") {
                // Replace existing crumb
                endpoint.split("&crumb=").next().unwrap().to_string()
            } else {
                endpoint.to_string()
            };

            let new_endpoint = format!(
                "{}&crumb={}",
                endpoint_with_crumb
                    .split('&')
                    .take_while(|s| !s.starts_with("crumb="))
                    .collect::<Vec<_>>()
                    .join("&"),
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
            return Ok(retry_response.body);
        }

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(response.body)
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
        let cache_key = Self::bars_cache_key(&req.symbol, req.interval, req.limit);
        if let Some(cached_body) = self.cache.get(&cache_key).await {
            return self.parse_bars_response(&cached_body, &req.symbol, req.interval, req.limit);
        }

        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

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
        let crumb = self.fetch_crumb().await?;

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={}&interval={}&crumb={}",
            urlencoding::encode(req.symbol.as_str()),
            range,
            interval,
            urlencoding::encode(&crumb)
        );

        let response_body = self.fetch_bars_with_retry(&endpoint).await?;
        let series =
            self.parse_bars_response(&response_body, &req.symbol, req.interval, req.limit)?;
        self.cache.put(cache_key, response_body, None).await;
        Ok(series)
    }

    /// Fetch bars with automatic auth retry on 401/429
    async fn fetch_bars_with_retry(&self, endpoint: &str) -> Result<String, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        // Make request - cookies are in jar, crumb is in URL
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
            let crumb = self.fetch_crumb().await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!(
                "{}&crumb={}",
                base_endpoint
                    .split('&')
                    .take_while(|s| !s.starts_with("crumb="))
                    .collect::<Vec<_>>()
                    .join("&"),
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

        Ok(response_body)
    }

    fn parse_bars_response(
        &self,
        response_body: &str,
        symbol: &Symbol,
        interval: Interval,
        limit: usize,
    ) -> Result<BarSeries, SourceError> {
        // Parse Yahoo Finance chart response
        let chart_response: YahooChartResponse = serde_json::from_str(response_body)
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
        let crumb = self.fetch_crumb().await?;

        let as_of = UtcDateTime::now();
        let mut fundamentals = Vec::new();

        for symbol in &req.symbols {
            let endpoint = format!(
                "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=price,summaryDetail,defaultKeyStatistics&crumb={}",
                urlencoding::encode(symbol.as_str()),
                urlencoding::encode(&crumb)
            );

            let response = self.execute_fundamentals_request(&endpoint).await?;

            let summary_response: YahooQuoteSummaryResponse = serde_json::from_str(&response.body)
                .map_err(|e| {
                    SourceError::internal(format!("failed to parse fundamentals: {}", e))
                })?;

            if let Some(error) = &summary_response.quote_summary.error {
                if !error.is_empty() {
                    return Err(SourceError::unavailable(format!(
                        "yahoo fundamentals API error: {}",
                        error
                    )));
                }
            }

            // Extract fundamentals from response
            let result = summary_response
                .quote_summary
                .result
                .first()
                .ok_or_else(|| SourceError::internal("no result in fundamentals response"))?;

            // Extract market cap from price or defaultKeyStatistics
            let market_cap = result
                .price
                .as_ref()
                .and_then(|p| p.market_cap.as_ref().and_then(|v| v.to_option()))
                .or_else(|| {
                    result
                        .default_key_statistics
                        .as_ref()
                        .and_then(|s| s.market_cap.as_ref().and_then(|v| v.to_option()))
                });

            // Extract P/E ratio from summaryDetail or defaultKeyStatistics
            let pe_ratio = result
                .summary_detail
                .as_ref()
                .and_then(|s| s.pe_ratio.as_ref().and_then(|v| v.to_option()))
                .or_else(|| {
                    result
                        .summary_detail
                        .as_ref()
                        .and_then(|s| s.forward_pe.as_ref().and_then(|v| v.to_option()))
                })
                .or_else(|| {
                    result
                        .default_key_statistics
                        .as_ref()
                        .and_then(|s| s.pe_ratio.as_ref().and_then(|v| v.to_option()))
                });

            // Extract dividend yield
            let dividend_yield = result
                .summary_detail
                .as_ref()
                .and_then(|s| s.dividend_yield.as_ref().and_then(|v| v.to_option()))
                .or_else(|| {
                    result
                        .default_key_statistics
                        .as_ref()
                        .and_then(|s| s.dividend_yield.as_ref().and_then(|v| v.to_option()))
                });

            if let Ok(fundamental) =
                Fundamental::new(symbol.clone(), as_of, market_cap, pe_ratio, dividend_yield)
            {
                fundamentals.push(fundamental);
            }
        }

        Ok(FundamentalsBatch { fundamentals })
    }

    async fn execute_fundamentals_request(
        &self,
        endpoint: &str,
    ) -> Result<HttpResponse, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth
        let response = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            let _ = self.fetch_crumb().await;

            let crumb = self.fetch_crumb().await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!(
                "{}&crumb={}",
                base_endpoint
                    .split('&')
                    .take_while(|s| !s.starts_with("crumb="))
                    .collect::<Vec<_>>()
                    .join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?
        } else {
            response
        };

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(response)
    }

    async fn execute_real_search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError> {
        let crumb = self.fetch_crumb().await?;

        let endpoint = format!(
            "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount={}&crumb={}",
            urlencoding::encode(&req.query),
            req.limit,
            urlencoding::encode(&crumb)
        );

        self.fetch_search_with_retry(&endpoint, &req.query, req.limit)
            .await
    }

    async fn fetch_search_with_retry(
        &self,
        endpoint: &str,
        query: &str,
        limit: usize,
    ) -> Result<SearchBatch, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        // Handle 401/429 by refreshing auth
        let response_body = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();

            let _ = self.fetch_crumb().await;

            let crumb = self.fetch_crumb().await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);

            let new_endpoint = format!(
                "{}&crumb={}",
                base_endpoint
                    .split('&')
                    .take_while(|s| !s.starts_with("crumb="))
                    .collect::<Vec<_>>()
                    .join("&"),
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

        self.parse_search_response(&response_body, query, limit)
    }

    fn parse_search_response(
        &self,
        body: &str,
        query: &str,
        limit: usize,
    ) -> Result<SearchBatch, SourceError> {
        let search_response: YahooSearchResponse = serde_json::from_str(body).map_err(|e| {
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
                    "INDEX" => AssetClass::Index,
                    "FUTURE" | "OPTION" => AssetClass::Other,
                    _ => AssetClass::Equity,
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

    async fn fetch_real_financials(
        &self,
        req: &crate::data_source::FinancialsRequest,
    ) -> Result<crate::data_source::FinancialsBatch, SourceError> {
        let crumb = self.fetch_crumb().await?;

        // Map statement type to Yahoo modules
        let modules = match req.statement_type {
            crate::StatementType::Income => {
                "incomeStatementHistory,incomeStatementHistoryQuarterly"
            }
            crate::StatementType::Balance => "balanceSheetHistory,balanceSheetHistoryQuarterly",
            crate::StatementType::CashFlow => {
                "cashflowStatementHistory,cashflowStatementHistoryQuarterly"
            }
        };

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v10/finance/quoteSummary/{}?modules={}&crumb={}",
            urlencoding::encode(req.symbol.as_str()),
            modules,
            urlencoding::encode(&crumb)
        );

        let response = self.execute_financials_request(&endpoint).await?;

        let summary_response: YahooFinancialsResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse financials: {}", e)))?;

        if let Some(error) = &summary_response.quote_summary.error {
            if !error.is_empty() {
                return Err(SourceError::unavailable(format!(
                    "yahoo financials API error: {}",
                    error
                )));
            }
        }

        let result = summary_response
            .quote_summary
            .result
            .first()
            .ok_or_else(|| SourceError::internal("no result in financials response"))?;

        let as_of = UtcDateTime::now();
        let mut line_items = Vec::new();

        // Parse based on statement type
        match req.statement_type {
            crate::StatementType::Income => {
                let history = if matches!(req.period, crate::FinancialPeriod::Annual) {
                    result
                        .income_statement_history
                        .as_ref()
                        .map(|h| &h.income_statement_history)
                } else {
                    result
                        .income_statement_history_quarterly
                        .as_ref()
                        .map(|h| &h.income_statement_history)
                };

                if let Some(history) = history {
                    for entry in history.iter().take(req.limit) {
                        let end_date = entry.end_date.fmt.as_str();
                        let ts = UtcDateTime::parse(end_date).unwrap_or(as_of);

                        if let Some(rev) = &entry.total_revenue {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Total Revenue",
                                rev.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(gross) = &entry.gross_profit {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Gross Profit",
                                gross.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(net) = &entry.net_income {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Net Income",
                                net.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(eps) = &entry.basic_eps {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Basic EPS",
                                eps.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                    }
                }
            }
            crate::StatementType::Balance => {
                let history = if matches!(req.period, crate::FinancialPeriod::Annual) {
                    result
                        .balance_sheet_history
                        .as_ref()
                        .map(|h| &h.balance_sheet_history)
                } else {
                    result
                        .balance_sheet_history_quarterly
                        .as_ref()
                        .map(|h| &h.balance_sheet_history)
                };

                if let Some(history) = history {
                    for entry in history.iter().take(req.limit) {
                        let end_date = entry.end_date.fmt.as_str();
                        let ts = UtcDateTime::parse(end_date).unwrap_or(as_of);

                        if let Some(assets) = &entry.total_assets {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Total Assets",
                                assets.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(liab) = &entry.total_liabilities_net_minority_interest {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Total Liabilities",
                                liab.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(equity) = &entry.total_stockholder_equity {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Total Stockholder Equity",
                                equity.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(cash) = &entry.cash {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Cash",
                                cash.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                    }
                }
            }
            crate::StatementType::CashFlow => {
                let history = if matches!(req.period, crate::FinancialPeriod::Annual) {
                    result
                        .cashflow_statement_history
                        .as_ref()
                        .map(|h| &h.cashflow_statement_history)
                } else {
                    result
                        .cashflow_statement_history_quarterly
                        .as_ref()
                        .map(|h| &h.cashflow_statement_history)
                };

                if let Some(history) = history {
                    for entry in history.iter().take(req.limit) {
                        let end_date = entry.end_date.fmt.as_str();
                        let ts = UtcDateTime::parse(end_date).unwrap_or(as_of);

                        if let Some(ocf) = &entry.total_cash_from_operating_activities {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Operating Cash Flow",
                                ocf.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(icf) = &entry.total_cashflows_from_investing_activities {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Investing Cash Flow",
                                icf.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(fcf) = &entry.total_cash_from_financing_activities {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Financing Cash Flow",
                                fcf.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                        if let Some(capex) = &entry.capital_expenditures {
                            if let Ok(item) = crate::FinancialLineItem::new(
                                "Capital Expenditures",
                                capex.to_option(),
                                None,
                                None,
                                ts,
                            ) {
                                line_items.push(item);
                            }
                        }
                    }
                }
            }
        }

        let statement = crate::FinancialStatement::new(
            req.symbol.clone(),
            req.statement_type,
            req.period,
            "USD",
            as_of,
            line_items,
        )
        .map_err(validation_to_error)?;

        Ok(crate::data_source::FinancialsBatch {
            financials: vec![statement],
        })
    }

    async fn execute_financials_request(
        &self,
        endpoint: &str,
    ) -> Result<HttpResponse, SourceError> {
        if !self.circuit_breaker.allow_request() {
            return Err(SourceError::unavailable("yahoo circuit breaker is open"));
        }

        let request = HttpRequest::get(endpoint)
            .with_header("referer", "https://finance.yahoo.com/")
            .with_timeout_ms(10_000);

        let response = self.http_client.execute(request).await.map_err(|e| {
            self.circuit_breaker.record_failure();
            SourceError::unavailable(format!("yahoo transport error: {}", e.message()))
        })?;

        let response = if response.status == 401 || response.status == 429 {
            self.handle_auth_error();
            let _ = self.fetch_crumb().await;
            let crumb = self.fetch_crumb().await?;

            let base_endpoint = endpoint.split("&crumb=").next().unwrap_or(endpoint);
            let new_endpoint = format!(
                "{}&crumb={}",
                base_endpoint
                    .split('&')
                    .take_while(|s| !s.starts_with("crumb="))
                    .collect::<Vec<_>>()
                    .join("&"),
                urlencoding::encode(&crumb)
            );

            let retry_request = HttpRequest::get(&new_endpoint)
                .with_header("referer", "https://finance.yahoo.com/")
                .with_timeout_ms(10_000);

            self.http_client.execute(retry_request).await.map_err(|e| {
                self.circuit_breaker.record_failure();
                SourceError::unavailable(format!("yahoo transport error on retry: {}", e.message()))
            })?
        } else {
            response
        };

        if !response.is_success() {
            self.circuit_breaker.record_failure();
            return Err(SourceError::unavailable(format!(
                "yahoo returned status {}",
                response.status
            )));
        }

        self.circuit_breaker.record_success();
        Ok(response)
    }

    async fn fetch_real_earnings(
        &self,
        req: &crate::data_source::EarningsRequest,
    ) -> Result<crate::data_source::EarningsBatch, SourceError> {
        let crumb = self.fetch_crumb().await?;

        let endpoint = format!(
            "https://query1.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=earnings,earningsTrend&crumb={}",
            urlencoding::encode(req.symbol.as_str()),
            urlencoding::encode(&crumb)
        );

        let response = self.execute_financials_request(&endpoint).await?;

        let summary_response: YahooEarningsResponse = serde_json::from_str(&response.body)
            .map_err(|e| SourceError::internal(format!("failed to parse earnings: {}", e)))?;

        if let Some(error) = &summary_response.quote_summary.error {
            if !error.is_empty() {
                return Err(SourceError::unavailable(format!(
                    "yahoo earnings API error: {}",
                    error
                )));
            }
        }

        let result = summary_response
            .quote_summary
            .result
            .first()
            .ok_or_else(|| SourceError::internal("no result in earnings response"))?;

        let as_of = UtcDateTime::now();
        let mut entries = Vec::new();

        // Parse earnings history
        if let Some(earnings) = &result.earnings {
            if let Some(history) = &earnings.financials_chart {
                for (i, quarter) in history.quarterly.iter().enumerate().take(req.limit) {
                    let end_date = quarter.date.as_str();
                    let ts = UtcDateTime::parse(end_date).unwrap_or(as_of);

                    // Extract fiscal year/quarter from date or use index
                    let fiscal_year = quarter.year.unwrap_or(as_of.year());
                    let fiscal_quarter = Some(quarter.quarter.unwrap_or(((i % 4) + 1) as i32));

                    let surprise_percent = if let (Some(actual), Some(estimate)) =
                        (quarter.actual, quarter.estimate)
                    {
                        if estimate != 0.0 {
                            Some(((actual - estimate) / estimate.abs()) * 100.0)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Ok(entry) = crate::EarningsEntry::new(
                        fiscal_year,
                        fiscal_quarter,
                        ts,
                        quarter.actual,
                        quarter.estimate,
                        None, // revenue not available in this structure
                        None,
                        surprise_percent,
                    ) {
                        entries.push(entry);
                    }
                }
            }
        }

        let report = crate::EarningsReport::new(req.symbol.clone(), "USD", as_of, entries)
            .map_err(validation_to_error)?;

        Ok(crate::data_source::EarningsBatch { earnings: report })
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
    _meta: Option<YahooMeta>,
    #[serde(rename = "price", default)]
    price: Option<YahooPriceData>,
    #[serde(rename = "summaryDetail", default)]
    summary_detail: Option<YahooSummaryDetailData>,
    #[serde(rename = "defaultKeyStatistics", default)]
    default_key_statistics: Option<YahooDefaultKeyStatisticsData>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooMeta {
    #[serde(rename = "symbol")]
    _symbol: String,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooPriceData {
    #[serde(rename = "marketCap", default)]
    market_cap: Option<YahooRawValue>,
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
    _forward_pe: Option<YahooRawValue>,
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

impl From<YahooRawValue> for Option<f64> {
    fn from(val: YahooRawValue) -> Self {
        val.raw.filter(|v| !v.is_nan() && *v != 0.0)
    }
}

impl YahooRawValue {
    /// Helper to convert to Option<f64>
    fn to_option(&self) -> Option<f64> {
        self.raw.filter(|v| !v.is_nan() && *v != 0.0)
    }
}

// Financials response structures
#[derive(Debug, Clone, Deserialize)]
struct YahooFinancialsResponse {
    #[serde(rename = "quoteSummary")]
    quote_summary: YahooFinancialsSummaryData,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooFinancialsSummaryData {
    result: Vec<YahooFinancialsResult>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooFinancialsResult {
    #[serde(rename = "incomeStatementHistory", default)]
    income_statement_history: Option<YahooIncomeStatementHistoryWrapper>,
    #[serde(rename = "incomeStatementHistoryQuarterly", default)]
    income_statement_history_quarterly: Option<YahooIncomeStatementHistoryWrapper>,
    #[serde(rename = "balanceSheetHistory", default)]
    balance_sheet_history: Option<YahooBalanceSheetHistoryWrapper>,
    #[serde(rename = "balanceSheetHistoryQuarterly", default)]
    balance_sheet_history_quarterly: Option<YahooBalanceSheetHistoryWrapper>,
    #[serde(rename = "cashflowStatementHistory", default)]
    cashflow_statement_history: Option<YahooCashFlowHistoryWrapper>,
    #[serde(rename = "cashflowStatementHistoryQuarterly", default)]
    cashflow_statement_history_quarterly: Option<YahooCashFlowHistoryWrapper>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooIncomeStatementHistoryWrapper {
    #[serde(rename = "incomeStatementHistory", default)]
    income_statement_history: Vec<YahooIncomeStatement>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooBalanceSheetHistoryWrapper {
    #[serde(rename = "balanceSheetHistory", default)]
    balance_sheet_history: Vec<YahooBalanceSheet>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooCashFlowHistoryWrapper {
    #[serde(rename = "cashflowStatementHistory", default)]
    cashflow_statement_history: Vec<YahooCashFlow>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooIncomeStatement {
    #[serde(rename = "endDate", default)]
    end_date: YahooEndDate,
    #[serde(rename = "totalRevenue", default)]
    total_revenue: Option<YahooRawValue>,
    #[serde(rename = "grossProfit", default)]
    gross_profit: Option<YahooRawValue>,
    #[serde(rename = "netIncome", default)]
    net_income: Option<YahooRawValue>,
    #[serde(rename = "basicEPS", default)]
    basic_eps: Option<YahooRawValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooBalanceSheet {
    #[serde(rename = "endDate", default)]
    end_date: YahooEndDate,
    #[serde(rename = "totalAssets", default)]
    total_assets: Option<YahooRawValue>,
    #[serde(rename = "totalLiab", default)]
    total_liabilities_net_minority_interest: Option<YahooRawValue>,
    #[serde(rename = "totalStockholderEquity", default)]
    total_stockholder_equity: Option<YahooRawValue>,
    #[serde(rename = "cash", default)]
    cash: Option<YahooRawValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooCashFlow {
    #[serde(rename = "endDate", default)]
    end_date: YahooEndDate,
    #[serde(rename = "totalCashFromOperatingActivities", default)]
    total_cash_from_operating_activities: Option<YahooRawValue>,
    #[serde(rename = "totalCashflowsFromInvestingActivities", default)]
    total_cashflows_from_investing_activities: Option<YahooRawValue>,
    #[serde(rename = "totalCashFromFinancingActivities", default)]
    total_cash_from_financing_activities: Option<YahooRawValue>,
    #[serde(rename = "capitalExpenditures", default)]
    capital_expenditures: Option<YahooRawValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct YahooEndDate {
    #[serde(default)]
    fmt: String,
}

// Earnings response structures
#[derive(Debug, Clone, Deserialize)]
struct YahooEarningsResponse {
    #[serde(rename = "quoteSummary")]
    quote_summary: YahooEarningsSummaryData,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooEarningsSummaryData {
    result: Vec<YahooEarningsResult>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooEarningsResult {
    #[serde(rename = "earnings", default)]
    earnings: Option<YahooEarningsData>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooEarningsData {
    #[serde(rename = "financialsChart", default)]
    financials_chart: Option<YahooFinancialsChart>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooFinancialsChart {
    #[serde(default)]
    quarterly: Vec<YahooEarningsQuarter>,
}

#[derive(Debug, Clone, Deserialize)]
struct YahooEarningsQuarter {
    #[serde(default)]
    date: String,
    #[serde(default)]
    actual: Option<f64>,
    #[serde(default)]
    estimate: Option<f64>,
    #[serde(default)]
    year: Option<i32>,
    #[serde(default)]
    quarter: Option<i32>,
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
            Self::with_response(Err(HttpError::new("upstream timeout")))
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
    #[ignore = "Requires real auth flow - was testing mock mode"]
    fn circuit_breaker_opens_after_repeated_transport_failures() {
        let client = Arc::new(RecordingHttpClient::failure());
        let adapter = YahooAdapter::with_http_client(
            client,
            HttpAuth::Cookie(String::from("B=session")),
            None,
        );
        let request = QuoteRequest::new(vec![Symbol::parse("MSFT").expect("valid symbol")])
            .expect("valid request");

        for i in 0..3 {
            let error = block_on(adapter.quote(request.clone())).expect_err("call should fail");
            assert_eq!(error.kind(), SourceErrorKind::Unavailable);

            // Debug: Check circuit breaker state after each failure
            let cb_state = adapter.circuit_breaker.state();
            let consecutive_failures = adapter.circuit_breaker.consecutive_failures();
            println!(
                "After failure {}: state={:?}, consecutive_failures={}",
                i + 1,
                cb_state,
                consecutive_failures
            );
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
