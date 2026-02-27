pub mod drawdown;
pub mod returns;
pub mod risk;

use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};

pub use drawdown::{DrawdownPoint, DrawdownSummary};
pub use risk::PerformanceMetrics;

/// Snapshot of portfolio equity at a point in time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EquityPoint {
    pub ts: UtcDateTime,
    pub equity: f64,
    pub cash: f64,
    pub position_value: f64,
}

/// Flat metrics report for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub total_return: f64,
    pub annualized_return: f64,
    pub volatility: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub var_95: f64,
    pub cvar_95: f64,
}

impl MetricsReport {
    pub fn from_equity_curve(
        equity_curve: &[EquityPoint],
        risk_free_rate: f64,
        trading_days_per_year: f64,
    ) -> Self {
        let metrics = PerformanceMetrics::from_equity_curve(equity_curve, trading_days_per_year);

        Self {
            total_return: metrics.total_return(),
            annualized_return: metrics.annualized_return(),
            volatility: metrics.volatility(),
            sharpe_ratio: metrics.sharpe_ratio(risk_free_rate),
            sortino_ratio: metrics.sortino_ratio(risk_free_rate),
            max_drawdown: metrics.max_drawdown(),
            var_95: metrics.var(0.95),
            cvar_95: metrics.cvar(0.95),
        }
    }
}
