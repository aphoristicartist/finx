use crate::error::WebError;
use crate::models::api::{BacktestRequest, BacktestResponse};
use axum::{http::StatusCode, Json};

pub async fn run_backtest(
    Json(req): Json<BacktestRequest>,
) -> Result<(StatusCode, Json<BacktestResponse>), WebError> {
    // Stub implementation - would integrate with ferrotick-backtest
    let response = BacktestResponse {
        status: "success".to_string(),
        message: format!("Backtest for strategy '{}' completed", req.strategy_name),
        metrics: Default::default(),
    };

    Ok((StatusCode::OK, Json(response)))
}
