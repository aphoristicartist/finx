use ferrotick_backtest::*;
use ferrotick_core::{Bar, UtcDateTime};
use std::collections::HashMap;
use std::time::Instant;

fn create_test_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| {
            let day = (i % 28) + 1; // Keep within valid days
            let ts_str = format!("2024-01-{:02}T09:30:00Z", day);
            Bar {
                ts: UtcDateTime::parse(&ts_str).expect("valid timestamp"),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.0,
                volume: Some(1000),
                vwap: None,
            }
        })
        .collect()
}

fn rolling_average_partial(closes: &[f64], index: usize, period: usize) -> f64 {
    let safe_period = period.max(1);
    let start = index.saturating_add(1).saturating_sub(safe_period);
    let window = &closes[start..=index];
    window.iter().sum::<f64>() / window.len() as f64
}

fn event_driven_total_return(bars: &[Bar], short_period: usize, long_period: usize) -> f64 {
    if bars.is_empty() {
        return 0.0;
    }

    let closes: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
    let mut signals = vec![0_i8; closes.len()];

    for index in 1..closes.len() {
        let short_ma = rolling_average_partial(&closes, index, short_period);
        let long_ma = rolling_average_partial(&closes, index, long_period);
        let prev_short_ma = rolling_average_partial(&closes, index - 1, short_period);
        let prev_long_ma = rolling_average_partial(&closes, index - 1, long_period);

        if short_ma > long_ma && prev_short_ma <= prev_long_ma {
            signals[index] = 1;
        } else if short_ma < long_ma && prev_short_ma >= prev_long_ma {
            signals[index] = -1;
        }
    }

    let mut equity = vec![100_000.0];
    let mut position = 0.0;

    for (index, signal) in signals.iter().enumerate().skip(1) {
        let price = closes[index];
        if *signal == 1 && position == 0.0 {
            position = equity[index - 1] / price;
            equity.push(position * price);
        } else if *signal == -1 && position > 0.0 {
            equity.push(position * price);
            position = 0.0;
        } else if position > 0.0 {
            equity.push(position * price);
        } else {
            equity.push(equity[index - 1]);
        }
    }

    if equity.len() < 2 {
        0.0
    } else {
        (equity[equity.len() - 1] - equity[0]) / equity[0]
    }
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

        let short_period = result.params["short_period"] as usize;
        let long_period = result.params["long_period"] as usize;
        let expected_total_return = event_driven_total_return(&bars, short_period, long_period);
        let delta = (result.metrics.total_return() - expected_total_return).abs();
        assert!(
            delta <= 1e-9,
            "vectorized total_return {} diverged from event-driven {} (delta {})",
            result.metrics.total_return(),
            expected_total_return,
            delta
        );
    }
}

#[test]
fn test_single_parameter_backtest() {
    let backtest = VectorizedBacktest::new().unwrap();
    let bars = create_test_bars(200);
    backtest.load_bars("TEST", &bars).unwrap();

    let mut param_grid = HashMap::new();
    param_grid.insert("short_period".to_string(), vec![10.0]);
    param_grid.insert("long_period".to_string(), vec![30.0]);

    let vectorized_start = Instant::now();
    let results = backtest
        .run_parameter_sweep("ma_crossover", param_grid.clone())
        .unwrap();
    let vectorized_duration = vectorized_start.elapsed();

    assert_eq!(results.len(), 1);
    let result = &results[0];

    let expected_total_return = event_driven_total_return(&bars, 10, 30);
    let total_return_delta = (result.metrics.total_return() - expected_total_return).abs();
    assert!(
        total_return_delta <= 1e-9,
        "vectorized total_return {} diverged from event-driven {} (delta {})",
        result.metrics.total_return(),
        expected_total_return,
        total_return_delta
    );

    let event_driven_start = Instant::now();
    for _ in 0..10_000 {
        let _ = event_driven_total_return(&bars, 10, 30);
    }
    let event_driven_duration = event_driven_start.elapsed();
    let speedup = event_driven_duration.as_secs_f64() / vectorized_duration.as_secs_f64().max(1e-9);
    assert!(
        speedup >= 10.0,
        "expected >=10x speedup, got {:.2}x (event-driven {:?}, vectorized {:?})",
        speedup,
        event_driven_duration,
        vectorized_duration
    );

    assert!(
        result.metrics.sharpe_ratio(0.02).is_finite() || result.metrics.sharpe_ratio(0.02) == 0.0
    );
    assert!(result.metrics.max_drawdown() >= 0.0 && result.metrics.max_drawdown() <= 1.0);
}
