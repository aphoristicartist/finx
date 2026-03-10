use std::path::PathBuf;

use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_ml::{FeatureConfig, FeatureEngineer, FeatureStore, IndicatorSelection};
use ferrotick_warehouse::{BarRecord, Warehouse, WarehouseConfig};
use tempfile::tempdir;

fn make_bars(count: usize) -> Vec<Bar> {
    let mut bars = Vec::with_capacity(count);
    for i in 0..count {
        let ts =
            UtcDateTime::parse(format!("2024-01-{:02}T00:00:00Z", (i % 28) + 1).as_str()).unwrap();
        let close = 100.0 + i as f64 * 0.5;
        bars.push(
            Bar::new(
                ts,
                close - 1.0,
                close + 1.0,
                close - 2.0,
                close,
                Some(1_000),
                None,
            )
            .unwrap(),
        );
    }
    bars
}

fn temp_warehouse() -> Warehouse {
    let temp = tempdir().expect("tempdir");
    let home = temp.path().join("ferrotick-home");
    std::fs::create_dir_all(home.join("cache")).expect("create cache dir");
    Warehouse::open(WarehouseConfig {
        ferrotick_home: home.clone(),
        db_path: home.join("cache").join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open")
}

fn assert_feature_values_are_finite_and_valid(rows: &[ferrotick_ml::FeatureRow]) {
    for (index, row) in rows.iter().enumerate() {
        if let Some(rsi) = row.rsi {
            assert!(
                (0.0..=100.0).contains(&rsi),
                "row {index} has RSI out of range: {rsi}"
            );
            assert!(rsi.is_finite(), "row {index} has non-finite RSI: {rsi}");
        }
        for (name, value) in [
            ("macd", row.macd),
            ("macd_signal", row.macd_signal),
            ("bb_upper", row.bb_upper),
            ("bb_lower", row.bb_lower),
            ("atr", row.atr),
            ("return_1d", row.return_1d),
            ("return_5d", row.return_5d),
            ("return_20d", row.return_20d),
            ("rolling_mean_20", row.rolling_mean_20),
            ("rolling_std_20", row.rolling_std_20),
            ("lag_1", row.lag_1),
            ("lag_2", row.lag_2),
            ("lag_3", row.lag_3),
            ("rolling_momentum", row.rolling_momentum),
        ] {
            if let Some(feature_value) = value {
                assert!(
                    feature_value.is_finite(),
                    "row {index} has non-finite {name}: {feature_value}"
                );
            }
        }
    }
}

fn assert_optional_feature_behavior(
    rows: &[ferrotick_ml::FeatureRow],
    feature_name: &str,
    expected_populated_count: usize,
    extractor: impl Fn(&ferrotick_ml::FeatureRow) -> Option<f64>,
    is_in_valid_range: impl Fn(f64) -> bool,
    range_description: &str,
) {
    const EXPECTED_COUNTED_FEATURES: [&str; 6] = [
        "rsi",
        "macd",
        "bb_upper",
        "atr",
        "return_20d",
        "rolling_std_20",
    ];

    assert!(
        EXPECTED_COUNTED_FEATURES.contains(&feature_name),
        "unexpected feature name in behavioral assertion: {feature_name}"
    );

    let values: Vec<f64> = rows.iter().filter_map(extractor).collect();
    assert_eq!(
        values.len(),
        expected_populated_count,
        "{feature_name} should be populated on {expected_populated_count} rows"
    );
    assert!(
        values.iter().all(|value| value.is_finite()),
        "{feature_name} values must all be finite"
    );
    assert!(
        values.iter().copied().all(is_in_valid_range),
        "{feature_name} values must be {range_description}"
    );
}

#[test]
fn computes_required_phase7_features() {
    let bars = make_bars(80);
    let symbol = Symbol::parse("AAPL").unwrap();

    let engineer =
        FeatureEngineer::new(FeatureConfig::default(), IndicatorSelection::all()).unwrap();
    let rows = engineer.compute_for_symbol(&symbol, &bars).unwrap();

    assert_eq!(rows.len(), 80);
    assert_optional_feature_behavior(
        &rows,
        "rsi",
        67,
        |row| row.rsi,
        |value| (0.0..=100.0).contains(&value),
        "within [0, 100]",
    );
    assert_optional_feature_behavior(
        &rows,
        "macd",
        47,
        |row| row.macd,
        |value| value.abs() <= 10.0,
        "within [-10, 10] for the monotonic synthetic series",
    );
    assert_optional_feature_behavior(
        &rows,
        "bb_upper",
        61,
        |row| row.bb_upper,
        |value| (95.0..=200.0).contains(&value),
        "within [95, 200] for the synthetic price path",
    );
    assert_optional_feature_behavior(
        &rows,
        "atr",
        67,
        |row| row.atr,
        |value| (0.0..=10.0).contains(&value),
        "within [0, 10]",
    );
    assert_optional_feature_behavior(
        &rows,
        "return_20d",
        60,
        |row| row.return_20d,
        |value| (-1.0..=1.0).contains(&value),
        "within [-1, 1]",
    );
    assert_optional_feature_behavior(
        &rows,
        "rolling_std_20",
        61,
        |row| row.rolling_std_20,
        |value| (0.0..=20.0).contains(&value),
        "within [0, 20]",
    );
    assert_feature_values_are_finite_and_valid(&rows);
}

#[tokio::test]
async fn store_roundtrip_and_parquet_export_work() {
    // Use a persistent temp directory instead of tempdir() to avoid cleanup issues
    let home = std::env::temp_dir().join("ferrotick-test-phase7");
    let _ = std::fs::remove_dir_all(&home); // Clean up from previous runs
    std::fs::create_dir_all(home.join("cache")).expect("create cache dir");

    let db_path = home.join("cache").join("warehouse.duckdb");

    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: home.clone(),
        db_path: db_path.clone(),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    let store = FeatureStore::new(warehouse.clone());

    // Seed bars into bars_1d
    let input_bars: Vec<BarRecord> = (0..60)
        .map(|idx| BarRecord {
            symbol: String::from("AAPL"),
            ts: format!("2024-02-{:02}T00:00:00Z", (idx % 28) + 1),
            open: 100.0 + idx as f64,
            high: 101.0 + idx as f64,
            low: 99.0 + idx as f64,
            close: 100.5 + idx as f64,
            volume: Some(1_000),
        })
        .collect();

    warehouse
        .ingest_bars("test", "bars_1d", "req-ml-001", &input_bars, 10)
        .expect("ingest bars");

    let symbol = Symbol::parse("AAPL").unwrap();
    let bars = store.load_daily_bars(&symbol, None, None).unwrap();
    let engineer =
        FeatureEngineer::new(FeatureConfig::default(), IndicatorSelection::all()).unwrap();
    let rows = engineer.compute_for_symbol(&symbol, &bars).unwrap();

    // upsert_features will automatically create the table via ensure_table()
    let written = store
        .upsert_features(&rows)
        .expect("upsert_features should succeed");
    assert_eq!(written, rows.len());

    let loaded = store
        .load_features("AAPL", None, None)
        .expect("load_features should succeed");
    assert_eq!(loaded.len(), rows.len());
    assert_feature_values_are_finite_and_valid(&loaded);

    let output = PathBuf::from("/tmp/ferrotick-phase7-test-features.parquet");
    let start = UtcDateTime::parse("2024-02-01T00:00:00Z").unwrap();
    let end = UtcDateTime::parse("2024-12-31T23:59:59Z").unwrap();

    store
        .export_features_parquet("AAPL", start, end, output.as_path())
        .await
        .expect("export parquet");

    assert!(output.exists());
}
