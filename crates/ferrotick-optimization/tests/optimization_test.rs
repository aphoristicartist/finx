//! Integration tests for ferrotick-optimization.

use ferrotick_optimization::*;
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_backtest::{BacktestConfig, BarEvent, Portfolio, SignalEvent, Strategy};
use std::collections::HashMap;

/// Create test bars for optimization testing.
fn create_test_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| {
            Bar::new(
                UtcDateTime::parse(&format!("2024-01-{:02}T12:00:00Z", (i % 28) + 1)).unwrap(),
                100.0 + (i as f64 * 0.1).sin() * 5.0,
                101.0 + (i as f64 * 0.1).sin() * 5.0,
                99.0 + (i as f64 * 0.1).sin() * 5.0,
                100.5 + (i as f64 * 0.1).sin() * 5.0,
                Some(1000),
                None,
            ).unwrap()
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

    let mut optimizer = GridSearchOptimizer::new();
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
