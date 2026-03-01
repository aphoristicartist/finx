use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub strategy_name: String,
    pub symbol: String,
    pub start_date: String,
    pub end_date: String,
    pub initial_capital: f64,
}

#[derive(Debug, Serialize)]
pub struct BacktestResponse {
    pub status: String,
    pub message: String,
    pub metrics: BacktestMetrics,
}

#[derive(Debug, Serialize, Default)]
pub struct BacktestMetrics {
    pub total_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub total_trades: usize,
}

#[derive(Debug, Serialize)]
pub struct StrategyInfo {
    pub name: String,
    pub description: String,
    pub parameters: Vec<String>,
}
