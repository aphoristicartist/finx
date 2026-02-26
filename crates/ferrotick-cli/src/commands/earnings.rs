use serde::Serialize;

use ferrotick_core::{EarningsReport, SourceRouter, SourceStrategy, Symbol};

use crate::cli::EarningsArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct EarningsResponseData {
    earnings: EarningsReport,
}

pub async fn run(
    args: &EarningsArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    let symbol = Symbol::parse(&args.symbol)?;

    let request = ferrotick_core::EarningsRequest::new(symbol, args.limit)
        .map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_earnings(&request, strategy.clone()).await {
        Ok(route) => {
            let data = serde_json::to_value(EarningsResponseData {
                earnings: route.data.earnings,
            })?;

            Ok(CommandResult::ok(data, route.source_chain)
                .with_errors(route.errors)
                .with_warnings(route.warnings)
                .with_latency(route.latency_ms)
                .with_cache_hit(false))
        }
        Err(failure) => {
            // Create empty earnings report for error case
            let empty_report = EarningsReport::new(
                Symbol::parse(&args.symbol)?,
                "USD",
                ferrotick_core::UtcDateTime::now(),
                vec![],
            )
            .map_err(|e| CliError::Command(e.to_string()))?;

            let data = serde_json::to_value(EarningsResponseData {
                earnings: empty_report,
            })?;
            Ok(CommandResult::ok(data, failure.source_chain)
                .with_errors(failure.errors)
                .with_warnings(failure.warnings)
                .with_latency(failure.latency_ms)
                .with_cache_hit(false))
        }
    }
}
