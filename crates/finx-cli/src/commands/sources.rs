use serde::Serialize;

use finx_core::{ProviderId, SourceRouter};

use crate::cli::SourcesArgs;
use crate::error::CliError;

use super::CommandResult;

const PROVIDER_OUTPUT_ORDER: [ProviderId; 4] = [
    ProviderId::Polygon,
    ProviderId::Alpaca,
    ProviderId::Alphavantage,
    ProviderId::Yahoo,
];

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

pub async fn run(
    args: &SourcesArgs,
    router: &SourceRouter,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    let mut sources = Vec::with_capacity(PROVIDER_OUTPUT_ORDER.len());
    for id in PROVIDER_OUTPUT_ORDER {
        let source_status = match router.snapshot(id).await {
            Some(snapshot) => {
                let capabilities = if args.verbose {
                    snapshot.capabilities.supported_endpoints()
                } else {
                    let mut compact = Vec::new();
                    if snapshot.capabilities.quote {
                        compact.push("quote");
                    }
                    if snapshot.capabilities.bars {
                        compact.push("bars");
                    }
                    compact
                };

                SourceStatus {
                    id,
                    available: snapshot.available(),
                    status: snapshot.status_label(),
                    capabilities,
                }
            }
            None => SourceStatus {
                id,
                available: false,
                status: "not_configured",
                capabilities: Vec::new(),
            },
        };
        sources.push(source_status);
    }

    let data = serde_json::to_value(SourcesResponseData { sources })?;

    Ok(CommandResult::ok(data, source_chain))
}
