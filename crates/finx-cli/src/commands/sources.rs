use serde::Serialize;

use finx_core::ProviderId;

use crate::cli::SourcesArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct SourceStatus {
    id: ProviderId,
    available: bool,
    status: &'static str,
    capabilities: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct SourcesResponseData {
    sources: Vec<SourceStatus>,
}

pub fn run(args: &SourcesArgs) -> Result<CommandResult, CliError> {
    let capabilities = if args.verbose {
        vec!["quote", "bars", "fundamentals", "search", "health"]
    } else {
        vec!["quote", "bars"]
    };

    let sources = ProviderId::ALL
        .into_iter()
        .map(|id| SourceStatus {
            id,
            available: true,
            status: "stub",
            capabilities: capabilities.clone(),
        })
        .collect::<Vec<_>>();

    let data = serde_json::to_value(SourcesResponseData { sources })?;

    Ok(CommandResult::ok(data)
        .with_warning("source health checks are not implemented in phase 0-1"))
}
