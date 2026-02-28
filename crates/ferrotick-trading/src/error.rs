use thiserror::Error;

#[derive(Debug, Error)]
pub enum TradingError {
    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Position not found: {0}")]
    PositionNotFound(String),

    #[error("Broker error: {0}")]
    Broker(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
