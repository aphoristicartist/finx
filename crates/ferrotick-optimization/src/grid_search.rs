//! Grid search optimization for strategy parameters.
//!
//! Performs exhaustive search over parameter space to find optimal configuration.

use ferrotick_backtest::{BacktestConfig, BacktestEngine, BacktestReport, BarEvent, Strategy};
use ferrotick_core::{Bar, Symbol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A range of values for a single parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamRange {
    /// Parameter name.
    pub name: String,
    /// Possible values for this parameter.
    pub values: Vec<f64>,
}

impl ParamRange {
    /// Create a new parameter range.
    pub fn new(name: impl Into<String>, values: Vec<f64>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }
}

/// Result of a single parameter combination test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamResult {
    /// Parameter values tested.
    pub params: HashMap<String, f64>,
    /// Performance metrics from the backtest.
    pub metrics: BacktestReport,
}

/// Result of grid search optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    /// Best parameter combination found.
    pub best_params: HashMap<String, f64>,
    /// Performance metrics for best parameters.
    pub best_metrics: BacktestReport,
    /// All parameter combinations tested.
    pub all_results: Vec<ParamResult>,
    /// Total number of combinations tested.
    pub combinations_tested: usize,
}

/// Grid search optimizer for strategy parameters.
///
/// Performs exhaustive search over all parameter combinations.
#[derive(Debug, Clone, Default)]
pub struct GridSearchOptimizer {
    /// Parameter ranges to search.
    param_ranges: Vec<ParamRange>,
}

impl GridSearchOptimizer {
    /// Create a new grid search optimizer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a parameter range to search.
    pub fn add_param(&mut self, name: impl Into<String>, values: Vec<f64>) -> &mut Self {
        self.param_ranges.push(ParamRange::new(name, values));
        self
    }

    /// Add a parameter range with linear spacing.
    pub fn add_param_range(
        &mut self,
        name: impl Into<String>,
        start: f64,
        end: f64,
        steps: usize,
    ) -> &mut Self {
        let values: Vec<f64> = (0..steps)
            .map(|i| start + (end - start) * (i as f64) / ((steps - 1).max(1) as f64))
            .collect();
        self.add_param(name, values)
    }

    /// Get the total number of parameter combinations.
    pub fn total_combinations(&self) -> usize {
        if self.param_ranges.is_empty() {
            return 0;
        }
        self.param_ranges.iter().map(|r| r.values.len()).product()
    }

    /// Run grid search optimization.
    ///
    /// # Arguments
    /// * `strategy_factory` - Function that creates a strategy from parameter values
    /// * `bars` - Market data to use for backtesting (without symbol field)
    /// * `config` - Backtest configuration
    ///
    /// # Returns
    /// Optimization report with best parameters and all results.
    pub async fn optimize<S>(
        &self,
        strategy_factory: impl Fn(&HashMap<String, f64>) -> S,
        bars: &[Bar],
        config: BacktestConfig,
    ) -> OptimizationReport
    where
        S: Strategy + Send,
    {
        let mut best_result: Option<ParamResult> = None;
        let mut all_results = Vec::new();
        let combinations = self.generate_combinations();

        // Use a default symbol for bars (optimization typically uses single symbol)
        let default_symbol = Symbol::parse("OPT").unwrap();

        for params in combinations {
            // Convert bars to BarEvents (single symbol assumption for optimization)
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

            let mut strategy = strategy_factory(&params);
            let mut engine = BacktestEngine::new(config.clone());

            match engine.run(&mut strategy, &bar_events).await {
                Ok(report) => {
                    let result = ParamResult {
                        params: params.clone(),
                        metrics: report.clone(),
                    };

                    if best_result.is_none()
                        || report.sharpe_ratio > best_result.as_ref().unwrap().metrics.sharpe_ratio
                    {
                        best_result = Some(result.clone());
                    }

                    all_results.push(result);
                }
                Err(e) => {
                    eprintln!("Backtest failed for params {:?}: {}", params, e);
                }
            }
        }

        let combinations_tested = all_results.len();
        let (best_params, best_metrics) = best_result
            .map(|r| (r.params, r.metrics))
            .unwrap_or_else(|| (HashMap::new(), create_empty_report(&config)));

        OptimizationReport {
            best_params,
            best_metrics,
            all_results,
            combinations_tested,
        }
    }

    /// Generate all parameter combinations.
    fn generate_combinations(&self) -> Vec<HashMap<String, f64>> {
        if self.param_ranges.is_empty() {
            return vec![HashMap::new()];
        }

        let mut combinations = vec![HashMap::new()];

        for param_range in &self.param_ranges {
            let mut new_combinations = Vec::new();

            for combo in &combinations {
                for value in &param_range.values {
                    let mut new_combo = combo.clone();
                    new_combo.insert(param_range.name.clone(), *value);
                    new_combinations.push(new_combo);
                }
            }

            combinations = new_combinations;
        }

        combinations
    }
}

/// Create an empty backtest report for error cases.
fn create_empty_report(config: &BacktestConfig) -> BacktestReport {
    BacktestReport {
        initial_capital: config.initial_capital,
        final_equity: config.initial_capital,
        total_return: 0.0,
        annualized_return: 0.0,
        volatility: 0.0,
        sharpe_ratio: f64::NEG_INFINITY,
        sortino_ratio: f64::NEG_INFINITY,
        max_drawdown: 0.0,
        var_95: 0.0,
        cvar_95: 0.0,
        trades: 0,
        win_rate: 0.0,
        equity_curve: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_combinations() {
        let mut optimizer = GridSearchOptimizer::new();
        optimizer.add_param("a", vec![1.0, 2.0]);
        optimizer.add_param("b", vec![3.0, 4.0]);

        let combinations = optimizer.generate_combinations();
        assert_eq!(combinations.len(), 4);
        assert_eq!(optimizer.total_combinations(), 4);
    }

    #[test]
    fn test_param_range_linear() {
        let mut optimizer = GridSearchOptimizer::new();
        optimizer.add_param_range("period", 5.0, 20.0, 4);

        let values: Vec<f64> = optimizer.param_ranges[0].values.clone();
        assert_eq!(values.len(), 4);
        assert!((values[0] - 5.0).abs() < 0.01);
        assert!((values[3] - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_optimizer() {
        let optimizer = GridSearchOptimizer::new();
        assert_eq!(optimizer.total_combinations(), 0);

        let combinations = optimizer.generate_combinations();
        assert_eq!(combinations.len(), 1);
        assert!(combinations[0].is_empty());
    }
}
