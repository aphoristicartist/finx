use thiserror::Error;

#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("invalid order: {0}")]
    InvalidOrder(String),

    #[error("no market data provided")]
    NoMarketData,

    #[error("no bar available for symbol '{0}'")]
    MissingBarForSymbol(String),

    #[error("limit order is missing limit_price")]
    MissingLimitPrice,

    #[error("stop order is missing stop_price")]
    MissingStopPrice,

    #[error("insufficient cash: required={required:.4}, available={available:.4}")]
    InsufficientCash { required: f64, available: f64 },

    #[error(
        "insufficient position for symbol '{symbol}': requested={requested:.4}, available={available:.4}"
    )]
    InsufficientPosition {
        symbol: String,
        requested: f64,
        available: f64,
    },

    #[error("event bus is closed")]
    EventBusClosed,

    #[error("engine error: {0}")]
    EngineError(String),

    #[error("unsupported strategy: {0}")]
    UnsupportedStrategy(String),

    #[error(transparent)]
    Validation(#[from] ferrotick_core::ValidationError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
