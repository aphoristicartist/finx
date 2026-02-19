use std::io::Write;

use finx_core::UtcDateTime;
use serde::Serialize;
use serde_json::Value;

use crate::error::CliError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamEventType {
    Start,
    Progress,
    Chunk,
    End,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StreamEventError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl StreamEventError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: None,
        }
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamEvent {
    pub event: StreamEventType,
    pub seq: u64,
    pub ts: UtcDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StreamEventError>,
}

pub struct NdjsonStreamWriter<W: Write> {
    writer: W,
    next_seq: u64,
}

impl<W: Write> NdjsonStreamWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            next_seq: 1,
        }
    }

    pub fn emit_start(&mut self, data: Option<Value>) -> Result<(), CliError> {
        self.emit(StreamEventType::Start, data, None)
    }

    pub fn emit_progress(&mut self, data: Option<Value>) -> Result<(), CliError> {
        self.emit(StreamEventType::Progress, data, None)
    }

    pub fn emit_chunk(&mut self, data: Option<Value>) -> Result<(), CliError> {
        self.emit(StreamEventType::Chunk, data, None)
    }

    pub fn emit_end(&mut self, data: Option<Value>) -> Result<(), CliError> {
        self.emit(StreamEventType::End, data, None)
    }

    pub fn emit_error(
        &mut self,
        error: StreamEventError,
        data: Option<Value>,
    ) -> Result<(), CliError> {
        self.emit(StreamEventType::Error, data, Some(error))
    }

    fn emit(
        &mut self,
        event: StreamEventType,
        data: Option<Value>,
        error: Option<StreamEventError>,
    ) -> Result<(), CliError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn emits_expected_event_sequence() {
        let mut sink = Vec::<u8>::new();

        {
            let mut writer = NdjsonStreamWriter::new(&mut sink);
            writer
                .emit_start(Some(json!({ "phase": "start" })))
                .expect("start");
            writer
                .emit_progress(Some(json!({ "pct": 50 })))
                .expect("progress");
            writer
                .emit_chunk(Some(json!({ "rows": 3 })))
                .expect("chunk");
            writer.emit_end(None).expect("end");
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let events = lines
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("json line"))
            .collect::<Vec<_>>();

        assert_eq!(events.len(), 4);
        assert_eq!(events[0].get("event"), Some(&Value::String("start".into())));
        assert_eq!(
            events[1].get("event"),
            Some(&Value::String("progress".into()))
        );
        assert_eq!(events[2].get("event"), Some(&Value::String("chunk".into())));
        assert_eq!(events[3].get("event"), Some(&Value::String("end".into())));
        assert_eq!(events[0].get("seq"), Some(&Value::from(1)));
        assert_eq!(events[3].get("seq"), Some(&Value::from(4)));
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
                    Some(json!({ "source": "polygon" })),
                )
                .expect("error event");
        }

        let lines = std::str::from_utf8(&sink).expect("utf8");
        let event = serde_json::from_str::<Value>(lines.trim()).expect("json");

        assert_eq!(event.get("event"), Some(&Value::String("error".into())));
        assert_eq!(
            event.pointer("/error/code"),
            Some(&Value::String("upstream_timeout".into()))
        );
        assert_eq!(event.pointer("/error/retryable"), Some(&Value::Bool(true)));
    }
}
