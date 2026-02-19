//! Core contracts for finx.
//!
//! This crate contains:
//! - Canonical domain models and validation
//! - Provider/source identifiers
//! - Response envelope and structured errors
//! - Data source traits/adapters and routing

pub mod adapters;
pub mod circuit_breaker;
pub mod data_source;
pub mod domain;
pub mod envelope;
pub mod error;
pub mod http_client;
pub mod provider_policy;
pub mod routing;
pub mod source;
pub mod throttling;

pub use adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
    SourceErrorKind,
};
pub use domain::{
    AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType, Fundamental, Instrument,
    Interval, Quote, Symbol, UtcDateTime,
};
pub use envelope::{Envelope, EnvelopeError, EnvelopeMeta};
pub use error::{CoreError, ValidationError};
pub use finx_warehouse::{
    BarRecord, CacheSyncReport, FundamentalRecord, QueryGuardrails, QueryResult, QuoteRecord,
    SqlColumn, Warehouse, WarehouseConfig, WarehouseError,
};
pub use http_client::{
    HttpAuth, HttpClient, HttpError, HttpMethod, HttpRequest, HttpResponse, NoopHttpClient,
};
pub use provider_policy::{BackoffPolicy, ProviderPolicy};
pub use routing::{
    RouteFailure, RouteResult, RouteSuccess, SourceRouter, SourceSnapshot, SourceStrategy,
};
pub use source::ProviderId;
pub use throttling::ThrottlingQueue;
