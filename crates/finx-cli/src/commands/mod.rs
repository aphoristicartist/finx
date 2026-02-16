mod bars;
mod fundamentals;
mod quote;
mod schema;
mod search;
mod sources;
mod sql;

use finx_core::{Envelope, EnvelopeMeta, ProviderId};
use serde_json::Value;
use uuid::Uuid;

use crate::cli::{Cli, Command, SourceSelector};
use crate::error::CliError;

pub struct CommandResult {
    pub data: Value,
    pub warnings: Vec<String>,
    pub errors: Vec<finx_core::EnvelopeError>,
    pub latency_ms: u64,
    pub cache_hit: bool,
}

impl CommandResult {
    pub fn ok(data: Value) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            errors: Vec::new(),
            latency_ms: 0,
            cache_hit: true,
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

pub fn run(cli: &Cli) -> Result<Envelope<Value>, CliError> {
    let command_result = match &cli.command {
        Command::Quote(args) => quote::run(args)?,
        Command::Bars(args) => bars::run(args)?,
        Command::Fundamentals(args) => fundamentals::run(args)?,
        Command::Search(args) => search::run(args)?,
        Command::Sql(args) => sql::run(args)?,
        Command::Schema(args) => schema::run(args)?,
        Command::Sources(args) => sources::run(args)?,
    };

    let CommandResult {
        data,
        warnings,
        errors,
        latency_ms,
        cache_hit,
    } = command_result;

    let (source_chain, source_warnings) = resolve_sources(cli.source);

    let mut meta = EnvelopeMeta::new(
        Uuid::new_v4().to_string(),
        "v1.0.0",
        source_chain,
        latency_ms,
        cache_hit,
    )?;

    if cli.profile {
        meta.push_warning("--profile is accepted but profiling is not implemented in this phase");
    }

    if cli.stream {
        meta.push_warning(
            "--stream is accepted but streaming output is not implemented in this phase",
        );
    }

    for warning in source_warnings {
        meta.push_warning(warning);
    }

    for warning in warnings {
        meta.push_warning(warning);
    }

    Envelope::with_errors(meta, data, errors).map_err(CliError::from)
}

fn resolve_sources(source: SourceSelector) -> (Vec<ProviderId>, Vec<String>) {
    match source {
        SourceSelector::Auto => (
            vec![ProviderId::Yahoo],
            vec![String::from(
                "source strategy 'auto' is stubbed to 'yahoo' until provider routing is implemented",
            )],
        ),
        SourceSelector::Yahoo => (vec![ProviderId::Yahoo], Vec::new()),
        SourceSelector::Polygon => (vec![ProviderId::Polygon], Vec::new()),
        SourceSelector::Alphavantage => (vec![ProviderId::Alphavantage], Vec::new()),
        SourceSelector::Alpaca => (vec![ProviderId::Alpaca], Vec::new()),
    }
}
