use ferrotick_backtest::*;
use ferrotick_core::{Bar, UtcDateTime};
use std::collections::HashMap;

fn create_test_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| {
            let day = (i % 28) + 1; // Keep within valid days
            let ts_str = format!("2024-01-{:02}T09:30:00Z", day);
            Bar {
                ts: UtcDateTime::parse(&ts_str).expect("valid timestamp"),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64,
                volume: Some(1000),
                vwap: None,
            }
        })
        .collect()
}

#[test]
fn test_vectorized_backtest_creation() {
    let backtest = VectorizedBacktest::new().unwrap();
    // Verify it creates without error - access internal state via a query
    drop(backtest);
}

#[test]
fn test_load_bars() {
    let backtest = VectorizedBacktest::new().unwrap();
    let bars = create_test_bars(100);
    backtest.load_bars("TEST", &bars).unwrap();
}

#[test]
fn test_parameter_sweep() {
    let backtest = VectorizedBacktest::new().unwrap();
    let bars = create_test_bars(100);
    backtest.load_bars("TEST", &bars).unwrap();

    let mut param_grid = HashMap::new();
    param_grid.insert("short_period".to_string(), vec![5.0, 10.0]);
    param_grid.insert("long_period".to_string(), vec![20.0, 50.0]);

    let results = backtest
        .run_parameter_sweep("ma_crossover", param_grid)
        .unwrap();

    assert_eq!(results.len(), 4); // 2 x 2 parameter combinations

    // Verify each result has metrics
    for result in &results {
        assert!(result.params.contains_key("short_period"));
        assert!(result.params.contains_key("long_period"));
    }
}

#[test]
fn test_single_parameter_backtest() {
    let backtest = VectorizedBacktest::new().unwrap();
    let bars = create_test_bars(100);
    backtest.load_bars("TEST", &bars).unwrap();

    let mut param_grid = HashMap::new();
    param_grid.insert("short_period".to_string(), vec![10.0]);
    param_grid.insert("long_period".to_string(), vec![30.0]);

    let results = backtest
        .run_parameter_sweep("ma_crossover", param_grid)
        .unwrap();

    assert_eq!(results.len(), 1);
    let result = &results[0];
    // Access metrics using methods
    assert!(
        result.metrics.sharpe_ratio(0.02).is_finite() || result.metrics.sharpe_ratio(0.02) == 0.0
    );
    assert!(result.metrics.max_drawdown() >= 0.0 && result.metrics.max_drawdown() <= 1.0);
}
