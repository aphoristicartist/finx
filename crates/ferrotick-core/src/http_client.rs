use reqwest::cookie::Jar;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Minimal HTTP method set needed by provider adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Authentication strategy applied to outgoing HTTP requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpAuth {
    None,
    BearerToken(String),
    Header { name: String, value: String },
    Cookie(String),
}

impl HttpAuth {
    pub fn apply(&self, headers: &mut BTreeMap<String, String>) {
        match self {
            Self::None => {}
            Self::BearerToken(token) => {
                headers.insert(String::from("authorization"), format!("Bearer {token}"));
            }
            Self::Header { name, value } => {
                headers.insert(name.to_ascii_lowercase(), value.clone());
            }
            Self::Cookie(cookie) => {
                headers.insert(String::from("cookie"), cookie.clone());
            }
        }
    }
}

/// HTTP request envelope used by adapter transport calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub timeout_ms: u64,
}

impl HttpRequest {
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: BTreeMap::new(),
            body: None,
            timeout_ms: 3_000,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, url)
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(name.into().to_ascii_lowercase(), value.into());
        self
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_auth(mut self, auth: &HttpAuth) -> Self {
        auth.apply(&mut self.headers);
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// HTTP response envelope returned by an adapter transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn ok_json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }

    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// Transport-level HTTP error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError {
    message: String,
    retryable: bool,
}

impl HttpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    pub fn non_retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

impl Display for HttpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HttpError {}

/// Adapter transport contract that supports async execution and auth-aware requests.
pub trait HttpClient: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>>;
}

/// Default no-op transport for deterministic offline tests.
#[derive(Debug, Default)]
pub struct NoopHttpClient;

impl HttpClient for NoopHttpClient {
    fn execute<'a>(
        &'a self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>> {
        Box::pin(async move { Ok(mock_noop_response(&request.url)) })
    }
}

fn mock_noop_response(url: &str) -> HttpResponse {
    if url.contains("fc.yahoo.com") {
        return HttpResponse::ok_json("{}");
    }
    if url.contains("finance.yahoo.com/v1/test/getcrumb") {
        return HttpResponse {
            status: 200,
            body: String::from("mockcrumb"),
        };
    }

    if url.contains("api.polygon.io/v2/aggs/ticker/") && url.contains("/prev") {
        let symbol = extract_between(url, "/ticker/", "/").unwrap_or("AAPL");
        let now_ts = time::OffsetDateTime::now_utc().unix_timestamp();
        return HttpResponse::ok_json(
            json!({
                "status": "OK",
                "results": [{
                    "T": symbol,
                    "c": 150.0,
                    "v": 1_000_000,
                    "t": now_ts
                }]
            })
            .to_string(),
        );
    }

    if url.contains("api.polygon.io/v2/aggs/ticker/") && url.contains("/range/") {
        let limit = query_param(url, "limit")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(30);
        let start_ts = time::OffsetDateTime::now_utc().unix_timestamp() - (limit as i64 * 60);
        let mut results = Vec::with_capacity(limit);
        for idx in 0..limit {
            let base = 100.0 + idx as f64 * 0.1;
            results.push(json!({
                "o": base,
                "h": base + 1.0,
                "l": base - 1.0,
                "c": base + 0.5,
                "v": 1_000_000 + idx as i64,
                "vw": base + 0.25,
                "t": start_ts + (idx as i64 * 60)
            }));
        }
        return HttpResponse::ok_json(json!({ "results": results }).to_string());
    }

    if url.contains("api.polygon.io/v3/reference/tickers") {
        let query = query_param(url, "search").unwrap_or_else(|| String::from("apple"));
        return HttpResponse::ok_json(
            json!({
                "results": [{
                    "ticker": "AAPL",
                    "name": format!("{} Incorporated", query),
                    "market": "stocks",
                    "active": true,
                    "primary_exchange": "XNAS",
                    "currency_name": "USD"
                }]
            })
            .to_string(),
        );
    }

    if url.contains("data.alpaca.markets/v2/stocks/quotes/latest") {
        let symbols = parse_symbols(url);
        let now_ts = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut quotes = Map::new();
        for (idx, symbol) in symbols.iter().enumerate() {
            let bid = 100.0 + idx as f64;
            quotes.insert(
                symbol.clone(),
                json!({
                    "bp": bid,
                    "ap": bid + 0.2,
                    "t": (now_ts + idx as i64).to_string(),
                    "v": 1_000_000 + idx as i64
                }),
            );
        }
        return HttpResponse::ok_json(json!({ "quotes": quotes }).to_string());
    }

    if url.contains("data.alpaca.markets/v2/stocks/") && url.contains("/bars?") {
        let limit = query_param(url, "limit")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(30);
        let mut bars = Vec::with_capacity(limit);
        for idx in 0..limit {
            let base = 200.0 + idx as f64 * 0.2;
            bars.push(json!({
                "t": format!("2024-01-01T09:{:02}:00Z", idx % 60),
                "o": base,
                "h": base + 1.0,
                "l": base - 1.0,
                "c": base + 0.25,
                "v": 500_000 + idx as i64,
                "vw": base + 0.1
            }));
        }
        return HttpResponse::ok_json(json!({ "bars": bars }).to_string());
    }

    if url.contains("alphavantage.co/query") {
        let function = query_param(url, "function").unwrap_or_default();
        if function == "GLOBAL_QUOTE" {
            return HttpResponse::ok_json(
                json!({
                    "Global Quote": {
                        "05. price": 150.0,
                        "06. volume": 1_000_000
                    }
                })
                .to_string(),
            );
        }
        if function.starts_with("TIME_SERIES") {
            let mut series = Map::new();
            for idx in 0..64 {
                series.insert(
                    format!("2024-01-01 09:{:02}:00", idx % 60),
                    json!({
                        "1. open": 120.0 + idx as f64 * 0.1,
                        "2. high": 121.0 + idx as f64 * 0.1,
                        "3. low": 119.0 + idx as f64 * 0.1,
                        "4. close": 120.5 + idx as f64 * 0.1,
                        "5. volume": 100_000 + idx as i64
                    }),
                );
            }
            return HttpResponse::ok_json(json!({ "Time Series (1min)": series }).to_string());
        }
        if function == "SYMBOL_SEARCH" {
            return HttpResponse::ok_json(
                json!({
                    "bestMatches": [{
                        "1. symbol": "AAPL",
                        "2. name": "Apple Inc.",
                        "3. type": "Equity",
                        "8. currency": "USD"
                    }]
                })
                .to_string(),
            );
        }
    }

    if url.contains("query1.finance.yahoo.com/v7/finance/quote?") {
        let symbols = parse_symbols(url);
        let result: Vec<Value> = symbols
            .iter()
            .enumerate()
            .map(|(idx, symbol)| {
                json!({
                    "symbol": symbol,
                    "regularMarketPrice": 180.0 + idx as f64,
                    "regularMarketBid": 179.9 + idx as f64,
                    "regularMarketAsk": 180.1 + idx as f64,
                    "regularMarketVolume": 1_000_000 + idx as i64,
                    "currency": "USD"
                })
            })
            .collect();

        return HttpResponse::ok_json(
            json!({
                "quoteResponse": {
                    "result": result,
                    "error": null
                }
            })
            .to_string(),
        );
    }

    if url.contains("query1.finance.yahoo.com/v8/finance/chart/") {
        let mut timestamp = Vec::with_capacity(512);
        let mut open = Vec::with_capacity(512);
        let mut high = Vec::with_capacity(512);
        let mut low = Vec::with_capacity(512);
        let mut close = Vec::with_capacity(512);
        let mut volume = Vec::with_capacity(512);
        for idx in 0..512 {
            let base = 100.0 + idx as f64 * 0.05;
            timestamp.push(json!(1_700_000_000 + idx as i64 * 60));
            open.push(json!(base));
            high.push(json!(base + 1.0));
            low.push(json!(base - 1.0));
            close.push(json!(base + 0.25));
            volume.push(json!(1_000_000 + idx as i64));
        }

        return HttpResponse::ok_json(
            json!({
                "chart": {
                    "result": [{
                        "timestamp": timestamp,
                        "indicators": {
                            "quote": [{
                                "open": open,
                                "high": high,
                                "low": low,
                                "close": close,
                                "volume": volume
                            }]
                        }
                    }],
                    "error": null
                }
            })
            .to_string(),
        );
    }

    if url.contains("query2.finance.yahoo.com/v1/finance/search") {
        return HttpResponse::ok_json(
            json!({
                "quotes": [{
                    "symbol": "AAPL",
                    "shortname": "Apple Inc.",
                    "exchange": "NASDAQ",
                    "quoteType": "EQUITY",
                    "currency": "USD"
                }]
            })
            .to_string(),
        );
    }

    if url.contains("/v10/finance/quoteSummary/") {
        let modules = query_param(url, "modules").unwrap_or_default();

        if modules.contains("earnings") {
            return HttpResponse::ok_json(
                json!({
                    "quoteSummary": {
                        "result": [{
                            "earnings": {
                                "financialsChart": {
                                    "quarterly": [{
                                        "date": "2024-12-31T00:00:00Z",
                                        "actual": 2.0,
                                        "estimate": 1.8,
                                        "year": 2024,
                                        "quarter": 4
                                    }]
                                }
                            }
                        }],
                        "error": null
                    }
                })
                .to_string(),
            );
        }

        if modules.contains("incomeStatementHistory")
            || modules.contains("balanceSheetHistory")
            || modules.contains("cashflowStatementHistory")
        {
            return HttpResponse::ok_json(
                json!({
                    "quoteSummary": {
                        "result": [{
                            "incomeStatementHistory": {
                                "incomeStatementHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalRevenue": { "raw": 100_000_000.0 },
                                    "grossProfit": { "raw": 40_000_000.0 },
                                    "netIncome": { "raw": 20_000_000.0 },
                                    "basicEPS": { "raw": 2.5 }
                                }]
                            },
                            "incomeStatementHistoryQuarterly": {
                                "incomeStatementHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalRevenue": { "raw": 25_000_000.0 },
                                    "grossProfit": { "raw": 10_000_000.0 },
                                    "netIncome": { "raw": 5_000_000.0 },
                                    "basicEPS": { "raw": 0.6 }
                                }]
                            },
                            "balanceSheetHistory": {
                                "balanceSheetHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalAssets": { "raw": 300_000_000.0 },
                                    "totalLiab": { "raw": 120_000_000.0 },
                                    "totalStockholderEquity": { "raw": 180_000_000.0 },
                                    "cash": { "raw": 40_000_000.0 }
                                }]
                            },
                            "balanceSheetHistoryQuarterly": {
                                "balanceSheetHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalAssets": { "raw": 310_000_000.0 },
                                    "totalLiab": { "raw": 125_000_000.0 },
                                    "totalStockholderEquity": { "raw": 185_000_000.0 },
                                    "cash": { "raw": 42_000_000.0 }
                                }]
                            },
                            "cashflowStatementHistory": {
                                "cashflowStatementHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalCashFromOperatingActivities": { "raw": 30_000_000.0 },
                                    "totalCashflowsFromInvestingActivities": { "raw": -8_000_000.0 },
                                    "totalCashFromFinancingActivities": { "raw": -5_000_000.0 },
                                    "capitalExpenditures": { "raw": -3_500_000.0 }
                                }]
                            },
                            "cashflowStatementHistoryQuarterly": {
                                "cashflowStatementHistory": [{
                                    "endDate": { "fmt": "2024-12-31T00:00:00Z" },
                                    "totalCashFromOperatingActivities": { "raw": 7_500_000.0 },
                                    "totalCashflowsFromInvestingActivities": { "raw": -2_000_000.0 },
                                    "totalCashFromFinancingActivities": { "raw": -1_200_000.0 },
                                    "capitalExpenditures": { "raw": -900_000.0 }
                                }]
                            }
                        }],
                        "error": null
                    }
                })
                .to_string(),
            );
        }

        return HttpResponse::ok_json(
            json!({
                "quoteSummary": {
                    "result": [{
                        "price": {
                            "marketCap": { "raw": 2_500_000_000_000.0 }
                        },
                        "summaryDetail": {
                            "forwardPE": { "raw": 28.0 },
                            "PE_RATIO": { "raw": 30.0 },
                            "dividendYield": { "raw": 0.005 }
                        },
                        "defaultKeyStatistics": {
                            "marketCap": { "raw": 2_500_000_000_000.0 },
                            "PE_RATIO": { "raw": 30.0 },
                            "dividendYield": { "raw": 0.005 }
                        }
                    }],
                    "error": null
                }
            })
            .to_string(),
        );
    }

    HttpResponse::ok_json("{}")
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        let name = parts.next()?;
        let value = parts.next().unwrap_or_default();
        if name.eq_ignore_ascii_case(key) {
            Some(
                urlencoding::decode(value)
                    .map(|decoded| decoded.into_owned())
                    .unwrap_or_else(|_| value.to_string()),
            )
        } else {
            None
        }
    })
}

fn parse_symbols(url: &str) -> Vec<String> {
    let raw = query_param(url, "symbols").unwrap_or_else(|| String::from("AAPL"));
    raw.split(',')
        .filter_map(|symbol| {
            let trimmed = symbol.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

fn extract_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let suffix = haystack.split(start).nth(1)?;
    Some(suffix.split(end).next().unwrap_or(suffix))
}

/// Production HTTP client using reqwest for real API calls.
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    client: Arc<reqwest::Client>,
    _cookie_jar: Arc<Jar>, // Keep the jar alive for the lifetime of the client
}

impl ReqwestHttpClient {
    /// Create a new ReqwestHttpClient with default configuration.
    /// Enables cookie store for maintaining session cookies across requests.
    pub fn new() -> Self {
        let cookie_jar = Arc::new(Jar::default());
        let client = Arc::new(
            reqwest::Client::builder()
                .cookie_provider(cookie_jar.clone())
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .build()
                .expect("failed to build reqwest client"),
        );
        Self {
            client,
            _cookie_jar: cookie_jar,
        }
    }

    /// Create a ReqwestHttpClient with a custom reqwest::Client.
    /// Note: For proper authentication, the provided client should have
    /// a cookie jar configured.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client: Arc::new(client),
            _cookie_jar: Arc::new(Jar::default()),
        }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestHttpClient {
    fn execute<'a>(
        &'a self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, HttpError>> + Send + 'a>> {
        Box::pin(async move {
            let mut builder = match request.method {
                HttpMethod::Get => self.client.get(&request.url),
                HttpMethod::Post => self.client.post(&request.url),
            };

            // Apply headers
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }

            // Apply timeout
            let timeout = std::time::Duration::from_millis(request.timeout_ms);
            builder = builder.timeout(timeout);

            // Apply body if present
            if let Some(body) = request.body {
                builder = builder.body(body);
            }

            // Execute request
            let response = builder.send().await.map_err(|e| {
                if e.is_timeout() {
                    HttpError::new(format!("request timeout: {}", e))
                } else if e.is_connect() {
                    HttpError::new(format!("connection failed: {}", e))
                } else {
                    HttpError::new(format!("request failed: {}", e))
                }
            })?;

            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| HttpError::new(format!("failed to read response body: {}", e)))?;

            Ok(HttpResponse { status, body })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_auth_populates_authorization_header() {
        let request = HttpRequest::get("https://example.test/quote")
            .with_auth(&HttpAuth::BearerToken(String::from("token-123")));

        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer token-123")
        );
    }

    #[test]
    fn custom_header_auth_preserves_name_and_value() {
        let request = HttpRequest::get("https://example.test/quote").with_auth(&HttpAuth::Header {
            name: String::from("X-API-Key"),
            value: String::from("demo"),
        });

        assert_eq!(
            request.headers.get("x-api-key").map(String::as_str),
            Some("demo")
        );
    }
}
