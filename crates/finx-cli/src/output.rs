pub mod stream_writer;

use std::io;

use finx_core::Envelope;
use serde_json::json;
use serde_json::Value;

use self::stream_writer::{NdjsonStreamWriter, StreamEventError};
use crate::cli::OutputFormat;
use crate::error::CliError;

pub fn render(
    envelope: &Envelope<Value>,
    format: OutputFormat,
    pretty: bool,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => {
            let payload = if pretty {
                serde_json::to_string_pretty(envelope)?
            } else {
                serde_json::to_string(envelope)?
            };
            println!("{payload}");
        }
        OutputFormat::Ndjson => {
            let payload = serde_json::to_string(envelope)?;
            println!("{payload}");
        }
        OutputFormat::Table => render_table(envelope)?,
    }

    Ok(())
}

pub fn render_stream(envelope: &Envelope<Value>, explain: bool) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut writer = NdjsonStreamWriter::new(stdout.lock());

    writer.emit_start(Some(json!({
        "request_id": envelope.meta.request_id,
        "trace_id": envelope.meta.trace_id,
        "schema_version": envelope.meta.schema_version,
    })))?;

    writer.emit_progress(Some(json!({
        "phase": "command_complete",
        "warning_count": envelope.meta.warnings.len(),
        "error_count": envelope.errors.len(),
    })))?;

    if explain {
        writer.emit_progress(Some(json!({
            "phase": "diagnostics",
            "explain": true,
        })))?;
    }

    writer.emit_chunk(Some(serde_json::to_value(envelope)?))?;

    for error in &envelope.errors {
        let mut stream_error = StreamEventError::new(error.code.clone(), error.message.clone());
        if let Some(retryable) = error.retryable {
            stream_error = stream_error.with_retryable(retryable);
        }

        let data = error
            .source
            .map(|source| json!({ "source": source.as_str() }));
        writer.emit_error(stream_error, data)?;
    }

    writer.emit_end(Some(json!({
        "status": if envelope.errors.is_empty() { "ok" } else { "error" },
        "warning_count": envelope.meta.warnings.len(),
        "error_count": envelope.errors.len(),
    })))?;

    Ok(())
}

fn render_table(envelope: &Envelope<Value>) -> Result<(), CliError> {
    println!("request_id  : {}", envelope.meta.request_id);
    if let Some(trace_id) = &envelope.meta.trace_id {
        println!("trace_id    : {trace_id}");
    }
    println!("schema      : {}", envelope.meta.schema_version);
    println!("generated_at: {}", envelope.meta.generated_at);
    println!(
        "sources     : {}",
        envelope
            .meta
            .source_chain
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("latency_ms  : {}", envelope.meta.latency_ms);
    println!("cache_hit   : {}", envelope.meta.cache_hit);

    if !envelope.meta.warnings.is_empty() {
        println!("warnings:");
        for warning in &envelope.meta.warnings {
            println!("  - {warning}");
        }
    }

    println!("data:");
    let pretty_data = serde_json::to_string_pretty(&envelope.data)?;
    for line in pretty_data.lines() {
        println!("  {line}");
    }

    if !envelope.errors.is_empty() {
        println!("errors:");
        for error in &envelope.errors {
            println!("  - {}: {}", error.code, error.message);
        }
    }

    Ok(())
}
