use std::path::PathBuf;

use ferrotick_core::{ProviderId, Symbol, UtcDateTime};
use ferrotick_ml::{FeatureConfig, FeatureEngineer, FeatureStore, IndicatorSelection};

use crate::cli::{MlArgs, MlCommand, MlExportArgs, MlFeaturesArgs};
use crate::error::CliError;

use super::CommandResult;

pub async fn run(args: &MlArgs, source_chain: Vec<ProviderId>) -> Result<CommandResult, CliError> {
    match &args.command {
        MlCommand::Features(features_args) => run_features(features_args, source_chain).await,
        MlCommand::Export(export_args) => run_export(export_args, source_chain).await,
    }
}

async fn run_features(
    args: &MlFeaturesArgs,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    if args.window == 0 {
        return Err(CliError::Command(String::from(
            "--window must be greater than zero",
        )));
    }

    if args.output.trim().to_ascii_lowercase() != "json" {
        return Err(CliError::Command(String::from(
            "ml features supports only --output json in Phase 7",
        )));
    }

    let symbol = Symbol::parse(&args.symbol)?;
    let start = parse_optional_cli_date(args.start.as_deref(), false)?;
    let end = parse_optional_cli_date(args.end.as_deref(), true)?;
    validate_range(start, end)?;

    let indicators = IndicatorSelection::from_csv(&args.indicators)
        .map_err(|err| CliError::Command(err.to_string()))?;

    let store = FeatureStore::open_default().map_err(|err| CliError::Command(err.to_string()))?;
    let bars = store
        .load_daily_bars(&symbol, start, end)
        .map_err(|err| CliError::Command(err.to_string()))?;

    if bars.is_empty() {
        return Ok(
            CommandResult::ok(
                serde_json::json!({
                    "symbol": symbol.as_str(),
                    "rows_computed": 0,
                    "stored_rows": 0,
                    "features": []
                }),
                source_chain,
            )
            .with_warning("no bars found in warehouse; run `ferrotick cache load <symbol>` first"),
        );
    }

    let mut config = FeatureConfig::default();
    config.window = args.window;
    config.bb_period = args.window;

    let engineer = FeatureEngineer::new(config, indicators)
        .map_err(|err| CliError::Command(err.to_string()))?;

    let rows = engineer
        .compute_for_symbol(&symbol, &bars)
        .map_err(|err| CliError::Command(err.to_string()))?;

    let stored_rows = store
        .upsert_features(&rows)
        .map_err(|err| CliError::Command(err.to_string()))?;

    Ok(CommandResult::ok(
        serde_json::json!({
            "symbol": symbol.as_str(),
            "rows_computed": rows.len(),
            "stored_rows": stored_rows,
            "start": start.map(UtcDateTime::format_rfc3339),
            "end": end.map(UtcDateTime::format_rfc3339),
            "features": rows,
        }),
        source_chain,
    ))
}

async fn run_export(
    args: &MlExportArgs,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    let symbol = Symbol::parse(&args.symbol)?;

    let start = parse_optional_cli_date(args.start.as_deref(), false)?
        .unwrap_or(UtcDateTime::parse("1970-01-01T00:00:00Z")?);
    let end = parse_optional_cli_date(args.end.as_deref(), true)?
        .unwrap_or(UtcDateTime::now());

    validate_range(Some(start), Some(end))?;

    let path = PathBuf::from(&args.output);
    let store = FeatureStore::open_default().map_err(|err| CliError::Command(err.to_string()))?;

    store
        .export_features_parquet(symbol.as_str(), start, end, path.as_path())
        .await
        .map_err(|err| CliError::Command(err.to_string()))?;

    Ok(CommandResult::ok(
        serde_json::json!({
            "symbol": symbol.as_str(),
            "output": path,
            "start": start.format_rfc3339(),
            "end": end.format_rfc3339(),
            "exported": true,
        }),
        source_chain,
    ))
}

fn parse_optional_cli_date(
    raw: Option<&str>,
    end_of_day: bool,
) -> Result<Option<UtcDateTime>, CliError> {
    match raw {
        Some(value) => Ok(Some(parse_cli_date(value, end_of_day)?)),
        None => Ok(None),
    }
}

fn parse_cli_date(raw: &str, end_of_day: bool) -> Result<UtcDateTime, CliError> {
    let normalized = if raw.contains('T') {
        raw.to_string()
    } else if end_of_day {
        format!("{}T23:59:59Z", raw)
    } else {
        format!("{}T00:00:00Z", raw)
    };

    UtcDateTime::parse(&normalized).map_err(CliError::Validation)
}

fn validate_range(start: Option<UtcDateTime>, end: Option<UtcDateTime>) -> Result<(), CliError> {
    if let (Some(start), Some(end)) = (start, end) {
        if start > end {
            return Err(CliError::Command(String::from(
                "--start must be earlier than or equal to --end",
            )));
        }
    }
    Ok(())
}
