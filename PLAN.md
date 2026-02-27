# Task: Implement Ferrotick Phase 11 - Strategy Optimization (Core Features)

## Objective
Create the `ferrotick-optimization` crate with grid search optimization and walk-forward validation for strategy parameter tuning.

## Requirements

1. Create new `ferrotick-optimization` crate with proper structure
2. Implement grid search optimization that exhaustively tests parameter combinations
3. Implement walk-forward validation for out-of-sample testing
4. Add proper error handling with thiserror
5. Add result storage for optimization results
6. Add basic tests for the optimization crate
7. Update workspace Cargo.toml to include the new crate
8. All tests must pass and cargo check must succeed

## Step-by-Step Implementation

### Step 1: Create the ferrotick-optimization crate
**File:** `crates/ferrotick-optimization/Cargo.toml`
**Action:** Create new crate directory and Cargo.toml
**What to do:**
Run `cargo new crates/ferrotick-optimization --lib` then update Cargo.toml with dependencies

**Code:**
```toml
[package]
name = "ferrotick-optimization"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-backtest = { path = "../ferrotick-backtest" }
ferrotick-strategies = { path = "../ferrotick-strategies" }
rand = "0.8"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"

[dev-dependencies]
tempfile = "3.17"
```

### Step 2: Update workspace Cargo.toml
**File:** `Cargo.toml` (workspace root)
**Action:** Modify
**Location:** In the `members` array
**What to do:** Add `"crates/ferrotick-optimization"` to the members list

**Code:**
```toml
[workspace]
members = [
  "crates/ferrotick-agent",
  "crates/ferrotick-backtest",
  "crates/ferrotick-cli",
  "crates/ferrotick-core",
  "crates/ferrotick-ml",
  "crates/ferrotick-optimization",
  "crates/ferrotick-strategies",
  "crates/ferrotick-warehouse",
]
```

### Step 3: Create error types
**File:** `crates/ferrotick-optimization/src/error.rs`
**Action:** Create new file
**What to do:** Define OptimizationError enum with thiserror

**Code:**
```rust
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
```

### Step 4: Create grid search module
**File:** `crates/ferrotick-optimization/src/grid_search.rs`
**Action:** Create new file
**What to do:** Implement grid search optimization

**Code:**
```rust
//! Grid search optimization for strategy parameters.
//!
//! Performs exhaustive search over parameter space to find optimal configuration.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ferrotick_core::Bar;
use ferrotick_backtest::{BacktestConfig, BacktestEngine, BacktestReport, BarEvent, Strategy, Symbol};

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
    pub fn add_param_range(&mut self, name: impl Into<String>, start: f64, end: f64, steps: usize) -> &mut Self {
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
    /// * `bars` - Market data to use for backtesting
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

        for params in combinations {
            // Convert bars to BarEvents (single symbol assumption for optimization)
            let bar_events: Vec<BarEvent> = bars
                .iter()
                .enumerate()
                .map(|(i, bar)| {
                    BarEvent::new(
                        Symbol::new(&bar.symbol),
                        ferrotick_core::Bar {
                            symbol: bar.symbol.clone(),
                            ts: bar.ts.clone(),
                            open: bar.open,
                            high: bar.high,
                            low: bar.low,
                            close: bar.close,
                            volume: bar.volume,
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

        let (best_params, best_metrics) = best_result
            .map(|r| (r.params, r.metrics))
            .unwrap_or_else(|| (HashMap::new(), create_empty_report(&config)));

        OptimizationReport {
            best_params,
            best_metrics,
            all_results,
            combinations_tested: all_results.len(),
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
```

### Step 5: Create walk-forward validation module
**File:** `crates/ferrotick-optimization/src/walk_forward.rs`
**Action:** Create new file
**What to do:** Implement walk-forward validation

**Code:**
```rust
//! Walk-forward validation for strategy optimization.
//!
//! Prevents overfitting by testing parameters on out-of-sample data.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ferrotick_core::Bar;
use ferrotick_backtest::{BacktestConfig, BacktestEngine, BacktestReport, BarEvent, Strategy, Symbol};
use crate::grid_search::GridSearchOptimizer;

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
        let step_len = self.step_pct.map(|s| (total_bars as f64 * s) as usize).unwrap_or(out_sample_len);

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
                .run_backtest(&strategy_factory, &opt_report.best_params, out_sample_bars, &config)
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
        let bar_events: Vec<BarEvent> = bars
            .iter()
            .map(|bar| {
                BarEvent::new(
                    Symbol::new(&bar.symbol),
                    ferrotick_core::Bar {
                        symbol: bar.symbol.clone(),
                        ts: bar.ts.clone(),
                        open: bar.open,
                        high: bar.high,
                        low: bar.low,
                        close: bar.close,
                        volume: bar.volume,
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
```

### Step 6: Create storage module
**File:** `crates/ferrotick-optimization/src/storage.rs`
**Action:** Create new file
**What to do:** Implement result storage

**Code:**
```rust
//! Storage for optimization results.
//!
//! Persists optimization results to disk for later analysis.

use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use crate::grid_search::OptimizationReport;
use crate::walk_forward::WalkForwardSummary;
use crate::{OptimizationError, OptimizationResult};

/// Stored optimization run metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRun {
    /// Unique identifier for this run.
    pub id: String,
    /// Timestamp of the run.
    pub timestamp: String,
    /// Strategy name.
    pub strategy_name: String,
    /// Grid search report.
    pub grid_search: Option<OptimizationReport>,
    /// Walk-forward summary.
    pub walk_forward: Option<WalkForwardSummary>,
}

/// Storage for optimization results.
#[derive(Debug, Clone)]
pub struct OptimizationStorage {
    /// Base directory for storing results.
    base_dir: String,
}

impl OptimizationStorage {
    /// Create a new storage instance.
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Save an optimization run.
    pub fn save(&self, run: &OptimizationRun) -> OptimizationResult<()> {
        let dir = Path::new(&self.base_dir);
        fs::create_dir_all(dir)?;

        let filename = format!("{}.json", run.id);
        let path = dir.join(&filename);
        let content = serde_json::to_string_pretty(run)?;

        fs::write(path, content)?;
        Ok(())
    }

    /// Load an optimization run by ID.
    pub fn load(&self, id: &str) -> OptimizationResult<OptimizationRun> {
        let path = Path::new(&self.base_dir).join(format!("{}.json", id));
        let content = fs::read_to_string(path)?;
        let run = serde_json::from_str(&content)?;
        Ok(run)
    }

    /// List all stored run IDs.
    pub fn list(&self) -> OptimizationResult<Vec<String>> {
        let dir = Path::new(&self.base_dir);
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut ids = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    ids.push(stem.to_string_lossy().to_string());
                }
            }
        }

        ids.sort();
        Ok(ids)
    }

    /// Delete a stored run by ID.
    pub fn delete(&self, id: &str) -> OptimizationResult<()> {
        let path = Path::new(&self.base_dir).join(format!("{}.json", id));
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run = OptimizationRun {
            id: "test-run-1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "sma_crossover".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run).unwrap();
        let loaded = storage.load("test-run-1").unwrap();
        assert_eq!(loaded.id, "test-run-1");
        assert_eq!(loaded.strategy_name, "sma_crossover");
    }

    #[test]
    fn test_list_runs() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run1 = OptimizationRun {
            id: "run-1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        let run2 = OptimizationRun {
            id: "run-2".to_string(),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run1).unwrap();
        storage.save(&run2).unwrap();

        let ids = storage.list().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"run-1".to_string()));
        assert!(ids.contains(&"run-2".to_string()));
    }

    #[test]
    fn test_delete_run() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

        let run = OptimizationRun {
            id: "to-delete".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            strategy_name: "test".to_string(),
            grid_search: None,
            walk_forward: None,
        };

        storage.save(&run).unwrap();
        storage.delete("to-delete").unwrap();
        assert!(storage.load("to-delete").is_err());
    }
}
```

### Step 7: Create lib.rs with exports
**File:** `crates/ferrotick-optimization/src/lib.rs`
**Action:** Create new file
**What to do:** Export all modules

**Code:**
```rust
//! # Ferrotick Optimization
//!
//! Strategy parameter optimization with grid search and walk-forward validation.
//!
//! ## Overview
//!
//! This crate provides tools for optimizing trading strategy parameters:
//!
//! - **Grid Search**: Exhaustive parameter space exploration
//! - **Walk-Forward Validation**: Out-of-sample testing to prevent overfitting
//! - **Result Storage**: Persist optimization results for analysis
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ferrotick_optimization::{GridSearchOptimizer, WalkForwardValidator};
//! use ferrotick_backtest::BacktestConfig;
//!
//! // Create a grid search optimizer
//! let mut optimizer = GridSearchOptimizer::new();
//! optimizer
//!     .add_param("short_period", vec![5.0, 10.0, 20.0])
//!     .add_param("long_period", vec![20.0, 50.0, 100.0]);
//!
//! // Run optimization
//! let report = optimizer.optimize(strategy_factory, &bars, config).await;
//! println!("Best Sharpe: {}", report.best_metrics.sharpe_ratio);
//!
//! // Validate with walk-forward
//! let validator = WalkForwardValidator::new(0.7, 0.2);
//! let summary = validator.validate(strategy_factory, &bars, &optimizer, config).await;
//!
//! if summary.overfitting_ratio > 1.5 {
//!     println!("Warning: Strategy may be overfitting!");
//! }
//! ```

pub mod error;
pub mod grid_search;
pub mod storage;
pub mod walk_forward;

pub use error::{OptimizationError, OptimizationResult};
pub use grid_search::{GridSearchOptimizer, OptimizationReport, ParamRange, ParamResult};
pub use storage::{OptimizationRun, OptimizationStorage};
pub use walk_forward::{WalkForwardSummary, WalkForwardValidator, WalkForwardWindow};

// Re-export commonly used types from dependencies
pub use ferrotick_backtest::{BacktestConfig, BacktestReport};
pub use ferrotick_core::Bar;
```

### Step 8: Create integration tests
**File:** `crates/ferrotick-optimization/tests/optimization_test.rs`
**Action:** Create new file
**What to do:** Add integration tests

**Code:**
```rust
//! Integration tests for ferrotick-optimization.

use ferrotick_optimization::*;
use ferrotick_core::Bar;
use ferrotick_backtest::{BacktestConfig, BacktestReport, BarEvent, Portfolio, SignalEvent, Strategy, Symbol};
use std::collections::HashMap;

/// Create test bars for optimization testing.
fn create_test_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| Bar {
            symbol: "TEST".to_string(),
            ts: format!("2024-01-{:02}T12:00:00Z", (i % 28) + 1),
            open: 100.0 + (i as f64 * 0.1).sin() * 5.0,
            high: 101.0 + (i as f64 * 0.1).sin() * 5.0,
            low: 99.0 + (i as f64 * 0.1).sin() * 5.0,
            close: 100.5 + (i as f64 * 0.1).sin() * 5.0,
            volume: 1000,
        })
        .collect()
}

/// Dummy strategy for testing that accepts parameters.
#[derive(Clone)]
struct TestStrategy {
    short_period: f64,
    long_period: f64,
}

impl Strategy for TestStrategy {
    fn on_bar(&mut self, _bar: &BarEvent, _portfolio: &Portfolio) -> Option<SignalEvent> {
        // Simple placeholder - doesn't generate signals
        None
    }

    fn create_order(
        &self,
        _signal: &SignalEvent,
        _portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<ferrotick_backtest::Order> {
        None
    }
}

fn test_strategy_factory(params: &HashMap<String, f64>) -> TestStrategy {
    TestStrategy {
        short_period: params.get("short_period").copied().unwrap_or(10.0),
        long_period: params.get("long_period").copied().unwrap_or(50.0),
    }
}

#[tokio::test]
async fn test_grid_search_generates_combinations() {
    let mut optimizer = GridSearchOptimizer::new();
    optimizer
        .add_param("short_period", vec![5.0, 10.0])
        .add_param("long_period", vec![20.0, 50.0]);

    assert_eq!(optimizer.total_combinations(), 4);
}

#[tokio::test]
async fn test_grid_search_runs_backtests() {
    let mut optimizer = GridSearchOptimizer::new();
    optimizer
        .add_param("short_period", vec![5.0, 10.0])
        .add_param("long_period", vec![20.0]);

    let bars = create_test_bars(50);
    let config = BacktestConfig::default();

    let report = optimizer.optimize(test_strategy_factory, &bars, config).await;

    // Should have tested 2 combinations (2 * 1)
    assert_eq!(report.combinations_tested, 2);
    assert_eq!(report.all_results.len(), 2);
    assert!(!report.best_params.is_empty());
}

#[tokio::test]
async fn test_walk_forward_splits_data() {
    let validator = WalkForwardValidator::new(0.7, 0.2);

    // 100 bars: 70 for training, 20 for testing, 10 leftover
    let bars = create_test_bars(100);

    let optimizer = GridSearchOptimizer::new();
    optimizer.add_param("short_period", vec![10.0]);

    let config = BacktestConfig::default();
    let summary = validator.validate(test_strategy_factory, &bars, &optimizer, config).await;

    // Should have at least 1 window
    assert!(summary.window_count >= 1);

    // Check window structure
    if let Some(window) = summary.windows.first() {
        assert_eq!(window.train_end - window.train_start, 70);
        assert_eq!(window.test_end - window.test_start, 20);
    }
}

#[tokio::test]
async fn test_optimization_storage() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let storage = OptimizationStorage::new(temp_dir.path().to_string_lossy().to_string());

    let run = OptimizationRun {
        id: "test-integration".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        strategy_name: "test_strategy".to_string(),
        grid_search: None,
        walk_forward: None,
    };

    storage.save(&run).unwrap();
    let loaded = storage.load("test-integration").unwrap();
    assert_eq!(loaded.id, "test-integration");
}
```

## Existing Patterns to Follow

- Error handling pattern (from `crates/ferrotick-backtest/src/error.rs`):
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    // ...
}
```

- Module exports pattern (from `crates/ferrotick-backtest/src/lib.rs`):
```rust
pub mod engine;
pub mod error;
pub use error::BacktestError;
pub use engine::{BacktestEngine, Strategy};
```

## Edge Cases and Error Handling

1. **Empty parameter ranges** → Return single empty combination
2. **No bars provided** → Return empty report with default metrics
3. **Backtest fails for a combination** → Skip and continue, log error
4. **Walk-forward with insufficient data** → Return empty summary
5. **Storage directory doesn't exist** → Create it

## Dependencies and Imports

- Add `ferrotick-optimization` to workspace members in root `Cargo.toml`
- All crate dependencies are already in workspace

## Acceptance Criteria

- [ ] `cargo check --workspace` passes with 0 errors
- [ ] `cargo test -p ferrotick-optimization` passes with 0 failures
- [ ] Grid search generates correct number of parameter combinations
- [ ] Walk-forward validation splits data correctly
- [ ] Storage can save and load optimization results

## Out of Scope

- Genetic algorithm (P1)
- Bayesian optimization (P1)
- CLI commands (P1)
- Multi-objective optimization (P2)
- Overfitting detection beyond basic ratio (P1)
