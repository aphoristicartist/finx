//! # Request Metadata and Tracking
//!
//! This module provides types for tracking requests across distributed systems.
//!
//! ## Types
//!
//! - [`RequestId`]: UUID v4 request identifier
//! - [`TraceId`]: W3C-style 32-character hex trace identifier
//! - [`AgentMetadata`]: Full metadata payload for AI-agent requests
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferrotick_agent::metadata::{RequestId, TraceId, AgentMetadata};
//!
//! let request_id = RequestId::new_v4();
//! let trace_id = TraceId::new();
//!
//! let metadata = AgentMetadata::new(
//!     request_id,
//!     trace_id,
//!     vec![ProviderId::Yahoo],
//!     142,
//!     false,
//! )?;
//! ```

use std::fmt::{self, Display, Formatter};

use ferrotick_core::{EnvelopeMeta, ProviderId, ValidationError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request identifier (UUID v4) for end-to-end request tracking.
///
/// The request ID is included in every envelope and can be used to correlate
/// logs, metrics, and traces across the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(Uuid);

impl RequestId {
    /// Generate a new random UUID v4 request ID.
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a request ID from a string.
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let uuid = Uuid::parse_str(input)
            .map_err(|_| ValidationError::InvalidRequestId)?;
        Ok(Self(uuid))
    }

    /// Get the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to a hyphenated string representation.
    pub fn to_hyphenated(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new_v4()
    }
}

/// Distributed tracing identifier (W3C-style 16-byte hex trace id).
///
/// The trace ID follows the W3C Trace Context specification format:
/// 32 lowercase hexadecimal characters representing 16 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    /// Generate a new random trace ID.
    ///
    /// Uses UUID v4's random bytes as the source of entropy.
    pub fn new() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    /// Create a trace ID from a string.
    ///
    /// Returns an error if the string is not a valid 32-character hex string.
    pub fn new_checked(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if !is_valid_trace_id(&value) {
            return Err(ValidationError::InvalidTraceId);
        }
        Ok(Self(value))
    }

    /// Get the trace ID as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for TraceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

fn is_valid_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.chars().all(|ch| ch.is_ascii_hexdigit())
        && value.chars().any(|ch| ch != '0')
}

/// Canonical metadata payload for AI-agent requests.
///
/// This structure is included in every envelope and provides:
/// - Request tracking (request_id, trace_id)
/// - Source attribution (source_chain)
/// - Performance metrics (latency_ms, cache_hit)
/// - Diagnostic information (warnings)
///
/// Field order is deterministic for stable JSON serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// Unique request identifier (UUID v4).
    pub request_id: RequestId,
    /// Distributed tracing identifier.
    pub trace_id: TraceId,
    /// Chain of data providers used for this request.
    pub source_chain: Vec<ProviderId>,
    /// Total request latency in milliseconds.
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub latency_ms: u64,
    /// Whether the response was served from cache.
    pub cache_hit: bool,
    /// Non-fatal warnings encountered during request processing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl AgentMetadata {
    /// Create new metadata with required fields.
    ///
    /// # Errors
    ///
    /// Returns an error if `source_chain` is empty.
    pub fn new(
        request_id: RequestId,
        trace_id: TraceId,
        source_chain: Vec<ProviderId>,
        latency_ms: u64,
        cache_hit: bool,
    ) -> Result<Self, ValidationError> {
        if source_chain.is_empty() {
            return Err(ValidationError::EmptySourceChain);
        }

        Ok(Self {
            request_id,
            trace_id,
            source_chain,
            latency_ms,
            cache_hit,
            warnings: Vec::new(),
        })
    }

    /// Create metadata with auto-generated IDs.
    pub fn new_auto(
        source_chain: Vec<ProviderId>,
        latency_ms: u64,
        cache_hit: bool,
    ) -> Result<Self, ValidationError> {
        Self::new(
            RequestId::new_v4(),
            TraceId::new(),
            source_chain,
            latency_ms,
            cache_hit,
        )
    }

    /// Add a warning to the metadata.
    pub fn push_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Add multiple warnings.
    pub fn extend_warnings(&mut self, warnings: impl IntoIterator<Item = String>) {
        self.warnings.extend(warnings);
    }

    /// Convert to an EnvelopeMeta for use in envelopes.
    pub fn into_envelope_meta(
        self,
        schema_version: &str,
    ) -> Result<EnvelopeMeta, ValidationError> {
        let mut meta = EnvelopeMeta::new(
            self.request_id.to_string(),
            schema_version,
            self.source_chain,
            self.latency_ms,
            self.cache_hit,
        )?
        .with_trace_id(self.trace_id.to_string())?;

        for warning in self.warnings {
            meta.push_warning(warning);
        }

        Ok(meta)
    }

    /// Generate deterministic JSON with stable field ordering.
    ///
    /// `latency_ms` is always formatted as an integer, never scientific notation.
    pub fn to_deterministic_json(&self) -> Result<String, serde_json::Error> {
        let request_id = serde_json::to_string(&self.request_id.to_string())?;
        let trace_id = serde_json::to_string(self.trace_id.as_str())?;
        let source_chain = serde_json::to_string(&self.source_chain)?;
        let warnings = serde_json::to_string(&self.warnings)?;

        Ok(format!(
            r#"{{"request_id":{},"trace_id":{},"source_chain":{},"latency_ms":{},"cache_hit":{},"warnings":{}}}"#,
            request_id,
            trace_id,
            source_chain,
            self.latency_ms,
            if self.cache_hit { "true" } else { "false" },
            warnings
        ))
    }
}

fn serialize_u64_decimal<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(*value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_is_uuid_v4() {
        let request_id = RequestId::new_v4();
        assert_eq!(request_id.as_uuid().get_version_num(), 4);
    }

    #[test]
    fn request_id_parses_valid_uuid() {
        let parsed = RequestId::parse("123e4567-e89b-42d3-a456-426614174000");
        assert!(parsed.is_ok());
    }

    #[test]
    fn request_id_rejects_invalid_uuid() {
        let parsed = RequestId::parse("not-a-uuid");
        assert!(parsed.is_err());
    }

    #[test]
    fn trace_id_format_is_valid() {
        let trace_id = TraceId::new();
        let s = trace_id.as_str();
        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(s.chars().any(|ch| ch != '0'));
    }

    #[test]
    fn trace_id_rejects_invalid_format() {
        let result = TraceId::new_checked("not-valid");
        assert!(result.is_err());
    }

    #[test]
    fn trace_id_accepts_valid_format() {
        let result = TraceId::new_checked("0123456789abcdef0123456789abcdef");
        assert!(result.is_ok());
    }

    #[test]
    fn metadata_requires_source_chain() {
        let result = AgentMetadata::new(
            RequestId::new_v4(),
            TraceId::new(),
            vec![],
            100,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn metadata_to_json_is_deterministic() {
        let metadata = AgentMetadata::new(
            RequestId::parse("123e4567-e89b-42d3-a456-426614174000").unwrap(),
            TraceId::new_checked("0123456789abcdef0123456789abcdef").unwrap(),
            vec![ProviderId::Yahoo],
            4200,
            true,
        )
        .unwrap();

        let json_a = metadata.to_deterministic_json().unwrap();
        let json_b = metadata.to_deterministic_json().unwrap();
        assert_eq!(json_a, json_b);
    }

    #[test]
    fn metadata_json_avoids_scientific_notation() {
        let metadata = AgentMetadata::new(
            RequestId::new_v4(),
            TraceId::new(),
            vec![ProviderId::Yahoo],
            100000,
            false,
        )
        .unwrap();

        let json = metadata.to_deterministic_json().unwrap();
        assert!(json.contains("\"latency_ms\":100000"));
        assert!(!json.contains("e+") && !json.contains("E+"));
    }

    #[test]
    fn metadata_converts_to_envelope_meta() {
        let metadata = AgentMetadata::new(
            RequestId::new_v4(),
            TraceId::new(),
            vec![ProviderId::Yahoo],
            100,
            false,
        )
        .unwrap();

        let envelope_meta = metadata.into_envelope_meta("v1.0.0");
        assert!(envelope_meta.is_ok());
    }
}
