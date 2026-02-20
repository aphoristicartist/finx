use serde::{Deserialize, Serialize};

use crate::{ProviderId, UtcDateTime, ValidationError};

/// Standard response envelope for all `finx` machine-readable outputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub meta: EnvelopeMeta,
    pub data: T,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<EnvelopeError>,
}

impl<T> Envelope<T> {
    pub fn success(meta: EnvelopeMeta, data: T) -> Self {
        Self {
            meta,
            data,
            errors: Vec::new(),
        }
    }

    pub fn with_errors(
        meta: EnvelopeMeta,
        data: T,
        errors: Vec<EnvelopeError>,
    ) -> Result<Self, ValidationError> {
        meta.validate_schema_compliance()?;
        for error in &errors {
            error.validate()?;
        }

        Ok(Self { meta, data, errors })
    }

    pub fn push_error(&mut self, error: EnvelopeError) -> Result<(), ValidationError> {
        error.validate()?;
        self.errors.push(error);
        Ok(())
    }
}

/// Metadata attached to every envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeMeta {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub schema_version: String,
    pub generated_at: UtcDateTime,
    pub source_chain: Vec<ProviderId>,
    pub latency_ms: u64,
    pub cache_hit: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl EnvelopeMeta {
    pub fn new(
        request_id: impl Into<String>,
        schema_version: impl Into<String>,
        source_chain: Vec<ProviderId>,
        latency_ms: u64,
        cache_hit: bool,
    ) -> Result<Self, ValidationError> {
        let request_id = request_id.into();
        let schema_version = schema_version.into();
        let meta = Self {
            request_id,
            trace_id: None,
            schema_version,
            generated_at: UtcDateTime::now(),
            source_chain,
            latency_ms,
            cache_hit,
            warnings: Vec::new(),
        };
        meta.validate_schema_compliance()?;
        Ok(meta)
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Result<Self, ValidationError> {
        let trace_id = trace_id.into();
        if !is_valid_trace_id(trace_id.as_str()) {
            return Err(ValidationError::InvalidTraceId);
        }

        self.trace_id = Some(trace_id);
        self.validate_schema_compliance()?;
        Ok(self)
    }

    pub fn push_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Validates runtime metadata against `schemas/v1/envelope.schema.json` constraints.
    pub fn validate_schema_compliance(&self) -> Result<(), ValidationError> {
        if self.request_id.trim().len() < 8 {
            return Err(ValidationError::InvalidRequestId);
        }

        if let Some(trace_id) = &self.trace_id {
            if !is_valid_trace_id(trace_id.as_str()) {
                return Err(ValidationError::InvalidTraceId);
            }
        }

        if !is_valid_schema_version(&self.schema_version) {
            return Err(ValidationError::InvalidSchemaVersion {
                value: self.schema_version.clone(),
            });
        }

        if self.source_chain.is_empty() {
            return Err(ValidationError::EmptySourceChain);
        }

        Ok(())
    }
}

/// Structured error payload for partial or failed responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ProviderId>,
}

impl EnvelopeError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        let error = Self {
            code: code.into(),
            message: message.into(),
            retryable: None,
            source: None,
        };
        error.validate()?;
        Ok(error)
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    pub fn with_source(mut self, source: ProviderId) -> Self {
        self.source = Some(source);
        self
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.code.trim().is_empty() {
            return Err(ValidationError::EmptyErrorCode);
        }

        if self.message.trim().is_empty() {
            return Err(ValidationError::EmptyErrorMessage);
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
        part.is_some_and(|segment| {
            !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit())
        })
    })
}

fn is_valid_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.chars().all(|ch| ch.is_ascii_hexdigit())
        && value.chars().any(|ch| ch != '0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_meta() {
        let meta = EnvelopeMeta::new("request-12345", "v1.0.0", vec![ProviderId::Yahoo], 11, true)
            .expect("meta should be valid");

        assert_eq!(meta.schema_version, "v1.0.0");
    }

    #[test]
    fn rejects_bad_schema_version() {
        let err = EnvelopeMeta::new("request-12345", "1.0.0", vec![ProviderId::Yahoo], 1, false)
            .expect_err("must fail");
        assert!(matches!(err, ValidationError::InvalidSchemaVersion { .. }));
    }

    #[test]
    fn rejects_empty_error_code() {
        let err = EnvelopeError::new("", "message").expect_err("must fail");
        assert!(matches!(err, ValidationError::EmptyErrorCode));
    }

    #[test]
    fn rejects_invalid_trace_id() {
        let meta = EnvelopeMeta::new("request-12345", "v1.0.0", vec![ProviderId::Yahoo], 1, false)
            .expect("meta must be valid");

        let err = meta.with_trace_id("not-a-trace-id").expect_err("must fail");
        assert!(matches!(err, ValidationError::InvalidTraceId));
    }
}
