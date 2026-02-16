mod cli;
mod commands;
mod error;
mod output;

use clap::Parser;

use crate::cli::Cli;
use crate::error::CliError;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(error.exit_code());
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    let envelope = commands::run(&cli)?;
    output::render(&envelope, cli.format, cli.pretty)?;

    if cli.strict && (!envelope.meta.warnings.is_empty() || !envelope.errors.is_empty()) {
        return Err(CliError::StrictModeViolation {
            warning_count: envelope.meta.warnings.len(),
            error_count: envelope.errors.len(),
        });
    }

    Ok(())
}
