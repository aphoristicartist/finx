use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{SchemaArgs, SchemaCommand};
use crate::error::CliError;

use super::CommandResult;

const SCHEMA_DIR: &str = "schemas/v1";
const KNOWN_SCHEMAS: [&str; 6] = [
    "envelope.schema.json",
    "quote.response.schema.json",
    "bars.response.schema.json",
    "fundamentals.response.schema.json",
    "sql.response.schema.json",
    "stream.event.schema.json",
];

#[derive(Debug, Serialize)]
struct SchemaListResponseData {
    schemas: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct SchemaGetResponseData {
    name: String,
    path: String,
    schema: serde_json::Value,
}

pub fn run(args: &SchemaArgs) -> Result<CommandResult, CliError> {
    match &args.command {
        SchemaCommand::List => {
            let data = SchemaListResponseData {
                schemas: KNOWN_SCHEMAS.to_vec(),
            };
            Ok(CommandResult::ok(serde_json::to_value(data)?))
        }
        SchemaCommand::Get(get_args) => {
            let file_name = resolve_schema_file_name(&get_args.name);
            let path = PathBuf::from(SCHEMA_DIR).join(&file_name);

            if !path_exists(&path) {
                return Err(CliError::Command(format!(
                    "schema '{}' not found under {}",
                    get_args.name, SCHEMA_DIR
                )));
            }

            let content = fs::read_to_string(&path)?;
            let schema = serde_json::from_str::<serde_json::Value>(&content)?;

            let data = SchemaGetResponseData {
                name: file_name,
                path: path.display().to_string(),
                schema,
            };

            Ok(CommandResult::ok(serde_json::to_value(data)?))
        }
    }
}

fn resolve_schema_file_name(input: &str) -> String {
    let normalized = input.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "envelope" => String::from("envelope.schema.json"),
        "quote" => String::from("quote.response.schema.json"),
        "bars" => String::from("bars.response.schema.json"),
        "fundamentals" => String::from("fundamentals.response.schema.json"),
        "sql" => String::from("sql.response.schema.json"),
        "stream" | "stream-event" => String::from("stream.event.schema.json"),
        other if other.ends_with(".json") => other.to_owned(),
        other => format!("{other}.schema.json"),
    }
}

fn path_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}
