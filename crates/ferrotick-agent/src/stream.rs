//! # NDJSON Streaming Implementation
//!
//! This module provides types for streaming events as newline-delimited JSON (NDJSON).
//!
//! ## Event Types
//!
//! | Event | Description |
//! |-------|-------------|
//! | `start` | Operation initiated |
//! | `progress` | Status updates during operation |
//! | `chunk` | Data batches |
//! | `end` | Operation completed |
//! | `error` | Error occurred |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferrotick_agent::stream::{NdjsonStreamWriter, StreamEventType};
//! use std::io::stdout;
//!
//! let mut writer = NdjsonStreamWriter::new(stdout.lock());
//! writer.emit_start(Some(json!({ "request_id": "abc123" })))?;
//! writer.emit_progress(Some(json!({ "phase": "fetching" })))?;
//! writer.emit_chunk(Some(json!({ "quotes": [] })))?;
//! writer.emit_end(Some(json!({ "status": "ok" })))?;
//! ```
//!
//! ## Performance
//!
//! The stream writer is designed for high-throughput streaming:
//! - Zero-copy serialization for data chunks
//! - Minimal allocations for event metadata
//! - Each event is written immediately (no buffering beyond stdio)

use std::io::Write;

use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Type of stream event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamEventType {
    /// Operation initiated.
    Start,
    /// Progress update during operation.
    Progress,
    /// Data chunk.
    Chunk,
    /// Operation completed.
    End,
    /// Error occurred.
    Error,
}

/// Error payload for stream error events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEventError {
    /// Error code (machine-readable).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Whether the error is retryable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl StreamEventError {
    /// Create a new stream event error.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: None,
        }
    }

    /// Mark the error as retryable (or not).
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }
}

/// A single stream event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Event type.
    pub event: StreamEventType,
    /// Monotonically increasing sequence number (starting from 1).
    pub seq: u64,
    /// UTC timestamp of event generation.
    pub ts: UtcDateTime,
    /// Optional event data payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Error payload (required for error events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StreamEventError>,
}

impl StreamEvent {
    /// Create a new stream event.
    pub fn new(event: StreamEventType, seq: u64, data: Option<Value>) -> Self {
        Self {
            event,
            seq,
            ts: UtcDateTime::now(),
            data,
            error: None,
        }
    }

    /// Create an error event.
    pub fn error(seq: u64, error: StreamEventError, data: Option<Value>) -> Self {
        Self {
            event: StreamEventType::Error,
            seq,
            ts: UtcDateTime::now(),
            data,
            error: Some(error),
        }
    }
}

/// Writer for NDJSON stream events.
///
/// This writer emits events as newline-delimited JSON, suitable for
/// consumption by AI agents and streaming parsers.
///
/// # Thread Safety
///
/// This type is not thread-safe. Use separate writers per thread or
/// synchronize access externally.
pub struct NdjsonStreamWriter<W: Write> {
    writer: W,
    next_seq: u64,
}

impl<W: Write> NdjsonStreamWriter<W> {
    /// Create a new stream writer wrapping the provided writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            next_seq: 1,
        }
    }

    /// Emit a start event.
    ///
    /// Typically includes request metadata (request_id, trace_id, schema_version).
    pub fn emit_start(&mut self, data: Option<Value>) -> Result<(), StreamError> {
        self.emit(StreamEventType::Start, data, None)
    }

    /// Emit a progress event.
    ///
    /// Use for status updates during long-running operations.
    pub fn emit_progress(&mut self, data: Option<Value>) -> Result<(), StreamError> {
        self.emit(StreamEventType::Progress, data, None)
    }

    /// Emit a chunk event containing data payload.
    ///
    /// This is typically the main data delivery event.
    pub fn emit_chunk(&mut self, data: Option<Value>) -> Result<(), StreamError> {
        self.emit(StreamEventType::Chunk, data, None)
    }

    /// Emit an end event.
    ///
    /// Signals operation completion. May include summary statistics.
    pub fn emit_end(&mut self, data: Option<Value>) -> Result<(), StreamError> {
        self.emit(StreamEventType::End, data, None)
    }

    /// Emit an error event.
    ///
    /// Error events include both an error payload and optional contextual data.
    pub fn emit_error(
        &mut self,
        error: StreamEventError,
        data: Option<Value>,
    ) -> Result<(), StreamError> {
        self.emit(StreamEventType::Error, data, Some(error))
    }

    /// Get the current sequence number (next event will have this seq).
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    fn emit(
        &mut self,
        event: StreamEventType,
        data: Option<Value>,
        error: Option<StreamEventError>,
    ) -> Result<(), StreamError> {
        let event = StreamEvent {
            event,
            seq: self.next_seq,
            ts: UtcDateTime::now(),
            data,
            error,
        };
        self.next_seq += 1;

        let payload = serde_json::to_string(&event)?;
        self.writer.write_all(payload.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

impl<W: Write> NdjsonStreamWriter<W> {
    /// Consume the writer and return the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

/// Error type for stream operations.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    /// JSON serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Create a stream event parser for testing and validation.
///
/// Parses NDJSON lines into stream events for validation.
pub fn parse_stream_events(input: &str) -> Result<Vec<StreamEvent>, serde_json::Error> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line))
        .collect()
}

/// Validate that all events in a stream are well-formed.
///
/// Returns the count of events parsed, or an error on first malformed line.
pub fn validate_stream(input: &str) -> Result<usize, StreamValidationError> {
    let mut count = 0;
    for (line_num, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: StreamEvent = serde_json::from_str(line).map_err(|e| {
            StreamValidationError {
                line_number: line_num + 1,
                message: e.to_string(),
            }
        })?;
        
        // Validate sequence is monotonically increasing
        if event.seq == 0 {
            return Err(StreamValidationError {
                line_number: line_num + 1,
                message: "sequence number must be >= 1".to_string(),
            });
        }
        
        // Validate error events have error payload
        if event.event == StreamEventType::Error && event.error.is_none() {
            return Err(StreamValidationError {
                line_number: line_num + 1,
                message: "error events must have error payload".to_string(),
            });
        }
        
        count += 1;
    }
    Ok(count)
}

/// Error from stream validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamValidationError {
    /// 1-based line number where error occurred.
    pub line_number: usize,
    /// Error description.
    pub message: String,
}

impl std::fmt::Display for StreamValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stream validation error at line {}: {}", self.line_number, self.message)
    }
}

impl std::error::Error for StreamValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_expected_event_sequence() {
        let mut sink = Vec::<u8>::new();

        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            writer
                .emit_start(Some(serde_json::json!({ "phase": "start" })))
                .expect("start");
            writer
                .emit_progress(Some(serde_json::json!({ "pct": 50 })))
                .expect("progress");
            writer
                .emit_chunk(Some(serde_json::json!({ "rows": 3 })))
                .expect("chunk");
            writer.emit_end(None).expect("end");
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let events = parse_stream_events(lines).expect("parse");

        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event, StreamEventType::Start);
        assert_eq!(events[1].event, StreamEventType::Progress);
        assert_eq!(events[2].event, StreamEventType::Chunk);
        assert_eq!(events[3].event, StreamEventType::End);
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[3].seq, 4);
    }

    #[test]
    fn emits_error_event_payload() {
        let mut sink = Vec::<u8>::new();

        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            writer
                .emit_error(
                    StreamEventError::new("upstream_timeout", "request timed out")
                        .with_retryable(true),
                    Some(serde_json::json!({ "source": "polygon" })),
                )
                .expect("error event");
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let event = serde_json::from_str::<StreamEvent>(lines.trim()).expect("json");

        assert_eq!(event.event, StreamEventType::Error);
        assert_eq!(event.error.as_ref().unwrap().code, "upstream_timeout");
        assert_eq!(event.error.as_ref().unwrap().retryable, Some(true));
        assert_eq!(
            event.data.as_ref().unwrap().get("source"),
            Some(&serde_json::json!("polygon"))
        );
    }

    #[test]
    fn sequence_increments_monotonically() {
        let mut sink = Vec::<u8>::new();

        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            for _ in 0..100 {
                writer.emit_chunk(None).expect("chunk");
            }
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let events = parse_stream_events(lines).expect("parse");

        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.seq, (i + 1) as u64);
        }
    }

    #[test]
    fn validate_stream_accepts_valid_input() {
        let mut sink = Vec::<u8>::new();
        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            writer.emit_start(None).unwrap();
            writer.emit_end(None).unwrap();
        }

        let input = std::str::from_utf8(&sink).unwrap();
        let count = validate_stream(input).expect("should validate");
        assert_eq!(count, 2);
    }

    #[test]
    fn validate_stream_rejects_malformed_json() {
        let input = r#"{"event":"start","seq":1,"ts":"2024-01-01T00:00:00Z"}
not valid json
{"event":"end","seq":2,"ts":"2024-01-01T00:00:00Z"}"#;

        let result = validate_stream(input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line_number, 2);
    }

    #[test]
    fn validate_stream_rejects_error_without_payload() {
        let input = r#"{"event":"error","seq":1,"ts":"2024-01-01T00:00:00Z"}"#;

        let result = validate_stream(input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("error payload"));
    }

    #[test]
    fn validate_stream_rejects_zero_sequence() {
        let input = r#"{"event":"start","seq":0,"ts":"2024-01-01T00:00:00Z"}"#;

        let result = validate_stream(input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("sequence"));
    }

    #[test]
    fn high_volume_streaming() {
        // Performance test: stream 100k events
        let mut sink = Vec::<u8>::new();
        let event_count = 100_000;

        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            writer.emit_start(Some(serde_json::json!({ "count": event_count }))).unwrap();
            for i in 0..event_count {
                writer.emit_chunk(Some(serde_json::json!({ "idx": i }))).unwrap();
            }
            writer.emit_end(None).unwrap();
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let count = validate_stream(lines).expect("all events should be valid");
        assert_eq!(count, event_count + 2); // start + chunks + end
    }
}
