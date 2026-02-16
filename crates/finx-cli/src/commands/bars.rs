use std::str::FromStr;

use finx_core::{Bar, BarSeries, Interval, Symbol, UtcDateTime};

use crate::cli::BarsArgs;
use crate::error::CliError;

use super::CommandResult;

pub fn run(args: &BarsArgs) -> Result<CommandResult, CliError> {
    if args.limit == 0 {
        return Err(CliError::Command(String::from(
            "--limit must be greater than zero",
        )));
    }

    let symbol = Symbol::parse(&args.symbol)?;
    let interval = Interval::from_str(&args.interval)?;
    let ts = UtcDateTime::now();

    let bars = (0..args.limit)
        .map(|index| {
            let base = 100.0 + index as f64;
            Bar::new(
                ts,
                base,
                base + 1.0,
                base - 1.0,
                base + 0.25,
                Some(5_000 + index as u64),
                Some(base),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let data = BarSeries::new(symbol, interval, bars);

    Ok(CommandResult::ok(serde_json::to_value(data)?)
        .with_warning("bars command currently returns synthetic placeholder bars"))
}
