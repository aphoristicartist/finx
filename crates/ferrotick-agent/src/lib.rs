//! # ferrotick-agent
//!
//! AI-agent UX primitives for ferrotick: JSON envelopes, NDJSON streaming, and schema validation.
//!
//! ## Overview
//!
//! This crate provides the foundational components for making ferrotick first-class
//! for AI agents with strict JSON schemas, streaming events, and machine-readable output.
//!
//! ## Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`envelope`] | JSON envelope construction and validation |
//! | [`stream`] | NDJSON streaming implementation |
//! | [`schema_registry`] | Schema management and validation |
//! | [`metadata`] | Request tracking with request_id, trace_id, etc. |
//!
//! ## Features
//!
//! - **Strict JSON envelope**: All commands emit valid schema-compliant JSON
//! - **NDJSON streaming**: Event types (start, progress, chunk, end, error) with sequence numbers
//! - **Schema introspection**: List and retrieve bundled JSON schemas
//! - **Machine metadata**: request_id, trace_id, source_chain, latency_ms, cache_hit, warnings
//! - **Deterministic output**: Consistent field ordering, stable decimal formatting
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ferrotick_agent::{EnvelopeBuilder, StreamEvent, NdjsonStreamWriter};
//!
//! // Create an envelope
//! let envelope = EnvelopeBuilder::new("v1.0.0")
//!     .with_data(json!({ "quotes": [] }))
//!     .build()?;
//!
//! // Stream events
//! let mut writer = NdjsonStreamWriter::new(stdout.lock());
//! writer.emit_start(Some(json!({ "request_id": envelope.meta.request_id })))?;
//! writer.emit_chunk(Some(envelope))?;
//! writer.emit_end(None)?;
//! ```

pub mod envelope;
pub mod metadata;
pub mod schema_registry;
pub mod stream;

// Re-export commonly used types at crate root
pub use envelope::{EnvelopeBuilder, EnvelopeValidator};
pub use metadata::{AgentMetadata, RequestId, TraceId};
pub use schema_registry::{SchemaRegistry, SchemaValidationError};
pub use stream::{
    NdjsonStreamWriter, StreamEvent, StreamEventError, StreamEventType,
};
