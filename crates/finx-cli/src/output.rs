use finx_core::Envelope;
use serde_json::Value;

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

fn render_table(envelope: &Envelope<Value>) -> Result<(), CliError> {
    println!("request_id  : {}", envelope.meta.request_id);
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
