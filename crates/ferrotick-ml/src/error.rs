use thiserror::Error;

#[derive(Debug, Error)]
pub enum MlError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("no data: {0}")]
    NoData(String),

    #[error("feature computation failed: {0}")]
    Compute(String),

    #[error("feature store error: {0}")]
    Store(String),

    #[error(transparent)]
    Validation(#[from] ferrotick_core::ValidationError),

    #[error(transparent)]
    Warehouse(#[from] ferrotick_warehouse::WarehouseError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Polars(#[from] polars::error::PolarsError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("model training failed: {0}")]
    Training(String),

    #[error("model prediction failed: {0}")]
    Prediction(String),

    #[error("ONNX error: {0}")]
    Onnx(String),
}
