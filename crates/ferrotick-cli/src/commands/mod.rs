mod bars;
mod cache;
mod cache_load;
mod export;
mod fundamentals;
mod quote;
mod schema;
mod search;
mod sources;
mod sql;
mod warehouse_sync;

use ferrotick_core::{Endpoint, Envelope, ProviderId, SourceRouter, SourceRouterBuilder, SourceStrategy};
use serde_json::Value;

use crate::cli::{CacheCommand, Cli, Command, ExportArgs, SourceSelector};
use crate::error::CliError;
use crate::metadata::Metadata;

pub struct CommandResult {
    pub data: Value,
    pub warnings: Vec<String>,
    pub errors: Vec<ferrotick_core::EnvelopeError>,
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

    pub fn with_errors(mut self, errors: Vec<ferrotick_core::EnvelopeError>) -> Self {
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
    let router = if cli.mock {
        SourceRouterBuilder::new()
            .with_mock_mode()
            .build()
    } else {
        SourceRouterBuilder::new()
            .with_real_clients()
            .build()
    };
    let strategy = to_source_strategy(cli.source);

    let command_result = match &cli.command {
        Command::Quote(args) => quote::run(args, &router, &strategy).await?,
        Command::Bars(args) => bars::run(args, &router, &strategy).await?,
        Command::Fundamentals(args) => fundamentals::run(args, &router, &strategy).await?,
        Command::Search(args) => search::run(args, &router, &strategy).await?,
        Command::Sql(args) => sql::run(
            args,
            cli.explain,
            non_provider_source_chain(&router, &strategy).await,
        )?,
        Command::Export(args) => {
            export::run(args)?
        }
        Command::Cache(args) => {
            match &args.command {
                CacheCommand::Load(load_args) => {
                    cache_load::run(load_args, &router, strategy.clone()).await?
                }
                CacheCommand::Sync => {
                    cache::run(args, non_provider_source_chain(&router, &strategy).await)?
                }
            }
        }
        Command::Schema(args) => {
            schema::run(args, non_provider_source_chain(&router, &strategy).await)?
        }
        Command::Sources(args) => {
            sources::run(
                args,
                &router,
                non_provider_source_chain(&router, &strategy).await,
            )
            .await?
        }
    };

    let CommandResult {
        data,
        mut warnings,
        errors,
        latency_ms,
        cache_hit,
        source_chain,
    } = command_result;

    if cli.explain && !matches!(&cli.command, Command::Sql(_)) {
        warnings.push(String::from(
            "--explain currently applies to the 'sql' command",
        ));
    }

    let mut metadata = Metadata::new(source_chain, latency_ms, cache_hit)?;

    if cli.profile {
        metadata
            .push_warning("--profile is accepted but profiling is not implemented in this phase");
    }

    for warning in warnings {
        metadata.push_warning(warning);
    }

    let _deterministic_metadata_json = metadata.to_deterministic_json()?;
    let meta = metadata.into_envelope_meta("v1.0.0")?;

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
    router
        .source_chain_for_strategy(Endpoint::Quote, strategy)
        .await
}
