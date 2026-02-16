use thiserror::Error;

/// CLI-level error categories mapped to exit codes.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Validation(#[from] finx_core::ValidationError),

    #[error("command error: {0}")]
    Command(String),

    #[error("strict mode failed: warnings={warning_count}, errors={error_count}")]
    StrictModeViolation {
        warning_count: usize,
        error_count: usize,
    },

    #[error(transparent)]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl CliError {
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Validation(_) => 2,
            Self::StrictModeViolation { .. } => 5,
            Self::Command(_) | Self::Serialization(_) | Self::Io(_) => 10,
        }
    }
}
