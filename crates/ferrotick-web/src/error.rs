use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebError {
    #[error("Backtest error: {0}")]
    Backtest(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for WebError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            WebError::Backtest(msg) => (StatusCode::BAD_REQUEST, msg),
            WebError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            WebError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (
            status,
            Json(json!({
                "error": message,
                "status": "error"
            })),
        )
            .into_response()
    }
}
