mod cli;
mod commands;
mod error;
mod metadata;
mod output;

use clap::Parser;
use std::process::ExitCode;

use crate::cli::Cli;
use crate::error::CliError;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run() -> Result<ExitCode, CliError> {
    let cli = Cli::parse();

    let envelope = commands::run(&cli).await?;
    if cli.stream {
        output::render_stream(&envelope, cli.explain)?;
    } else {
        output::render(&envelope, cli.format, cli.pretty)?;
    }

    if cli.strict && (!envelope.meta.warnings.is_empty() || !envelope.errors.is_empty()) {
        return Err(CliError::StrictModeViolation {
            warning_count: envelope.meta.warnings.len(),
            error_count: envelope.errors.len(),
        });
    }

    if !envelope.errors.is_empty() {
        return Ok(ExitCode::from(3));
    }

    Ok(ExitCode::SUCCESS)
}
