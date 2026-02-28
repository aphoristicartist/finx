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
    Warehouse::open(WarehouseConfig {
        ferrotick_home: home.clone(),
        db_path: home.join("cache").join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open")
}

#[test]
fn computes_required_phase7_features() {
    let bars = make_bars(80);
    let symbol = Symbol::parse("AAPL").unwrap();

    let engineer =
        FeatureEngineer::new(FeatureConfig::default(), IndicatorSelection::all()).unwrap();
    let rows = engineer.compute_for_symbol(&symbol, &bars).unwrap();

    assert_eq!(rows.len(), 80);
    assert!(rows.iter().any(|row| row.rsi.is_some()));
    assert!(rows.iter().any(|row| row.macd.is_some()));
    assert!(rows.iter().any(|row| row.bb_upper.is_some()));
    assert!(rows.iter().any(|row| row.atr.is_some()));
    assert!(rows.iter().any(|row| row.return_20d.is_some()));
    assert!(rows.iter().any(|row| row.rolling_std_20.is_some()));
}

#[tokio::test]
async fn store_roundtrip_and_parquet_export_work() {
    let warehouse = temp_warehouse();
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

    let written = store.upsert_features(&rows).unwrap();
    assert_eq!(written, rows.len());

    let loaded = store.load_features("AAPL", None, None).unwrap();
    assert_eq!(loaded.len(), rows.len());

    let output = PathBuf::from("/tmp/ferrotick-phase7-test-features.parquet");
    let start = UtcDateTime::parse("2024-02-01T00:00:00Z").unwrap();
    let end = UtcDateTime::parse("2024-12-31T23:59:59Z").unwrap();

    store
        .export_features_parquet("AAPL", start, end, output.as_path())
        .await
        .expect("export parquet");

    assert!(output.exists());
}
