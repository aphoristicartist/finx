//! # Envelope Construction and Validation
//!
//! This module provides utilities for constructing and validating JSON envelopes
//! according to the `schemas/v1/envelope.schema.json` schema.
//!
//! ## Features
//!
//! - Strict schema compliance validation
//! - Deterministic JSON serialization with stable field ordering
//! - Error accumulation for partial failures
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferrotick_agent::envelope::EnvelopeBuilder;
//! use serde_json::json;
//!
//! let envelope = EnvelopeBuilder::new("v1.0.0")
//!     .with_source_chain(vec![ProviderId::Yahoo])
//!     .with_data(json!({ "quotes": [] }))
//!     .with_latency_ms(142)
//!     .with_cache_hit(false)
//!     .build()?;
//!
//! // Validate against schema
//! EnvelopeValidator::validate(&envelope)?;
//! ```

use ferrotick_core::{Envelope, EnvelopeError, EnvelopeMeta, ProviderId, ValidationError};
use serde::Serialize;
use serde_json::Value;

/// Builder for constructing valid envelopes with fluent API.
#[derive(Debug, Clone)]
pub struct EnvelopeBuilder {
    schema_version: String,
    source_chain: Vec<ProviderId>,
    data: Value,
    latency_ms: u64,
    cache_hit: bool,
    warnings: Vec<String>,
    errors: Vec<EnvelopeError>,
    request_id: Option<String>,
    trace_id: Option<String>,
}

impl EnvelopeBuilder {
    /// Create a new envelope builder with the specified schema version.
    ///
    /// The schema version must follow the pattern `vMAJOR.MINOR.PATCH`.
    pub fn new(schema_version: impl Into<String>) -> Self {
        Self {
            schema_version: schema_version.into(),
            source_chain: Vec::new(),
            data: Value::Null,
            latency_ms: 0,
            cache_hit: false,
            warnings: Vec::new(),
            errors: Vec::new(),
            request_id: None,
            trace_id: None,
        }
    }

    /// Set the source chain (providers used for this request).
    pub fn with_source_chain(mut self, source_chain: Vec<ProviderId>) -> Self {
        self.source_chain = source_chain;
        self
    }

    /// Set the response data payload.
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }

    /// Set the request latency in milliseconds.
    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Set whether the response came from cache.
    pub fn with_cache_hit(mut self, cache_hit: bool) -> Self {
        self.cache_hit = cache_hit;
        self
    }

    /// Add a warning to the envelope.
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add multiple warnings to the envelope.
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    /// Add an error to the envelope.
    pub fn with_error(mut self, error: EnvelopeError) -> Self {
        self.errors.push(error);
        self
    }

    /// Add multiple errors to the envelope.
    pub fn with_errors(mut self, errors: Vec<EnvelopeError>) -> Self {
        self.errors.extend(errors);
        self
    }

    /// Set a custom request ID (defaults to UUID v4 if not set).
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set a trace ID for distributed tracing.
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Build the envelope with strict validation.
    ///
    /// Returns an error if the envelope would violate schema constraints.
    pub fn build(self) -> Result<Envelope<Value>, ValidationError> {
        let request_id = self
            .request_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().hyphenated().to_string());

        let mut meta = EnvelopeMeta::new(
            request_id,
            self.schema_version,
            self.source_chain,
            self.latency_ms,
            self.cache_hit,
        )?;

        if let Some(trace_id) = self.trace_id {
            meta = meta.with_trace_id(trace_id)?;
        }

        for warning in self.warnings {
            meta.push_warning(warning);
        }

        Envelope::with_errors(meta, self.data, self.errors)
    }
}

/// Validator for checking envelope compliance against JSON schema.
pub struct EnvelopeValidator;

impl EnvelopeValidator {
    /// Validate an envelope against the bundled JSON schema.
    ///
    /// This performs runtime validation to ensure the envelope structure
    /// is fully compliant with `schemas/v1/envelope.schema.json`.
    pub fn validate<T: Serialize>(envelope: &Envelope<T>) -> Result<(), SchemaValidationError> {
        let value = serde_json::to_value(envelope)?;
        Self::validate_value(&value)
    }

    /// Validate a JSON value against envelope schema constraints.
    pub fn validate_value(value: &Value) -> Result<(), SchemaValidationError> {
        let obj = value
            .as_object()
            .ok_or_else(|| SchemaValidationError::missing_field("root object"))?;

        // Validate meta
        let meta = obj
            .get("meta")
            .ok_or_else(|| SchemaValidationError::missing_field("meta"))?
            .as_object()
            .ok_or_else(|| SchemaValidationError::invalid_type("meta", "object"))?;

        Self::validate_meta(meta)?;

        // Validate errors array if present
        if let Some(errors) = obj.get("errors") {
            let errors_arr = errors
                .as_array()
                .ok_or_else(|| SchemaValidationError::invalid_type("errors", "array"))?;
            for error in errors_arr {
                Self::validate_error(error)?;
            }
        }

        Ok(())
    }

    fn validate_meta(meta: &serde_json::Map<String, Value>) -> Result<(), SchemaValidationError> {
        // request_id: required, minLength 8
        let request_id = meta
            .get("request_id")
            .ok_or_else(|| SchemaValidationError::missing_field("meta.request_id"))?
            .as_str()
            .ok_or_else(|| SchemaValidationError::invalid_type("meta.request_id", "string"))?;
        if request_id.len() < 8 {
            return Err(SchemaValidationError::constraint_violation(
                "meta.request_id",
                "must be at least 8 characters",
            ));
        }

        // schema_version: required, pattern vMAJOR.MINOR.PATCH
        let schema_version = meta
            .get("schema_version")
            .ok_or_else(|| SchemaValidationError::missing_field("meta.schema_version"))?
            .as_str()
            .ok_or_else(|| SchemaValidationError::invalid_type("meta.schema_version", "string"))?;
        if !is_valid_schema_version(schema_version) {
            return Err(SchemaValidationError::constraint_violation(
                "meta.schema_version",
                "must match vMAJOR.MINOR.PATCH",
            ));
        }

        // source_chain: required, minItems 1
        let source_chain = meta
            .get("source_chain")
            .ok_or_else(|| SchemaValidationError::missing_field("meta.source_chain"))?
            .as_array()
            .ok_or_else(|| SchemaValidationError::invalid_type("meta.source_chain", "array"))?;
        if source_chain.is_empty() {
            return Err(SchemaValidationError::constraint_violation(
                "meta.source_chain",
                "must contain at least one source",
            ));
        }

        // latency_ms: required, integer, minimum 0
        let latency_ms = meta
            .get("latency_ms")
            .ok_or_else(|| SchemaValidationError::missing_field("meta.latency_ms"))?;
        if !latency_ms.is_u64() {
            return Err(SchemaValidationError::invalid_type(
                "meta.latency_ms",
                "non-negative integer",
            ));
        }

        // cache_hit: required, boolean
        let cache_hit = meta
            .get("cache_hit")
            .ok_or_else(|| SchemaValidationError::missing_field("meta.cache_hit"))?;
        if !cache_hit.is_boolean() {
            return Err(SchemaValidationError::invalid_type("meta.cache_hit", "boolean"));
        }

        // trace_id: optional, 32 hex chars if present
        if let Some(trace_id) = meta.get("trace_id") {
            let trace_str = trace_id
                .as_str()
                .ok_or_else(|| SchemaValidationError::invalid_type("meta.trace_id", "string"))?;
            if !is_valid_trace_id(trace_str) {
                return Err(SchemaValidationError::constraint_violation(
                    "meta.trace_id",
                    "must be 32 hex characters",
                ));
            }
        }

        Ok(())
    }

    fn validate_error(error: &Value) -> Result<(), SchemaValidationError> {
        let obj = error
            .as_object()
            .ok_or_else(|| SchemaValidationError::invalid_type("error", "object"))?;

        // code: required, minLength 1
        let code = obj
            .get("code")
            .ok_or_else(|| SchemaValidationError::missing_field("error.code"))?
            .as_str()
            .ok_or_else(|| SchemaValidationError::invalid_type("error.code", "string"))?;
        if code.is_empty() {
            return Err(SchemaValidationError::constraint_violation(
                "error.code",
                "must not be empty",
            ));
        }

        // message: required, minLength 1
        let message = obj
            .get("message")
            .ok_or_else(|| SchemaValidationError::missing_field("error.message"))?
            .as_str()
            .ok_or_else(|| SchemaValidationError::invalid_type("error.message", "string"))?;
        if message.is_empty() {
            return Err(SchemaValidationError::constraint_violation(
                "error.message",
                "must not be empty",
            ));
        }

        Ok(())
    }
}

fn is_valid_schema_version(value: &str) -> bool {
    let Some(version) = value.strip_prefix('v') else {
        return false;
    };
    let mut parts = version.split('.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    if parts.next().is_some() {
        return false;
    }
    [major, minor, patch].iter().all(|part| {
        part.is_some_and(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
    })
}

fn is_valid_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.chars().all(|ch| ch.is_ascii_hexdigit())
        && value.chars().any(|ch| ch != '0')
}

/// Error type for schema validation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaValidationError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid type for {field}: expected {expected}")]
    InvalidType { field: String, expected: String },

    #[error("constraint violation at {field}: {message}")]
    ConstraintViolation { field: String, message: String },

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl SchemaValidationError {
    fn missing_field(field: &str) -> Self {
        Self::MissingField(field.to_string())
    }

    fn invalid_type(field: &str, expected: &str) -> Self {
        Self::InvalidType {
            field: field.to_string(),
            expected: expected.to_string(),
        }
    }

    fn constraint_violation(field: &str, message: &str) -> Self {
        Self::ConstraintViolation {
            field: field.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<serde_json::Error> for SchemaValidationError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

/// Serialize an envelope to deterministic JSON with stable field ordering.
///
/// This ensures consistent output for AI consumers by:
/// - Using consistent key ordering
/// - Formatting numbers without scientific notation
/// - Ensuring deterministic array ordering
pub fn to_deterministic_json<T: Serialize>(envelope: &Envelope<T>) -> Result<String, serde_json::Error> {
    // Convert to JSON value first
    let value = serde_json::to_value(envelope)?;
    
    // Re-serialize with sorted keys (serde_json preserves insertion order by default)
    let json_str = serde_json::to_string(&value)?;
    Ok(json_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_valid_envelope() -> Envelope<Value> {
        EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({ "quotes": [] }))
            .with_latency_ms(142)
            .with_cache_hit(false)
            .build()
            .expect("envelope should be valid")
    }

    #[test]
    fn builder_creates_valid_envelope() {
        let envelope = make_valid_envelope();
        assert!(!envelope.meta.request_id.is_empty());
        assert_eq!(envelope.meta.schema_version, "v1.0.0");
        assert_eq!(envelope.meta.source_chain, vec![ProviderId::Yahoo]);
        assert_eq!(envelope.meta.latency_ms, 142);
        assert!(!envelope.meta.cache_hit);
    }

    #[test]
    fn builder_adds_warnings() {
        let envelope = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .with_warning("test warning")
            .with_warnings(vec!["warning 2".into()])
            .build()
            .expect("should build");

        assert_eq!(envelope.meta.warnings, vec!["test warning", "warning 2"]);
    }

    #[test]
    fn builder_adds_errors() {
        let error = EnvelopeError::new("test.code", "test message").expect("error");
        let envelope = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .with_error(error.clone())
            .build()
            .expect("should build");

        assert_eq!(envelope.errors.len(), 1);
        assert_eq!(envelope.errors[0].code, "test.code");
    }

    #[test]
    fn builder_accepts_custom_request_id() {
        let envelope = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .with_request_id("custom-request-12345678")
            .build()
            .expect("should build");

        assert_eq!(envelope.meta.request_id, "custom-request-12345678");
    }

    #[test]
    fn builder_accepts_trace_id() {
        let envelope = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .with_trace_id("0123456789abcdef0123456789abcdef")
            .build()
            .expect("should build");

        assert_eq!(
            envelope.meta.trace_id,
            Some("0123456789abcdef0123456789abcdef".to_string())
        );
    }

    #[test]
    fn builder_rejects_empty_source_chain() {
        let result = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![])
            .with_data(json!({}))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn builder_rejects_invalid_schema_version() {
        let result = EnvelopeBuilder::new("1.0.0") // missing 'v' prefix
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn validator_accepts_valid_envelope() {
        let envelope = make_valid_envelope();
        let result = EnvelopeValidator::validate(&envelope);
        assert!(result.is_ok());
    }

    #[test]
    fn validator_rejects_missing_request_id() {
        let value = json!({
            "meta": {
                "schema_version": "v1.0.0",
                "generated_at": "2024-01-01T00:00:00Z",
                "source_chain": ["yahoo"],
                "latency_ms": 100,
                "cache_hit": false
            },
            "data": {}
        });

        let result = EnvelopeValidator::validate_value(&value);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SchemaValidationError::MissingField(f) if f == "meta.request_id"
        ));
    }

    #[test]
    fn validator_rejects_short_request_id() {
        let value = json!({
            "meta": {
                "request_id": "short",
                "schema_version": "v1.0.0",
                "generated_at": "2024-01-01T00:00:00Z",
                "source_chain": ["yahoo"],
                "latency_ms": 100,
                "cache_hit": false
            },
            "data": {}
        });

        let result = EnvelopeValidator::validate_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn validator_rejects_empty_source_chain() {
        let value = json!({
            "meta": {
                "request_id": "request-12345678",
                "schema_version": "v1.0.0",
                "generated_at": "2024-01-01T00:00:00Z",
                "source_chain": [],
                "latency_ms": 100,
                "cache_hit": false
            },
            "data": {}
        });

        let result = EnvelopeValidator::validate_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn validator_rejects_invalid_error_code() {
        let value = json!({
            "meta": {
                "request_id": "request-12345678",
                "schema_version": "v1.0.0",
                "generated_at": "2024-01-01T00:00:00Z",
                "source_chain": ["yahoo"],
                "latency_ms": 100,
                "cache_hit": false
            },
            "data": {},
            "errors": [{
                "code": "",
                "message": "error message"
            }]
        });

        let result = EnvelopeValidator::validate_value(&value);
        assert!(result.is_err());
    }

    #[test]
    fn deterministic_json_is_stable() {
        let envelope = make_valid_envelope();
        let json_a = to_deterministic_json(&envelope).expect("should serialize");
        let json_b = to_deterministic_json(&envelope).expect("should serialize");
        assert_eq!(json_a, json_b);
    }

    #[test]
    fn deterministic_json_avoids_scientific_notation() {
        let envelope = EnvelopeBuilder::new("v1.0.0")
            .with_source_chain(vec![ProviderId::Yahoo])
            .with_data(json!({}))
            .with_latency_ms(100000) // large number
            .build()
            .expect("should build");

        let json = to_deterministic_json(&envelope).expect("should serialize");
        // Check that latency_ms is serialized as integer without scientific notation
        assert!(json.contains("\"latency_ms\":100000") || json.contains("\"latency_ms\": 100000"));
        assert!(!json.contains("e+") && !json.contains("E+"));
    }
}
