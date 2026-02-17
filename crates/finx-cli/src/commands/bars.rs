use std::str::FromStr;

use finx_core::{BarSeries, BarsRequest, Interval, SourceRouter, SourceStrategy, Symbol};

use crate::cli::BarsArgs;
use crate::error::CliError;

use super::CommandResult;
use super::warehouse_sync;

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
            let series = route.data;
            let warehouse_warning = warehouse_sync::sync_bars(
                route.selected_source,
                series.interval,
                series.bars.as_slice(),
                series.symbol.as_str(),
                route.latency_ms,
            )
            .err()
            .map(|error| format!("warehouse sync (bars) failed: {error}"));
            let data = serde_json::to_value(series)?;
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
