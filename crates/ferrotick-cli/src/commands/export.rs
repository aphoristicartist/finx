//! Export data from warehouse to CSV format.

use ferrotick_core::{ProviderId, QueryGuardrails, Warehouse};

use crate::cli::ExportArgs;

use crate::error::CliError;

use super::CommandResult;

pub fn run(args: &ExportArgs) -> Result<CommandResult, CliError> {
    let warehouse = Warehouse::open_default()
        .map_err(|error| CliError::Command(error.to_string()))?;

    // Default queries if table is specified
    let query = if let Some(table) = &args.table {
        match table.as_str() {
            "bars" | "bars_1d" | "bars_1m" => {
                if let Some(symbol) = &args.symbol {
                    format!("SELECT * FROM {} WHERE symbol = '{}' ORDER BY ts", table, symbol)
                } else {
                    format!("SELECT * FROM {} ORDER BY ts", table)
                }
            }
            "quotes" => {
                if let Some(symbol) = &args.symbol {
                    format!("SELECT * FROM quotes WHERE symbol = '{}'", symbol)
                } else {
                    "SELECT * FROM quotes".to_string()
                }
            }
            "fundamentals" => {
                if let Some(symbol) = &args.symbol {
                    format!("SELECT * FROM fundamentals WHERE symbol = '{}' ORDER BY date", symbol)
                } else {
                    "SELECT * FROM fundamentals ORDER BY date".to_string()
                }
            }
            _ => {
                return Err(CliError::Command(format!(
                    "unknown table '{}'. Valid tables: bars, quotes, fundamentals",
                    table
                )));
            }
        }
    } else {
        args.query.clone().unwrap_or_else(|| "SELECT 1 LIMIT 0".to_string())
    };

    let guardrails = QueryGuardrails {
        max_rows: args.max_rows.as_deref().unwrap_or("100000").parse::<usize>().unwrap_or(100000),
        query_timeout_ms: args.query_timeout_ms.as_deref().unwrap_or("30000").parse::<u64>().unwrap_or(30000),
    };

    let result = warehouse
        .execute_query(&query, guardrails, false)
        .map_err(|error| CliError::Command(error.to_string()))?;

    if result.rows.is_empty() {
        eprintln!("⚠ No data found for query");
        return Ok(CommandResult::ok(
            serde_json::json!({
                "query": query,
                "rows": 0,
                "exported": false
            }),
            vec![],
        ));
    }

    // Export based on format
    match args.export_format.as_str() {
        "csv" => {
            export_csv(&args.output, &result.columns, &result.rows)?;
        }
        "parquet" => {
            eprintln!("ℹ Parquet export requires additional dependencies. Using CSV format instead.");
            export_csv(&args.output, &result.columns, &result.rows)?;
        }
        _ => {
            return Err(CliError::Command(format!(
                "unsupported format '{}'. Valid formats: csv, parquet (exports as csv for now)",
                args.export_format
            )));
        }
    }

    Ok(CommandResult::ok(
        serde_json::json!({
            "query": query,
            "format": "csv",
            "output": args.output,
            "rows_exported": result.row_count,
            "exported": true
        }),
        vec![ProviderId::Yahoo], // Export uses warehouse which may contain data from various sources
    ))
}

fn export_csv(
    output_path: &str,
    columns: &[ferrotick_core::SqlColumn],
    rows: &[Vec<serde_json::Value>],
) -> Result<(), CliError> {
    use std::io::Write;
    use std::fs::File;
    use std::io::BufWriter;

    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);

    // Write header
    let header: Vec<String> = columns
        .iter()
        .map(|c| c.name.clone())
        .collect();
    writeln!(writer, "{}", header.join(","))?;

    // Write rows
    for row in rows {
        let values: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(i, _)| {
                row.get(i)
                    .map(|v| match v {
                        serde_json::Value::Null => String::new(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::String(s) => {
                            // Escape quotes and commas
                            format!("\"{}\"", s.replace("\"", "\"\""))
                        }
                        _ => String::new(),
                    })
                    .unwrap_or_default()
            })
            .collect();
        writeln!(writer, "{}", values.join(","))?;
    }

    writer.flush()?;
    eprintln!("✓ Exported {} rows to {}", rows.len(), output_path);
    Ok(())
}
