use serde::Serialize;

use ferrotick_core::{Quote, QuoteRequest, SourceRouter, SourceStrategy, Symbol};

use crate::cli::QuoteArgs;
use crate::error::CliError;

use super::warehouse_sync;
use super::CommandResult;

#[derive(Debug, Serialize)]
struct QuoteResponseData {
    quotes: Vec<Quote>,
}

pub async fn run(
    args: &QuoteArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    let symbols = args
        .symbols
        .iter()
        .map(|raw| Symbol::parse(raw))
        .collect::<Result<Vec<_>, _>>()?;

    let request =
        QuoteRequest::new(symbols).map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_quote(&request, strategy.clone()).await {
        Ok(route) => {
            let quotes = route.data.quotes;
            let warehouse_warning = warehouse_sync::sync_quotes(
                route.selected_source,
                quotes.as_slice(),
                route.latency_ms,
            )
            .err()
            .map(|error| format!("warehouse sync (quote) failed: {error}"));
            let data = serde_json::to_value(QuoteResponseData { quotes })?;

            let mut result = CommandResult::ok(data, route.source_chain)
                .with_errors(route.errors)
                .with_warnings(route.warnings)
                .with_latency(route.latency_ms)
                .with_cache_hit(false);
            if let Some(warning) = warehouse_warning {
                result = result.with_warning(warning);
            }
            Ok(result)
        }
        Err(failure) => {
            let data = serde_json::to_value(QuoteResponseData { quotes: Vec::new() })?;
            Ok(CommandResult::ok(data, failure.source_chain)
                .with_errors(failure.errors)
                .with_warnings(failure.warnings)
                .with_latency(failure.latency_ms)
                .with_cache_hit(false))
        }
    }
}
