use thiserror::Error;

/// CLI-level error categories mapped to exit codes.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Validation(#[from] ferrotick_core::ValidationError),

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

    #[error(transparent)]
    Stream(#[from] ferrotick_agent::stream::StreamError),
}

impl CliError {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Validation(_) => 2,
            Self::Command(_) => 2,
            Self::StrictModeViolation { .. } => 5,
            Self::Serialization(_) => 4,
            Self::Io(_) => 10,
            Self::Stream(_) => 6,
        }
    }
}
