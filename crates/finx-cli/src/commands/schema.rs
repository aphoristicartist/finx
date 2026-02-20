use std::fs;
use std::path::{Path, PathBuf};

use finx_core::ProviderId;
use serde::Serialize;

use crate::cli::{SchemaArgs, SchemaCommand};
use crate::error::CliError;

use super::CommandResult;

const SCHEMA_DIR: &str = "schemas/v1";

#[derive(Debug, Serialize)]
struct SchemaListResponseData {
    schemas: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SchemaGetResponseData {
    name: String,
    path: String,
    schema: serde_json::Value,
}

pub fn run(args: &SchemaArgs, source_chain: Vec<ProviderId>) -> Result<CommandResult, CliError> {
    match &args.command {
        SchemaCommand::List => {
            let schemas = list_available_schema_names()?;
            let data = SchemaListResponseData { schemas };
            Ok(CommandResult::ok(
                serde_json::to_value(data)?,
                source_chain.clone(),
            ))
        }
        SchemaCommand::Get(get_args) => {
            let file_name = resolve_schema_file_name(&get_args.name)?;
            let path = resolve_schema_path(&file_name, &get_args.name)?;
            let content = fs::read_to_string(&path)?;
            let schema = serde_json::from_str::<serde_json::Value>(&content)?;

            let data = SchemaGetResponseData {
                name: file_name,
                path: path.display().to_string(),
                schema,
            };

            Ok(CommandResult::ok(serde_json::to_value(data)?, source_chain))
        }
    }
}

fn list_available_schema_names() -> Result<Vec<String>, CliError> {
    let mut names = Vec::new();
    for entry in fs::read_dir(SCHEMA_DIR)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        if file_name.ends_with(".json") {
            names.push(file_name.to_owned());
        }
    }

    names.sort();
    Ok(names)
}

fn resolve_schema_file_name(input: &str) -> Result<String, CliError> {
    let normalized = input.trim().to_ascii_lowercase();
    let file_name = match normalized.as_str() {
        "envelope" => String::from("envelope.schema.json"),
        "quote" => String::from("quote.response.schema.json"),
        "bars" => String::from("bars.response.schema.json"),
        "fundamentals" => String::from("fundamentals.response.schema.json"),
        "sql" => String::from("sql.response.schema.json"),
        "stream" | "stream-event" => String::from("stream.event.schema.json"),
        other if other.ends_with(".json") => other.to_owned(),
        other if other.contains(".schema.") || other.contains(".response.") => {
            format!("{other}.json")
        }
        other => format!("{other}.schema.json"),
    };

    if !is_safe_schema_file_name(&file_name) {
        return Err(CliError::Command(format!(
            "invalid schema name '{}'",
            input.trim()
        )));
    }

    Ok(file_name)
}

fn resolve_schema_path(file_name: &str, original_name: &str) -> Result<PathBuf, CliError> {
    let schema_root = fs::canonicalize(SCHEMA_DIR)?;
    let candidate = schema_root.join(file_name);

    if !candidate.exists() || !candidate.is_file() {
        return Err(CliError::Command(format!(
            "schema '{}' not found under {}",
            original_name, SCHEMA_DIR
        )));
    }

    let canonical_candidate = fs::canonicalize(&candidate)?;
    if !canonical_candidate.starts_with(&schema_root) {
        return Err(CliError::Command(format!(
            "schema '{}' resolves outside {}",
            original_name, SCHEMA_DIR
        )));
    }

    Ok(canonical_candidate)
}

fn is_safe_schema_file_name(file_name: &str) -> bool {
    if file_name.is_empty() {
        return false;
    }

    let path = Path::new(file_name);
    path.components().count() == 1
        && path.file_name().and_then(|name| name.to_str()) == Some(file_name)
        && file_name.ends_with(".json")
}
