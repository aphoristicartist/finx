// Test library for provider contract tests
pub use ferrotick_core::{
    adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter},
    data_source::{
        BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
        HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
    },
    routing::{SourceRouter, SourceRouterBuilder, SourceStrategy},
    http_client::{HttpAuth, NoopHttpClient},
    ProviderId, Symbol,
};
pub use std::sync::Arc;

pub mod test_helpers;