use serde::Serialize;

use finx_core::{Fundamental, Symbol, UtcDateTime};

use crate::cli::FundamentalsArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct FundamentalsResponseData {
    fundamentals: Vec<Fundamental>,
}

pub fn run(args: &FundamentalsArgs) -> Result<CommandResult, CliError> {
    let as_of = UtcDateTime::now();

    let fundamentals = args
        .symbols
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            let symbol = Symbol::parse(raw)?;
            Fundamental::new(
                symbol,
                as_of,
                Some(1_000_000_000.0 + (index as f64) * 100_000_000.0),
                Some(20.0 + index as f64),
                Some(0.015),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let data = serde_json::to_value(FundamentalsResponseData { fundamentals })?;

    Ok(CommandResult::ok(data).with_warning(
        "fundamentals command currently returns deterministic placeholder fundamentals",
    ))
}
