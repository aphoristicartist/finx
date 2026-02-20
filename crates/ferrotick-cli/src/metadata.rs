use std::fmt::{Display, Formatter};

use ferrotick_core::{EnvelopeMeta, ProviderId, ValidationError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request identifier (UUID v4) for end-to-end request tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(Uuid);

impl RequestId {
    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

/// Distributed tracing identifier (W3C-style 16-byte hex trace id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for TraceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[cfg(test)]
fn is_valid_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.chars().all(|ch| ch.is_ascii_hexdigit())
        && value.chars().any(|ch| ch != '0')
}

/// Canonical command metadata payload used to construct envelope metadata.
///
/// Field order is fixed to keep deterministic JSON serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub request_id: RequestId,
    pub trace_id: TraceId,
    pub source_chain: Vec<ProviderId>,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pub latency_ms: u64,
    pub cache_hit: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Metadata {
    pub fn new(
        source_chain: Vec<ProviderId>,
        latency_ms: u64,
        cache_hit: bool,
    ) -> Result<Self, ValidationError> {
        if source_chain.is_empty() {
            return Err(ValidationError::EmptySourceChain);
        }

        Ok(Self {
            request_id: RequestId::new_v4(),
            trace_id: TraceId::new(),
            source_chain,
            latency_ms,
            cache_hit,
            warnings: Vec::new(),
        })
    }

    pub fn push_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn into_envelope_meta(self, schema_version: &str) -> Result<EnvelopeMeta, ValidationError> {
        let mut envelope_meta = EnvelopeMeta::new(
            self.request_id.to_string(),
            schema_version,
            self.source_chain,
            self.latency_ms,
            self.cache_hit,
        )?
        .with_trace_id(self.trace_id.to_string())?;

        for warning in self.warnings {
            envelope_meta.push_warning(warning);
        }

        Ok(envelope_meta)
    }

    /// Deterministic JSON representation with stable numeric formatting.
    ///
    /// `latency_ms` is emitted as an integer token, never scientific notation.
    pub fn to_deterministic_json(&self) -> Result<String, serde_json::Error> {
        let request_id = serde_json::to_string(self.request_id.to_string().as_str())?;
        let trace_id = serde_json::to_string(self.trace_id.as_str())?;
        let source_chain = serde_json::to_string(&self.source_chain)?;
        let warnings = serde_json::to_string(&self.warnings)?;

        Ok(format!(
            "{{\"request_id\":{request_id},\"trace_id\":{trace_id},\"source_chain\":{source_chain},\"latency_ms\":{},\"cache_hit\":{},\"warnings\":{warnings}}}",
            self.latency_ms,
            if self.cache_hit { "true" } else { "false" }
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
        assert_eq!(request_id.0.get_version_num(), 4);
    }

    #[test]
    fn trace_id_matches_expected_shape() {
        let trace_id = TraceId::new();
        assert!(is_valid_trace_id(trace_id.as_str()));
    }

    #[test]
    fn deterministic_json_is_stable_and_non_scientific() {
        let metadata = Metadata {
            request_id: RequestId(Uuid::parse_str("123e4567-e89b-42d3-a456-426614174000").unwrap()),
            trace_id: TraceId(String::from("0123456789abcdef0123456789abcdef")),
            source_chain: vec![ProviderId::Yahoo, ProviderId::Polygon],
            latency_ms: 4200,
            cache_hit: true,
            warnings: vec![String::from("w1")],
        };

        let rendered_a = metadata.to_deterministic_json().expect("serializes");
        let rendered_b = metadata.to_deterministic_json().expect("serializes");

        assert_eq!(rendered_a, rendered_b);
        assert!(rendered_a.contains("\"latency_ms\":4200"));
        assert!(!rendered_a.contains("\"latency_ms\":4.2e"));
        assert!(!rendered_a.contains("\"latency_ms\":4.2E"));
    }
}
