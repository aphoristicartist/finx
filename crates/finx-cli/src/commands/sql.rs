use serde::Serialize;
use serde_json::Value;

use finx_core::ProviderId;
use finx_warehouse::{QueryGuardrails, Warehouse};

use crate::cli::SqlArgs;
use crate::error::CliError;

use super::CommandResult;

#[derive(Debug, Serialize)]
struct SqlColumn {
    name: String,
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Debug, Serialize)]
struct SqlResponseData {
    columns: Vec<SqlColumn>,
    rows: Vec<Vec<Value>>,
    row_count: usize,
    truncated: bool,
}

pub fn run(
    args: &SqlArgs,
    explain: bool,
    source_chain: Vec<ProviderId>,
) -> Result<CommandResult, CliError> {
    let query = args.query.trim();
    if query.is_empty() {
        return Err(CliError::Command(String::from("query must not be empty")));
    }

    // Open warehouse
    let warehouse = Warehouse::open_default()
        .map_err(|e| CliError::Command(format!("failed to open warehouse: {}", e)))?;

    // Execute query with guardrails
    let guardrails = QueryGuardrails {
        max_rows: args.max_rows,
        query_timeout_ms: args.query_timeout_ms,
    };

    let result = warehouse
        .execute_query(query, guardrails, args.write)
        .map_err(|e| CliError::Command(format!("query execution failed: {}", e)))?;

    // Transform result into response format
    let data = SqlResponseData {
        columns: result
            .columns
            .into_iter()
            .map(|col| SqlColumn {
                name: col.name,
                r#type: col.r#type,
            })
            .collect(),
        rows: result.rows,
        row_count: result.row_count,
        truncated: result.truncated,
    };

    let mut command_result = CommandResult::ok(serde_json::to_value(&data)?, source_chain);

    // Add warning if results were truncated
    if data.truncated {
        command_result = command_result.with_warning(format!(
            "result truncated at {} rows (use --max-rows to increase limit)",
            data.row_count
        ));
    }

    if explain {
        let explain_sql = format!("EXPLAIN {query}");
        let explain_guardrails = QueryGuardrails {
            max_rows: args.max_rows.clamp(1, 256),
            query_timeout_ms: args.query_timeout_ms,
        };

        match warehouse.execute_query(explain_sql.as_str(), explain_guardrails, false) {
            Ok(explain_result) => {
                let plan_lines = explain_result
                    .rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(format_sql_value)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    })
                    .collect::<Vec<_>>();

                if plan_lines.is_empty() {
                    command_result =
                        command_result.with_warning("explain: planner returned no diagnostics");
                } else {
                    for line in plan_lines {
                        command_result = command_result.with_warning(format!("explain: {line}"));
                    }
                }

                if explain_result.truncated {
                    command_result = command_result.with_warning(
                        "explain: diagnostics truncated by --max-rows (increase limit to view full plan)",
                    );
                }
            }
            Err(error) => {
                command_result = command_result
                    .with_warning(format!("explain: failed to build query plan: {error}"));
            }
        }
    }

    Ok(command_result)
}

fn format_sql_value(value: &Value) -> String {
    match value {
        Value::Null => String::from("null"),
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}
