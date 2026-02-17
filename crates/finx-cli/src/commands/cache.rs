use finx_core::{ProviderId, Warehouse};

use crate::cli::{CacheArgs, CacheCommand};
use crate::error::CliError;

use super::CommandResult;

pub fn run(args: &CacheArgs, source_chain: Vec<ProviderId>) -> Result<CommandResult, CliError> {
    match args.command {
        CacheCommand::Sync => {
            let warehouse =
                Warehouse::open_default().map_err(|error| CliError::Command(error.to_string()))?;
            let report = warehouse
                .sync_cache()
                .map_err(|error| CliError::Command(error.to_string()))?;
            let mut result = CommandResult::ok(serde_json::to_value(report)?, source_chain);
            if result_has_sync_failures(&result.data) {
                result = result.with_warning(String::from(
                    "cache sync completed with failures; inspect failed_partitions",
                ));
            }
            Ok(result)
        }
    }
}

fn result_has_sync_failures(data: &serde_json::Value) -> bool {
    data.get("failed_partitions")
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
        > 0
}
