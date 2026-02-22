//! Load historical data from providers into the warehouse cache.

use ferrotick_core::{
    BarSeries, BarsRequest, Interval, ProviderId, SourceRouter, SourceStrategy, Symbol, UtcDateTime,
};
use ferrotick_warehouse::BarRecord;

use crate::cli::CacheLoadArgs;
use crate::error::CliError;

use super::CommandResult;

pub async fn run(
    args: &CacheLoadArgs,
    router: &SourceRouter,
    strategy: SourceStrategy,
) -> Result<CommandResult, CliError> {
    let symbol = Symbol::parse(&args.symbol)
        .map_err(|e| CliError::Validation(e))?;

    let warehouse = ferrotick_warehouse::Warehouse::open_default()
        .map_err(|error| CliError::Command(error.to_string()))?;

    let days = args.days.as_deref().unwrap_or("30").parse::<u32>().unwrap_or(30);
    let interval = match args.interval.as_str() {
        "1m" => Interval::OneMinute,
        "5m" => Interval::FiveMinutes,
        "15m" => Interval::FifteenMinutes,
        "1h" => Interval::OneHour,
        "1d" => Interval::OneDay,
        _ => Interval::OneDay,
    };

    let limit = (days * 24) as usize; // Approximate bars per day
    let now = UtcDateTime::now();
    let request_id = format!("cache_load:{}:{}", args.symbol, now.into_inner().unix_timestamp());

    // Fetch bars
    let bars_request = BarsRequest::new(symbol.clone(), interval, limit)?;
    let route_result = router.route_bars(&bars_request, strategy).await;

    let data = match route_result {
        Ok(result) => {
            let bars = result.data;
            let source_chain = result.source_chain.clone();

            // Ingest into warehouse
            if !bars.bars.is_empty() {
                let bar_records: Vec<BarRecord> = bars
                    .bars
                    .iter()
                    .map(|bar| BarRecord {
                        symbol: bars.symbol.as_str().to_string(),
                        ts: bar.ts.format_rfc3339(),
                        open: bar.open,
                        high: bar.high,
                        low: bar.low,
                        close: bar.close,
                        volume: bar.volume,
                    })
                    .collect();

                warehouse
                    .ingest_bars(
                        source_chain
                            .first()
                            .unwrap_or(&ProviderId::Yahoo)
                            .as_str(),
                        &format!("bars_{}", interval_str(interval)),
                        &request_id,
                        &bar_records,
                        result.latency_ms as u64,
                    )
                    .map_err(|error| CliError::Command(error.to_string()))?;

                eprintln!("✓ Cached {} bars to warehouse", bars.bars.len());
            }

            let response_value = serde_json::to_value(CacheLoadResponse {
                symbol: args.symbol.clone(),
                days,
                interval: args.interval.clone(),
                bars_loaded: bars.bars.len(),
                source_chain: source_chain.clone(),
                cached_at: UtcDateTime::now().format_rfc3339(),
            })?;

            return Ok(CommandResult::ok(response_value, source_chain));
        }
        Err(failure) => {
            return Ok(
                CommandResult::ok(
                    serde_json::to_value(CacheLoadResponse {
                        symbol: args.symbol.clone(),
                        days,
                        interval: args.interval.clone(),
                        bars_loaded: 0,
                        source_chain: failure.source_chain.clone(),
                        cached_at: UtcDateTime::now().format_rfc3339(),
                    })?,
                    failure.source_chain,
                )
                .with_errors(failure.errors)
            );
        }
    };

    Ok(CommandResult::ok(data, vec![]))
}

#[derive(Debug, serde::Serialize)]
struct CacheLoadResponse {
    symbol: String,
    days: u32,
    interval: String,
    bars_loaded: usize,
    source_chain: Vec<ProviderId>,
    cached_at: String,
}

fn interval_str(interval: Interval) -> &'static str {
    match interval {
        Interval::OneMinute => "1m",
        Interval::FiveMinutes => "5m",
        Interval::FifteenMinutes => "15m",
        Interval::OneHour => "1h",
        Interval::OneDay => "1d",
    }
}
