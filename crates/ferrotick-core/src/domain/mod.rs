//! # Domain Models
//!
//! Canonical domain types for Ferrotick market data.
//!
//! ## Overview
//!
//! This module provides strongly-typed domain models with built-in validation.
//! All models are designed to be:
//!
//! - **Type-safe**: Invalid states are unrepresentable
//! - **Validated**: Construction validates all invariants
//! - **Serializable**: Full serde support for JSON
//!
//! ## Models
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Quote`] | Top-of-book quote with price, bid, ask |
//! | [`Bar`] | OHLCV bar with timestamp |
//! | [`BarSeries`] | Collection of bars for a symbol/interval |
//! | [`Fundamental`] | Company fundamentals snapshot |
//! | [`Instrument`] | Instrument metadata |
//! | [`CorporateAction`] | Corporate actions (dividends, splits) |
//! | [`Symbol`] | Validated stock symbol |
//! | [`Interval`] | Bar interval (1m, 5m, 1h, 1d) |
//! | [`UtcDateTime`] | UTC timestamp |
//!
//! ## Validation
//!
//! All domain types enforce invariants at construction time:
//!
//! ```rust,ignore
//! use ferrotick_core::{Bar, UtcDateTime, ValidationError};
//!
//! // Valid bar
//! let ts = UtcDateTime::parse("2024-01-01T00:00:00Z")?;
//! let bar = Bar::new(ts, 100.0, 105.0, 95.0, 102.0, Some(1000), None)?;
//!
//! // Invalid bar (high < low) - returns ValidationError
//! let invalid = Bar::new(ts, 100.0, 95.0, 105.0, 102.0, Some(1000), None);
//! assert!(matches!(invalid, Err(ValidationError::InvalidBarRange)));
//! ```
//!
//! ## Asset Classes
//!
//! Supported asset classes via [`AssetClass`]:
//!
//! - `Equity` - Common stocks
//! - `Etf` - Exchange-traded funds
//! - `Index` - Market indices
//! - `Crypto` - Cryptocurrencies
//! - `Forex` - Currency pairs
//! - `Fund` - Mutual funds
//! - `Other` - Other instruments

mod interval;
mod models;
mod symbol;
mod timestamp;

pub use interval::Interval;
pub use models::{
    validate_currency_code, AssetClass, Bar, BarSeries, CorporateAction, CorporateActionType,
    Fundamental, Instrument, Quote,
};
pub use symbol::Symbol;
pub use timestamp::UtcDateTime;
