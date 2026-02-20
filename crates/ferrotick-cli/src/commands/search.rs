use serde::Serialize;

use ferrotick_core::{Instrument, SearchRequest, SourceRouter, SourceStrategy};

use crate::cli::SearchArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct SearchResponseData {
    query: String,
    results: Vec<Instrument>,
}

pub async fn run(
    args: &SearchArgs,
    router: &SourceRouter,
    strategy: &SourceStrategy,
) -> Result<CommandResult, CliError> {
    if args.limit == 0 {
        return Err(CliError::Command(String::from(
            "--limit must be greater than zero",
        )));
    }

    let query = args.query.trim();
    if query.is_empty() {
        return Err(CliError::Command(String::from("query must not be empty")));
    }

    let request = SearchRequest::new(query, args.limit)
        .map_err(|error| CliError::Command(error.to_string()))?;

    match router.route_search(&request, strategy.clone()).await {
        Ok(route) => {
            let data = serde_json::to_value(SearchResponseData {
                query: route.data.query,
                results: route.data.results,
            })?;
            Ok(CommandResult::ok(data, route.source_chain)
                .with_errors(route.errors)
                .with_warnings(route.warnings)
                .with_latency(route.latency_ms)
                .with_cache_hit(false))
        }
        Err(failure) => {
            let data = serde_json::to_value(SearchResponseData {
                query: query.to_owned(),
                results: Vec::new(),
            })?;
            Ok(CommandResult::ok(data, failure.source_chain)
                .with_errors(failure.errors)
                .with_warnings(failure.warnings)
                .with_latency(failure.latency_ms)
                .with_cache_hit(false))
        }
    }
}
