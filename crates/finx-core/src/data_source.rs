use std::fmt::{Display, Formatter};

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
}

impl Endpoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Bars => "bars",
            Self::Fundamentals => "fundamentals",
            Self::Search => "search",
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
}

impl CapabilitySet {
    pub const fn new(quote: bool, bars: bool, fundamentals: bool, search: bool) -> Self {
        Self {
            quote,
            bars,
            fundamentals,
            search,
        }
    }

    pub const fn full() -> Self {
        Self::new(true, true, true, true)
    }

    pub const fn supports(self, endpoint: Endpoint) -> bool {
        match endpoint {
            Endpoint::Quote => self.quote,
            Endpoint::Bars => self.bars,
            Endpoint::Fundamentals => self.fundamentals,
            Endpoint::Search => self.search,
        }
    }

    pub fn supported_endpoints(self) -> Vec<&'static str> {
        let mut values = Vec::with_capacity(4);
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

/// Source adapter contract.
pub trait DataSource: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> CapabilitySet;
    fn quote(&self, req: &QuoteRequest) -> Result<QuoteBatch, SourceError>;
    fn bars(&self, req: &BarsRequest) -> Result<BarSeries, SourceError>;
    fn fundamentals(&self, req: &FundamentalsRequest) -> Result<FundamentalsBatch, SourceError>;
    fn search(&self, req: &SearchRequest) -> Result<SearchBatch, SourceError>;
    fn health(&self) -> HealthStatus;
}
