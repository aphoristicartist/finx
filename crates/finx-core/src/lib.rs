//! Core contracts for finx.
//!
//! This crate contains:
//! - Canonical domain models and validation
//! - Provider/source identifiers
//! - Response envelope and structured errors

pub mod domain;
pub mod envelope;
pub mod error;
pub mod source;

pub use domain::{
    AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType, Fundamental, Instrument,
    Interval, Quote, Symbol, UtcDateTime,
};
pub use envelope::{Envelope, EnvelopeError, EnvelopeMeta};
pub use error::{CoreError, ValidationError};
pub use source::ProviderId;
