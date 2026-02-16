use serde::Serialize;

use finx_core::{Quote, Symbol, UtcDateTime};

use crate::cli::QuoteArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct QuoteResponseData {
    quotes: Vec<Quote>,
}

pub fn run(args: &QuoteArgs) -> Result<CommandResult, CliError> {
    let as_of = UtcDateTime::now();

    let quotes = args
        .symbols
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            let symbol = Symbol::parse(raw)?;
            let base = 100.0 + index as f64;
            Quote::new(
                symbol,
                base,
                Some(base - 0.05),
                Some(base + 0.05),
                Some(1_000 + (index as u64) * 10),
                "USD",
                as_of,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let data = serde_json::to_value(QuoteResponseData { quotes })?;

    Ok(CommandResult::ok(data)
        .with_warning("quote command currently returns deterministic stub data"))
}
