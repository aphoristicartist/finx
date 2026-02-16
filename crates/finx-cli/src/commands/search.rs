use serde::Serialize;

use finx_core::{AssetClass, Instrument, Symbol};

use crate::cli::SearchArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct SearchResponseData {
    query: String,
    results: Vec<Instrument>,
}

pub fn run(args: &SearchArgs) -> Result<CommandResult, CliError> {
    if args.limit == 0 {
        return Err(CliError::Command(String::from(
            "--limit must be greater than zero",
        )));
    }

    let query = args.query.trim();
    if query.is_empty() {
        return Err(CliError::Command(String::from("query must not be empty")));
    }

    let catalog = vec![
        Instrument::new(
            Symbol::parse("AAPL")?,
            "Apple Inc.",
            Some(String::from("NASDAQ")),
            "USD",
            AssetClass::Equity,
            true,
        )?,
        Instrument::new(
            Symbol::parse("MSFT")?,
            "Microsoft Corporation",
            Some(String::from("NASDAQ")),
            "USD",
            AssetClass::Equity,
            true,
        )?,
        Instrument::new(
            Symbol::parse("SPY")?,
            "SPDR S&P 500 ETF Trust",
            Some(String::from("ARCA")),
            "USD",
            AssetClass::Etf,
            true,
        )?,
    ];

    let query_lower = query.to_ascii_lowercase();
    let results = catalog
        .into_iter()
        .filter(|instrument| {
            instrument
                .symbol
                .as_str()
                .to_ascii_lowercase()
                .contains(&query_lower)
                || instrument.name.to_ascii_lowercase().contains(&query_lower)
        })
        .take(args.limit)
        .collect::<Vec<_>>();

    let data = serde_json::to_value(SearchResponseData {
        query: query.to_owned(),
        results,
    })?;

    Ok(CommandResult::ok(data)
        .with_warning("search currently uses a static in-memory catalog in phase 0-1"))
}
