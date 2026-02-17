use std::str::FromStr;

use finx_core::{BarSeries, BarsRequest, Interval, SourceRouter, SourceStrategy, Symbol};

use crate::cli::BarsArgs;
use crate::error::CliError;

use super::CommandResult;

pub async fn run(
    args: &BarsArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    if args.limit == 0 {
        return Err(CliError::Command(String::from(
            "--limit must be greater than zero",
        )));
    }

    let symbol = Symbol::parse(&args.symbol)?;
    let interval = Interval::from_str(&args.interval)?;
    let request = BarsRequest::new(symbol.clone(), interval, args.limit)
        .map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_bars(&request, strategy.clone()).await {
        Ok(route) => {
            let data = serde_json::to_value(route.data)?;
            Ok(CommandResult::ok(data, route.source_chain)
                .with_errors(route.errors)
                .with_warnings(route.warnings)
                .with_latency(route.latency_ms)
                .with_cache_hit(false))
        }
        Err(failure) => {
            let empty_series = BarSeries::new(symbol, interval, Vec::new());
            Ok(
                CommandResult::ok(serde_json::to_value(empty_series)?, failure.source_chain)
                    .with_errors(failure.errors)
                    .with_warnings(failure.warnings)
                    .with_latency(failure.latency_ms)
                    .with_cache_hit(false),
            )
        }
    }
}
