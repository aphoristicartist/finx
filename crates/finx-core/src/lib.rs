//! Core contracts for finx.
//!
//! This crate contains:
//! - Canonical domain models and validation
//! - Provider/source identifiers
//! - Response envelope and structured errors
//! - Data source traits/adapters and routing

pub mod adapters;
pub mod data_source;
pub mod domain;
pub mod envelope;
pub mod error;
pub mod routing;
pub mod source;

pub use adapters::{PolygonAdapter, YahooAdapter};
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
pub use routing::{
    RouteFailure, RouteResult, RouteSuccess, SourceRouter, SourceSnapshot, SourceStrategy,
};
pub use source::ProviderId;
