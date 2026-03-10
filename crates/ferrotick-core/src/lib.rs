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
//! ```rust,no_run
//! use std::sync::Arc;
//! use ferrotick_core::{
//!     PolygonAdapter, QuoteRequest, Symbol,
//!     http_client::{HttpAuth, NoopHttpClient},
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a provider adapter
//! let adapter = PolygonAdapter::with_http_client(Arc::new(NoopHttpClient), HttpAuth::None, None);
//!
//! // Build a quote request
//! let request = QuoteRequest::new(vec![Symbol::parse("AAPL")?])?;
//! let _ = (adapter, request);
//! # Ok(())
//! # }
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
pub mod assets;
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

// Asset types
pub use assets::{
    CryptoExchange, CryptoPair, ForexPair, FuturesContract, Greeks, OptionContract, OptionType,
};

// Adapter implementations
pub use adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter};

// Circuit breaker
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

// Caching
pub use cache::{CacheMode, CacheStore};

// Data source trait and types
pub use data_source::{
    BarsRequest, CapabilitySet, DataSource, EarningsBatch, EarningsRequest, Endpoint,
    FinancialsBatch, FinancialsRequest, FundamentalsBatch, FundamentalsRequest, HealthState,
    HealthStatus, QuoteBatch, QuoteRequest, SearchBatch, SearchRequest, SourceError,
    SourceErrorKind,
};

// Domain models
pub use domain::{
    AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType, EarningsEntry,
    EarningsReport, FinancialLineItem, FinancialPeriod, FinancialStatement, Fundamental,
    Instrument, Interval, Quote, StatementType, Symbol, UtcDateTime,
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
    HttpAuth, HttpClient, HttpError, HttpMethod, HttpRequest, HttpResponse, ReqwestHttpClient,
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
