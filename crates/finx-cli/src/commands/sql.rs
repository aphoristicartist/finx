use serde::Serialize;
use serde_json::Value;

use finx_core::ProviderId;

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

pub fn run(args: &SqlArgs, source_chain: Vec<ProviderId>) -> Result<CommandResult, CliError> {
    let query = args.query.trim();
    if query.is_empty() {
        return Err(CliError::Command(String::from("query must not be empty")));
    }

    let rows = vec![vec![
        Value::String(String::from("placeholder")),
        Value::Number(1.into()),
    ]];

    let data = SqlResponseData {
        columns: vec![
            SqlColumn {
                name: String::from("status"),
                r#type: String::from("TEXT"),
            },
            SqlColumn {
                name: String::from("value"),
                r#type: String::from("BIGINT"),
            },
        ],
        row_count: rows.len(),
        rows,
        truncated: false,
    };

    Ok(
        CommandResult::ok(serde_json::to_value(data)?, source_chain).with_warning(format!(
            "sql execution is stubbed in phase 0-1 (query accepted, max_rows={}, timeout_ms={})",
            args.max_rows, args.query_timeout_ms
        )),
    )
}
