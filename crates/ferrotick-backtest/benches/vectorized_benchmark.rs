use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ferrotick_backtest::*;
use ferrotick_core::{Bar, UtcDateTime};
use std::collections::HashMap;

fn create_test_bars(n: usize) -> Vec<Bar> {
    (0..n)
        .map(|i| {
            let day = (i % 28) + 1;
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

fn benchmark_parameter_sweep(c: &mut Criterion) {
    let backtest = VectorizedBacktest::new().unwrap();
    let bars = create_test_bars(1_000);
    backtest.load_bars("TEST", &bars).unwrap();

    let mut param_grid = HashMap::new();
    param_grid.insert(
        "short_period".to_string(),
        (1..=10).map(|x| x as f64).collect(),
    );
    param_grid.insert(
        "long_period".to_string(),
        (10..=50).step_by(5).map(|x| x as f64).collect(),
    );

    c.bench_function("parameter_sweep_10x9", |b| {
        b.iter(|| {
            backtest.run_parameter_sweep("ma_crossover", black_box(param_grid.clone()))
        })
    });
}

criterion_group!(benches, benchmark_parameter_sweep);
criterion_main!(benches);
