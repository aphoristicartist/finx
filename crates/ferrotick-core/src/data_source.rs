//! Data source trait and request/response types.
//!
//! This module defines the core adapter contract (`DataSource`) that all
//! provider implementations must follow, along with the request and response
//! types for each endpoint.
//!
//! # Endpoints
//!
//! | Endpoint | Request | Response | Description |
//! |----------|---------|----------|-------------|
//! | Quote | [`QuoteRequest`] | [`QuoteBatch`] | Real-time/delayed quotes |
//! | Bars | [`BarsRequest`] | [`BarSeries`] | Historical OHLCV data |
//! | Fundamentals | [`FundamentalsRequest`] | [`FundamentalsBatch`] | Company fundamentals |
//! | Search | [`SearchRequest`] | [`SearchBatch`] | Instrument search |
//!
//! # Example
//!
//! ```rust,no_run
//! use ferrotick_core::{QuoteRequest, Symbol};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let request = QuoteRequest::new(vec![Symbol::parse("AAPL")?])?;
//! assert_eq!(request.symbols.len(), 1);
//! # Ok(())
//! # }
//! ```

use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::{BarSeries, Fundamental, Instrument, Interval, ProviderId, Quote, Symbol};

/// Data endpoint type used for routing and capability checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endpoint {
    Quote,
    Bars,
    Fundamentals,
    Search,
    Financials,
    Earnings,
}

impl Endpoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Bars => "bars",
            Self::Fundamentals => "fundamentals",
            Self::Search => "search",
            Self::Financials => "financials",
            Self::Earnings => "earnings",
        }
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Supported endpoint matrix for a data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    pub quote: bool,
    pub bars: bool,
    pub fundamentals: bool,
    pub search: bool,
    pub financials: bool,
    pub earnings: bool,
}

impl CapabilitySet {
    pub const fn new(
        quote: bool,
        bars: bool,
        fundamentals: bool,
        search: bool,
        financials: bool,
        earnings: bool,
    ) -> Self {
        Self {
            quote,
            bars,
            fundamentals,
            search,
            financials,
            earnings,
        }
    }

    pub const fn full() -> Self {
        Self::new(true, true, true, true, true, true)
    }

    pub const fn supports(self, endpoint: Endpoint) -> bool {
        match endpoint {
            Endpoint::Quote => self.quote,
            Endpoint::Bars => self.bars,
            Endpoint::Fundamentals => self.fundamentals,
            Endpoint::Search => self.search,
            Endpoint::Financials => self.financials,
            Endpoint::Earnings => self.earnings,
        }
    }

    pub fn supported_endpoints(self) -> Vec<&'static str> {
        let mut values = Vec::with_capacity(6);
        if self.quote {
            values.push("quote");
        }
        if self.bars {
            values.push("bars");
        }
        if self.fundamentals {
            values.push("fundamentals");
        }
        if self.search {
            values.push("search");
        }
        if self.financials {
            values.push("financials");
        }
        if self.earnings {
            values.push("earnings");
        }
        values
    }
}

/// Health state used by source scoring and `sources` command output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Runtime source health snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub state: HealthState,
    pub rate_available: bool,
    /// Provider score component used by `auto` routing.
    pub score: u16,
}

impl HealthStatus {
    pub const fn new(state: HealthState, rate_available: bool, score: u16) -> Self {
        Self {
            state,
            rate_available,
            score,
        }
    }

    pub const fn healthy(score: u16) -> Self {
        Self::new(HealthState::Healthy, true, score)
    }

    pub const fn degraded(score: u16) -> Self {
        Self::new(HealthState::Degraded, true, score)
    }
}

/// Adapter-level error classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceErrorKind {
    UnsupportedEndpoint,
    Unavailable,
    RateLimited,
    InvalidRequest,
    AdapterNotRegistered,
    Internal,
}

/// Structured source error used by router fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError {
    kind: SourceErrorKind,
    message: String,
    retryable: bool,
}

impl SourceError {
    pub fn unsupported_endpoint(endpoint: Endpoint) -> Self {
        Self {
            kind: SourceErrorKind::UnsupportedEndpoint,
            message: format!("endpoint '{endpoint}' is not supported by this source"),
            retryable: false,
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: SourceErrorKind::Unavailable,
            message: message.into(),
            retryable: true,
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            kind: SourceErrorKind::RateLimited,
            message: message.into(),
            retryable: true,
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            kind: SourceErrorKind::InvalidRequest,
            message: message.into(),
            retryable: false,
        }
    }

    pub fn adapter_not_registered(provider: ProviderId) -> Self {
        Self {
            kind: SourceErrorKind::AdapterNotRegistered,
            message: format!("source adapter '{provider}' is not registered"),
            retryable: false,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: SourceErrorKind::Internal,
            message: message.into(),
            retryable: false,
        }
    }

    pub const fn kind(&self) -> SourceErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    pub const fn code(&self) -> &'static str {
        match self.kind {
            SourceErrorKind::UnsupportedEndpoint => "source.unsupported_endpoint",
            SourceErrorKind::Unavailable => "source.unavailable",
            SourceErrorKind::RateLimited => "source.rate_limited",
            SourceErrorKind::InvalidRequest => "source.invalid_request",
            SourceErrorKind::AdapterNotRegistered => "source.adapter_not_registered",
            SourceErrorKind::Internal => "source.internal",
        }
    }
}

impl Display for SourceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code())
    }
}

impl std::error::Error for SourceError {}

/// Request payload for quote endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteRequest {
    pub symbols: Vec<Symbol>,
}

impl QuoteRequest {
    pub fn new(symbols: Vec<Symbol>) -> Result<Self, SourceError> {
        if symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "quote request must include at least one symbol",
            ));
        }
        Ok(Self { symbols })
    }
}

/// Request payload for bar endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsRequest {
    pub symbol: Symbol,
    pub interval: Interval,
    pub limit: usize,
}

impl BarsRequest {
    pub fn new(symbol: Symbol, interval: Interval, limit: usize) -> Result<Self, SourceError> {
        if limit == 0 {
            return Err(SourceError::invalid_request(
                "bars request limit must be greater than zero",
            ));
        }
        Ok(Self {
            symbol,
            interval,
            limit,
        })
    }
}

/// Request payload for fundamentals endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundamentalsRequest {
    pub symbols: Vec<Symbol>,
}

impl FundamentalsRequest {
    pub fn new(symbols: Vec<Symbol>) -> Result<Self, SourceError> {
        if symbols.is_empty() {
            return Err(SourceError::invalid_request(
                "fundamentals request must include at least one symbol",
            ));
        }
        Ok(Self { symbols })
    }
}

/// Request payload for search endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub query: String,
    pub limit: usize,
}

impl SearchRequest {
    pub fn new(query: impl Into<String>, limit: usize) -> Result<Self, SourceError> {
        let query = query.into();
        if query.trim().is_empty() {
            return Err(SourceError::invalid_request(
                "search query must not be empty",
            ));
        }
        if limit == 0 {
            return Err(SourceError::invalid_request(
                "search request limit must be greater than zero",
            ));
        }
        Ok(Self { query, limit })
    }
}

/// Normalized quote batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteBatch {
    pub quotes: Vec<Quote>,
}

/// Normalized fundamentals batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundamentalsBatch {
    pub fundamentals: Vec<Fundamental>,
}

/// Normalized search batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchBatch {
    pub query: String,
    pub results: Vec<Instrument>,
}

/// Request payload for financials endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinancialsRequest {
    pub symbol: Symbol,
    pub statement_type: crate::StatementType,
    pub period: crate::FinancialPeriod,
    pub limit: usize,
}

impl FinancialsRequest {
    pub fn new(
        symbol: Symbol,
        statement_type: crate::StatementType,
        period: crate::FinancialPeriod,
        limit: usize,
    ) -> Result<Self, SourceError> {
        if limit == 0 {
            return Err(SourceError::invalid_request(
                "financials request limit must be greater than zero",
            ));
        }
        Ok(Self {
            symbol,
            statement_type,
            period,
            limit,
        })
    }
}

/// Request payload for earnings endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarningsRequest {
    pub symbol: Symbol,
    pub limit: usize,
}

impl EarningsRequest {
    pub fn new(symbol: Symbol, limit: usize) -> Result<Self, SourceError> {
        if limit == 0 {
            return Err(SourceError::invalid_request(
                "earnings request limit must be greater than zero",
            ));
        }
        Ok(Self { symbol, limit })
    }
}

/// Normalized financials batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialsBatch {
    pub financials: Vec<crate::FinancialStatement>,
}

/// Normalized earnings batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarningsBatch {
    pub earnings: crate::EarningsReport,
}

/// Source adapter contract.
///
/// All data providers must implement this trait to be used with the router.
/// The trait uses async methods returning boxed futures for flexibility.
///
/// # Required Methods
///
/// | Method | Description |
/// |--------|-------------|
/// | [`id`](DataSource::id) | Unique provider identifier |
/// | [`capabilities`](DataSource::capabilities) | Supported endpoints |
/// | [`quote`](DataSource::quote) | Fetch quotes |
/// | [`bars`](DataSource::bars) | Fetch OHLCV bars |
/// | [`fundamentals`](DataSource::fundamentals) | Fetch fundamentals |
/// | [`search`](DataSource::search) | Search instruments |
/// | [`health`](DataSource::health) | Check source health |
///
/// # Example Implementation
///
/// ```text
/// use ferrotick_core::{DataSource, ProviderId, CapabilitySet, QuoteRequest, QuoteBatch};
///
/// struct MyAdapter {
///     // fields...
/// }
///
/// impl DataSource for MyAdapter {
///     fn id(&self) -> ProviderId {
///         ProviderId::new("my_provider")
///     }
///     
///     fn capabilities(&self) -> CapabilitySet {
///         CapabilitySet::new(true, true, false, true, false, false)
///     }
///
///     // ... implement quote/bars/fundamentals/search/financials/earnings/health
/// }
/// ```
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` as they may be shared across threads.
pub trait DataSource: Send + Sync {
    /// Returns the unique provider identifier.
    fn id(&self) -> ProviderId;

    /// Returns the set of supported endpoints.
    fn capabilities(&self) -> CapabilitySet;

    /// Fetches real-time or delayed quotes for the requested symbols.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported (check [`capabilities`](DataSource::capabilities))
    /// - The provider is unavailable
    /// - Rate limiting is in effect
    /// - Invalid symbols are provided
    fn quote<'a>(
        &'a self,
        req: QuoteRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QuoteBatch, SourceError>> + Send + 'a>>;

    /// Fetches historical OHLCV bars.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported
    /// - Invalid interval or limit values
    /// - The provider is unavailable
    fn bars<'a>(
        &'a self,
        req: BarsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BarSeries, SourceError>> + Send + 'a>>;

    /// Fetches company fundamentals.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported
    /// - Invalid symbols are provided
    /// - The provider is unavailable
    fn fundamentals<'a>(
        &'a self,
        req: FundamentalsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FundamentalsBatch, SourceError>> + Send + 'a>>;

    /// Searches for instruments matching a query.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported
    /// - Empty query is provided
    /// - The provider is unavailable
    fn search<'a>(
        &'a self,
        req: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchBatch, SourceError>> + Send + 'a>>;

    /// Fetches financial statements.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported
    /// - Invalid symbol or limit is provided
    /// - The provider is unavailable
    fn financials<'a>(
        &'a self,
        req: FinancialsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FinancialsBatch, SourceError>> + Send + 'a>>;

    /// Fetches earnings data.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError`] if:
    /// - The endpoint is not supported
    /// - Invalid symbol or limit is provided
    /// - The provider is unavailable
    fn earnings<'a>(
        &'a self,
        req: EarningsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EarningsBatch, SourceError>> + Send + 'a>>;

    /// Returns the current health status of this source.
    ///
    /// Used by the router for source scoring and fallback decisions.
    fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = HealthStatus> + Send + 'a>>;
}
