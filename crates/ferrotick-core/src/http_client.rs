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
        let _ = request;
        Box::pin(async move { Ok(HttpResponse::ok_json("{}")) })
    }
}

/// Production HTTP client using reqwest for real API calls.
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    client: Arc<reqwest::Client>,
}

impl ReqwestHttpClient {
    /// Create a new ReqwestHttpClient with default configuration.
    pub fn new() -> Self {
        Self {
            client: Arc::new(
                reqwest::Client::builder()
                    .user_agent("ferrotick/0.1.0")
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new()),
            ),
        }
    }

    /// Create a ReqwestHttpClient with a custom reqwest::Client.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client: Arc::new(client),
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
