use thiserror::Error;

/// Validation and contract errors exposed by `finx-core`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("symbol cannot be empty")]
    EmptySymbol,
    #[error("symbol length {len} exceeds max {max}")]
    SymbolTooLong { len: usize, max: usize },
    #[error("symbol must start with an ASCII letter: '{ch}'")]
    SymbolInvalidStart { ch: char },
    #[error("symbol contains invalid character '{ch}' at index {index}")]
    SymbolInvalidChar { ch: char, index: usize },

    #[error("invalid interval '{value}', expected one of 1m, 5m, 15m, 1h, 1d")]
    InvalidInterval { value: String },
    #[error("invalid source '{value}', expected one of yahoo, polygon, alphavantage, alpaca")]
    InvalidSource { value: String },

    #[error("timestamp must be RFC3339 UTC (suffix Z): '{value}'")]
    TimestampNotUtc { value: String },

    #[error("currency must be a 3-letter uppercase ISO code: '{value}'")]
    InvalidCurrency { value: String },

    #[error("field '{field}' must be finite")]
    NonFiniteValue { field: &'static str },
    #[error("field '{field}' must be non-negative")]
    NegativeValue { field: &'static str },

    #[error("bar high must be >= low")]
    InvalidBarRange,
    #[error("bar open/close must be within high/low range")]
    InvalidBarBounds,

    #[error("request_id must be at least 8 characters")]
    InvalidRequestId,
    #[error("trace_id must be 32 hex characters")]
    InvalidTraceId,
    #[error("schema_version must match vMAJOR.MINOR.PATCH: '{value}'")]
    InvalidSchemaVersion { value: String },
    #[error("source_chain must contain at least one source")]
    EmptySourceChain,

    #[error("error code cannot be empty")]
    EmptyErrorCode,
    #[error("error message cannot be empty")]
    EmptyErrorMessage,
}

/// Top-level error type for core operations.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
