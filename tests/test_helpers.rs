//! Test helpers for creating adapters with mock HTTP clients.
//!
//! Since mock mode was removed, tests need to create adapters with
//! NoopHttpClient to avoid making real API calls.

use std::sync::Arc;

use ferrotick_core::{
    http_client::{HttpAuth, NoopHttpClient},
    routing::SourceRouter,
    AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter,
};

/// Create a PolygonAdapter with NoopHttpClient for testing.
pub fn mock_polygon() -> PolygonAdapter {
    PolygonAdapter::with_http_client(Arc::new(NoopHttpClient::default()), HttpAuth::None)
}

/// Create an AlpacaAdapter with NoopHttpClient for testing.
pub fn mock_alpaca() -> AlpacaAdapter {
    AlpacaAdapter::with_http_client(
        Arc::new(NoopHttpClient::default()),
        "test-key".to_string(),
        "test-secret".to_string(),
    )
}

/// Create an AlphaVantageAdapter with NoopHttpClient for testing.
pub fn mock_alphavantage() -> AlphaVantageAdapter {
    AlphaVantageAdapter::with_http_client(
        Arc::new(NoopHttpClient::default()),
        "test-key".to_string(),
    )
}

/// Create a YahooAdapter with NoopHttpClient for testing.
pub fn mock_yahoo() -> YahooAdapter {
    YahooAdapter::with_http_client(Arc::new(NoopHttpClient::default()), HttpAuth::None)
}

/// Create a SourceRouter with all adapters using NoopHttpClient.
pub fn mock_router() -> SourceRouter {
    let http_client = Arc::new(NoopHttpClient::default());
    SourceRouter::new(vec![
        Arc::new(PolygonAdapter::with_http_client(
            http_client.clone(),
            HttpAuth::None,
        )),
        Arc::new(AlpacaAdapter::with_http_client(
            http_client.clone(),
            "test-key".to_string(),
            "test-secret".to_string(),
        )),
        Arc::new(AlphaVantageAdapter::with_http_client(
            http_client.clone(),
            "test-key".to_string(),
        )),
        Arc::new(YahooAdapter::with_http_client(
            http_client.clone(),
            HttpAuth::None,
        )),
    ])
}
