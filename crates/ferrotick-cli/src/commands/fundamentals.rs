use serde::Serialize;

use ferrotick_core::{Fundamental, FundamentalsRequest, SourceRouter, SourceStrategy, Symbol};

use crate::cli::FundamentalsArgs;
use crate::error::CliError;

use super::warehouse_sync;
use super::CommandResult;

#[derive(Debug, Serialize)]
struct FundamentalsResponseData {
    fundamentals: Vec<Fundamental>,
}

pub async fn run(
    args: &FundamentalsArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    let symbols = args
        .symbols
        .iter()
        .map(|raw| Symbol::parse(raw))
        .collect::<Result<Vec<_>, _>>()?;

    let request =
        FundamentalsRequest::new(symbols).map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_fundamentals(&request, strategy.clone()).await {
        Ok(route) => {
            let fundamentals = route.data.fundamentals;
            let warehouse_warning = warehouse_sync::sync_fundamentals(
                route.selected_source,
                fundamentals.as_slice(),
                route.latency_ms,
            )
            .err()
            .map(|error| format!("warehouse sync (fundamentals) failed: {error}"));
            let data = serde_json::to_value(FundamentalsResponseData { fundamentals })?;

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
            let data = serde_json::to_value(FundamentalsResponseData {
                fundamentals: Vec::new(),
            })?;
            Ok(CommandResult::ok(data, failure.source_chain)
                .with_errors(failure.errors)
                .with_warnings(failure.warnings)
                .with_latency(failure.latency_ms)
                .with_cache_hit(false))
        }
    }
}
