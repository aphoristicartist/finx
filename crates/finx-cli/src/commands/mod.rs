mod bars;
mod cache;
mod fundamentals;
mod quote;
mod schema;
mod search;
mod sources;
mod sql;
mod warehouse_sync;

use finx_core::{Endpoint, Envelope, EnvelopeMeta, ProviderId, SourceRouter, SourceStrategy};
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
    pub source_chain: Vec<ProviderId>,
}

impl CommandResult {
    pub fn ok(data: Value, source_chain: Vec<ProviderId>) -> Self {
        Self {
            data,
            warnings: Vec::new(),
            errors: Vec::new(),
            latency_ms: 0,
            cache_hit: true,
            source_chain,
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    pub fn with_errors(mut self, errors: Vec<finx_core::EnvelopeError>) -> Self {
        self.errors.extend(errors);
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn with_cache_hit(mut self, cache_hit: bool) -> Self {
        self.cache_hit = cache_hit;
        self
    }
}

pub async fn run(cli: &Cli) -> Result<Envelope<Value>, CliError> {
    let router = SourceRouter::default();
    let strategy = to_source_strategy(cli.source);

    let command_result = match &cli.command {
        Command::Quote(args) => quote::run(args, &router, &strategy).await?,
        Command::Bars(args) => bars::run(args, &router, &strategy).await?,
        Command::Fundamentals(args) => fundamentals::run(args, &router, &strategy).await?,
        Command::Search(args) => search::run(args, &router, &strategy).await?,
        Command::Sql(args) => {
            sql::run(args, non_provider_source_chain(&router, &strategy).await)?
        }
        Command::Cache(args) => {
            cache::run(args, non_provider_source_chain(&router, &strategy).await)?
        }
        Command::Schema(args) => {
            schema::run(args, non_provider_source_chain(&router, &strategy).await)?
        }
        Command::Sources(args) => {
            sources::run(args, &router, non_provider_source_chain(&router, &strategy).await)
                .await?
        }
    };

    let CommandResult {
        data,
        warnings,
        errors,
        latency_ms,
        cache_hit,
        source_chain,
    } = command_result;

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

    for warning in warnings {
        meta.push_warning(warning);
    }

    Envelope::with_errors(meta, data, errors).map_err(CliError::from)
}

fn to_source_strategy(source: SourceSelector) -> SourceStrategy {
    match source {
        SourceSelector::Auto => SourceStrategy::Auto,
        SourceSelector::Yahoo => SourceStrategy::Strict(ProviderId::Yahoo),
        SourceSelector::Polygon => SourceStrategy::Strict(ProviderId::Polygon),
        SourceSelector::Alphavantage => SourceStrategy::Strict(ProviderId::Alphavantage),
        SourceSelector::Alpaca => SourceStrategy::Strict(ProviderId::Alpaca),
    }
}

async fn non_provider_source_chain(
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Vec<ProviderId> {
    router.source_chain_for_strategy(Endpoint::Quote, strategy).await
}
