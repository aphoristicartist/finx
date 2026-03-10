//! Walk-forward validation for strategy optimization.
//!
//! Prevents overfitting by testing parameters on out-of-sample data.

use crate::grid_search::GridSearchOptimizer;
use ferrotick_backtest::{BacktestConfig, BacktestEngine, BacktestReport, BarEvent, Strategy};
use ferrotick_core::{Bar, Symbol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a single walk-forward window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardWindow {
    /// Start index of training data.
    pub train_start: usize,
    /// End index of training data.
    pub train_end: usize,
    /// Start index of test data.
    pub test_start: usize,
    /// End index of test data.
    pub test_end: usize,
    /// Best parameters found in training.
    pub best_params: HashMap<String, f64>,
    /// Performance on training data.
    pub train_metrics: BacktestReport,
    /// Performance on out-of-sample test data.
    pub test_metrics: Option<BacktestReport>,
}

/// Summary of walk-forward validation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardSummary {
    /// Individual window results.
    pub windows: Vec<WalkForwardWindow>,
    /// Average Sharpe ratio on training data.
    pub avg_train_sharpe: f64,
    /// Average Sharpe ratio on test data.
    pub avg_test_sharpe: f64,
    /// Overfitting ratio (train_sharpe / test_sharpe).
    /// Values > 1.5 suggest overfitting.
    pub overfitting_ratio: f64,
    /// Number of windows tested.
    pub window_count: usize,
}

/// Walk-forward validator for out-of-sample testing.
#[derive(Debug, Clone)]
pub struct WalkForwardValidator {
    /// Percentage of data for in-sample training.
    in_sample_pct: f64,
    /// Percentage of data for out-of-sample testing.
    out_sample_pct: f64,
    /// Step size for rolling windows (None = use out_sample_pct).
    step_pct: Option<f64>,
}

impl WalkForwardValidator {
    /// Create a new walk-forward validator.
    ///
    /// # Arguments
    /// * `in_sample_pct` - Fraction of data for training (e.g., 0.7 for 70%)
    /// * `out_sample_pct` - Fraction of data for testing (e.g., 0.2 for 20%)
    ///
    /// # Panics
    /// Panics if in_sample_pct + out_sample_pct > 1.0.
    pub fn new(in_sample_pct: f64, out_sample_pct: f64) -> Self {
        assert!(
            in_sample_pct + out_sample_pct <= 1.0,
            "In-sample + out-sample must not exceed 100%"
        );
        Self {
            in_sample_pct,
            out_sample_pct,
            step_pct: None,
        }
    }

    /// Set custom step size for rolling windows.
    pub fn with_step(mut self, step_pct: f64) -> Self {
        self.step_pct = Some(step_pct);
        self
    }

    /// Run walk-forward validation.
    ///
    /// # Arguments
    /// * `strategy_factory` - Function to create strategy from parameters
    /// * `bars` - Full market data
    /// * `optimizer` - Grid search optimizer for finding best parameters
    /// * `config` - Backtest configuration
    pub async fn validate<S>(
        &self,
        strategy_factory: impl Fn(&HashMap<String, f64>) -> S + Clone,
        bars: &[Bar],
        optimizer: &GridSearchOptimizer,
        config: BacktestConfig,
    ) -> WalkForwardSummary
    where
        S: Strategy + Send + Clone,
    {
        let mut windows = Vec::new();
        let total_bars = bars.len();
        let in_sample_len = (total_bars as f64 * self.in_sample_pct) as usize;
        let out_sample_len = (total_bars as f64 * self.out_sample_pct) as usize;
        let step_len = self
            .step_pct
            .map(|s| (total_bars as f64 * s) as usize)
            .unwrap_or(out_sample_len);

        if in_sample_len == 0 || out_sample_len == 0 {
            return WalkForwardSummary {
                windows: vec![],
                avg_train_sharpe: 0.0,
                avg_test_sharpe: 0.0,
                overfitting_ratio: f64::NAN,
                window_count: 0,
            };
        }

        let mut start = 0;
        while start + in_sample_len + out_sample_len <= total_bars {
            let in_sample_end = start + in_sample_len;
            let out_sample_end = in_sample_end + out_sample_len;

            // Optimize on in-sample data
            let in_sample_bars = &bars[start..in_sample_end];
            let opt_report = optimizer
                .optimize(strategy_factory.clone(), in_sample_bars, config.clone())
                .await;

            // Test on out-of-sample data
            let out_sample_bars = &bars[in_sample_end..out_sample_end];
            let test_metrics = self
                .run_backtest(
                    &strategy_factory,
                    &opt_report.best_params,
                    out_sample_bars,
                    &config,
                )
                .await;

            windows.push(WalkForwardWindow {
                train_start: start,
                train_end: in_sample_end,
                test_start: in_sample_end,
                test_end: out_sample_end,
                best_params: opt_report.best_params,
                train_metrics: opt_report.best_metrics,
                test_metrics,
            });

            // Move window forward
            start += step_len;
        }

        // Calculate summary statistics
        let (avg_train_sharpe, avg_test_sharpe) = calculate_averages(&windows);
        let overfitting_ratio = if avg_test_sharpe.abs() > 0.001 {
            avg_train_sharpe / avg_test_sharpe
        } else {
            f64::NAN
        };

        WalkForwardSummary {
            window_count: windows.len(),
            avg_train_sharpe,
            avg_test_sharpe,
            overfitting_ratio,
            windows,
        }
    }

    /// Run a single backtest with given parameters.
    async fn run_backtest<S>(
        &self,
        strategy_factory: &impl Fn(&HashMap<String, f64>) -> S,
        params: &HashMap<String, f64>,
        bars: &[Bar],
        config: &BacktestConfig,
    ) -> Option<BacktestReport>
    where
        S: Strategy + Send,
    {
        // Use a default symbol for bars (optimization typically uses single symbol)
        let default_symbol = Symbol::parse("OPT").unwrap();

        let bar_events: Vec<BarEvent> = bars
            .iter()
            .map(|bar| {
                BarEvent::new(
                    default_symbol.clone(),
                    ferrotick_core::Bar {
                        ts: bar.ts,
                        open: bar.open,
                        high: bar.high,
                        low: bar.low,
                        close: bar.close,
                        volume: bar.volume,
                        vwap: bar.vwap,
                    },
                )
            })
            .collect();

        let mut strategy = strategy_factory(params);
        let mut engine = BacktestEngine::new(config.clone());

        engine.run(&mut strategy, &bar_events).await.ok()
    }
}

/// Calculate average Sharpe ratios from windows.
fn calculate_averages(windows: &[WalkForwardWindow]) -> (f64, f64) {
    if windows.is_empty() {
        return (0.0, 0.0);
    }

    let train_sum: f64 = windows.iter().map(|w| w.train_metrics.sharpe_ratio).sum();
    let test_sum: f64 = windows
        .iter()
        .filter_map(|w| w.test_metrics.as_ref())
        .map(|m| m.sharpe_ratio)
        .sum();

    let test_count = windows.iter().filter(|w| w.test_metrics.is_some()).count();

    (
        train_sum / windows.len() as f64,
        if test_count > 0 {
            test_sum / test_count as f64
        } else {
            0.0
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = WalkForwardValidator::new(0.7, 0.2);
        assert_eq!(validator.in_sample_pct, 0.7);
        assert_eq!(validator.out_sample_pct, 0.2);
    }

    #[test]
    #[should_panic]
    fn test_validator_invalid_percentages() {
        WalkForwardValidator::new(0.8, 0.3); // Should panic
    }

    #[test]
    fn test_custom_step() {
        let validator = WalkForwardValidator::new(0.7, 0.2).with_step(0.1);
        assert_eq!(validator.step_pct, Some(0.1));
    }
}
