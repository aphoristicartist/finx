use thiserror::Error;

/// Errors that can occur during optimization.
#[derive(Debug, Error)]
pub enum OptimizationError {
    /// Invalid parameter range specified.
    #[error("Invalid parameter range: {0}")]
    InvalidRange(String),

    /// No valid parameter combinations available.
    #[error("No valid parameter combinations")]
    NoCombinations,

    /// Backtest execution failed.
    #[error("Backtest failed: {0}")]
    Backtest(#[from] ferrotick_backtest::BacktestError),

    /// I/O error during storage operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for optimization operations.
pub type OptimizationResult<T> = Result<T, OptimizationError>;
