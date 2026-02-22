//! # Ferrotick Core
//!
//! Core contracts and domain types for the Ferrotick financial data toolkit.
//!
//! ## Overview
//!
//! This crate provides the foundational components for Ferrotick:
//!
//! - **Canonical domain models** for quotes, bars, fundamentals, and instruments
//! - **Provider/source identifiers** for multi-adapter support
//! - **Response envelope** with metadata and structured errors
//! - **Data source traits** for provider adapters
//! - **Routing logic** for source selection and fallback
//! - **Circuit breaker** for resilient upstream calls
//!
//! ## Feature Flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `default` | Standard feature set |
//!
//! ## Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`adapters`] | Provider adapters (Polygon, Yahoo, Alpha Vantage, Alpaca) |
//! | [`circuit_breaker`] | Circuit breaker for resilient calls |
//! | [`data_source`] | Data source trait and request/response types |
//! | [`domain`] | Domain models (Quote, Bar, Fundamental, Instrument) |
//! | [`envelope`] | Response envelope with metadata |
//! | [`error`] | Core error types |
//! | [`http_client`] | HTTP client abstraction |
//! | [`provider_policy`] | Provider policies for routing |
//! | [`routing`] | Source routing and selection |
//! | [`source`] | Provider identifiers |
//! | [`throttling`] | Rate limiting support |
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ferrotick_core::{PolygonAdapter, QuoteRequest, DataSource, Symbol};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a provider adapter
//!     let adapter = PolygonAdapter::default();
//!     
//!     // Fetch a quote
//!     let request = QuoteRequest::new(vec![Symbol::new("AAPL")])?;
//!     let response = adapter.quote(request).await?;
//!     
//!     // Access the data
//!     if let Some(quote) = response.quotes.first() {
//!         println!("AAPL price: ${:.2}", quote.price);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │  CLI / User     │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐     ┌──────────────────┐
//! │  Source Router  │────▶│ Circuit Breaker  │
//! └────────┬────────┘     └──────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐     ┌──────────────────┐
//! │ Data Source     │────▶│ HTTP Client      │
//! │ (Adapter Trait) │     │ (reqwest/none)   │
//! └─────────────────┘     └──────────────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Domain Models   │
//! │ (Quote, Bar)    │
//! └─────────────────┘
//! ```
//!
//! ## Error Handling
//!
//! All operations return `Result` types with structured errors:
//!
//! ```rust
//! use ferrotick_core::{SourceError, SourceErrorKind};
//!
//! fn handle_error(error: SourceError) {
//!     match error.kind() {
//!         SourceErrorKind::RateLimited => {
//!             // Wait and retry
//!         }
//!         SourceErrorKind::Unavailable => {
//!             // Try fallback source
//!         }
//!         SourceErrorKind::InvalidRequest => {
//!             // Report to user
//!         }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## Security
//!
//! - API keys are read from environment variables only (never logged)
//! - All HTTP requests use TLS via rustls
//! - Input validation on all domain types

pub mod adapters;
pub mod cache;
pub mod circuit_breaker;
pub mod data_source;
pub mod domain;
pub mod envelope;
pub mod error;
pub mod http_client;
pub mod provider_policy;
pub mod retry;
pub mod routing;
pub mod source;
pub mod throttling;

// Re-export commonly used types at crate root for convenience

// Adapter implementations
pub use adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter};

// Circuit breaker
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

// Caching
pub use cache::{CacheMode, CacheStore};

// Data source trait and types
pub use data_source::{
    BarsRequest, CapabilitySet, DataSource, Endpoint, FundamentalsBatch, FundamentalsRequest,
    HealthState, HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
    SourceErrorKind,
};

// Domain models
pub use domain::{
    AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType, Fundamental, Instrument,
    Interval, Quote, Symbol, UtcDateTime,
};

// Envelope types
pub use envelope::{Envelope, EnvelopeError, EnvelopeMeta};

// Error types
pub use error::{CoreError, ValidationError};

// Warehouse (re-exported from ferrotick-warehouse)
pub use ferrotick_warehouse::{
    BarRecord, CacheSyncReport, FundamentalRecord, QueryGuardrails, QueryResult, QuoteRecord,
    SqlColumn, Warehouse, WarehouseConfig, WarehouseError,
};

// HTTP client types
pub use http_client::{
    HttpAuth, HttpClient, HttpError, HttpMethod, HttpRequest, HttpResponse, NoopHttpClient,
    ReqwestHttpClient,
};

// Provider policies
pub use provider_policy::{BackoffPolicy, ProviderPolicy};

// Retry logic
pub use retry::{Backoff, RetryConfig};

// Routing types
pub use routing::{
    RouteFailure, RouteResult, RouteSuccess, SourceRouter, SourceRouterBuilder, SourceSnapshot,
    SourceStrategy,
};

// Source identifiers
pub use source::ProviderId;

// Throttling
pub use throttling::ThrottlingQueue;
