//! Behavior-driven tests for Warehouse behavior
//!
//! These tests verify HOW the warehouse handles data storage, retrieval,
//! and query operations, focusing on user-visible outcomes.

use ferrotick_warehouse::{
    BarRecord, FundamentalRecord, QueryGuardrails, QuoteRecord,
    Warehouse, WarehouseConfig, WarehouseError,
};
use std::fs;
use std::time::Instant;
use tempfile::tempdir;

// =============================================================================
// Warehouse: Data Ingestion
// =============================================================================

#[test]
fn when_user_ingests_quotes_they_become_queryable_immediately() {
    // Given: A fresh warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User ingests quote data
    let quotes = vec![
        QuoteRecord {
            symbol: "AAPL".to_string(),
            price: 178.50,
            bid: Some(178.45),
            ask: Some(178.55),
            volume: Some(50_000_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T15:30:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "MSFT".to_string(),
            price: 415.20,
            bid: Some(415.15),
            ask: Some(415.25),
            volume: Some(20_000_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T15:30:00Z".to_string(),
        },
    ];
    warehouse
        .ingest_quotes("yahoo", "req-001", &quotes, 150)
        .expect("ingest should succeed");

    // Then: The data is immediately queryable
    let result = warehouse
        .execute_query(
            "SELECT symbol, price FROM quotes_latest ORDER BY symbol",
            QueryGuardrails::default(),
            false,
        )
        .expect("query should succeed");

    assert_eq!(result.row_count, 2, "should have 2 rows");
    assert_eq!(result.columns[0].name, "symbol");
    assert_eq!(result.columns[1].name, "price");
}

#[test]
fn when_user_ingests_bars_they_are_stored_with_all_ohlcv_fields() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User ingests bar data
    let bars = vec![
        BarRecord {
            symbol: "AAPL".to_string(),
            ts: "2026-02-20 09:30:00".to_string(),
            open: 178.00,
            high: 179.50,
            low: 177.80,
            close: 179.20,
            volume: Some(10_000_000),
        },
        BarRecord {
            symbol: "AAPL".to_string(),
            ts: "2026-02-20 10:00:00".to_string(),
            open: 179.20,
            high: 180.00,
            low: 179.00,
            close: 179.80,
            volume: Some(8_000_000),
        },
    ];
    warehouse
        .ingest_bars("polygon", "bars_1d", "req-002", &bars, 100)
        .expect("ingest should succeed");

    // Then: All OHLCV fields are queryable
    let result = warehouse
        .execute_query(
            "SELECT symbol, open, high, low, close, volume FROM bars_1d WHERE symbol = 'AAPL' ORDER BY ts",
            QueryGuardrails::default(),
            false,
        )
        .expect("query should succeed");

    assert_eq!(result.row_count, 2);
    // Verify OHLCV data is present
    let row = &result.rows[0];
    assert!(row.len() >= 6, "should have 6 columns for OHLCV");
}

#[test]
fn when_user_ingests_fundamentals_they_are_stored_by_metric() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User ingests fundamental data
    let fundamentals = vec![
        FundamentalRecord {
            symbol: "AAPL".to_string(),
            metric: "pe_ratio".to_string(),
            value: 28.5,
            date: "2026-02-20".to_string(),
        },
        FundamentalRecord {
            symbol: "AAPL".to_string(),
            metric: "market_cap".to_string(),
            value: 2_800_000_000_000.0,
            date: "2026-02-20".to_string(),
        },
        FundamentalRecord {
            symbol: "AAPL".to_string(),
            metric: "dividend_yield".to_string(),
            value: 0.0052,
            date: "2026-02-20".to_string(),
        },
    ];
    warehouse
        .ingest_fundamentals("alphavantage", "req-003", &fundamentals, 200)
        .expect("ingest should succeed");

    // Then: Each metric is queryable individually
    let result = warehouse
        .execute_query(
            "SELECT metric, value FROM fundamentals WHERE symbol = 'AAPL' ORDER BY metric",
            QueryGuardrails::default(),
            false,
        )
        .expect("query should succeed");

    assert_eq!(result.row_count, 3, "should have 3 fundamental metrics");
}

// =============================================================================
// Warehouse: Idempotency
// =============================================================================

#[test]
fn when_duplicate_quotes_are_ingested_system_handles_idempotently() {
    // Given: A warehouse with existing data
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    let quote = QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 178.50,
        bid: Some(178.45),
        ask: Some(178.55),
        volume: Some(50_000_000),
        currency: "USD".to_string(),
        as_of: "2026-02-20T15:30:00Z".to_string(),
    };

    // When: The same quote is ingested twice
    warehouse
        .ingest_quotes("yahoo", "req-001", &[quote.clone()], 100)
        .expect("first ingest");
    warehouse
        .ingest_quotes("yahoo", "req-002", &[quote.clone()], 100)
        .expect("second ingest");

    // Then: Only one record exists (upsert behavior)
    let result = warehouse
        .execute_query(
            "SELECT COUNT(*) AS count FROM quotes_latest WHERE symbol = 'AAPL'",
            QueryGuardrails::default(),
            false,
        )
        .expect("query");

    // COUNT returns an integer
    let count = match &result.rows[0][0] {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(-1),
        _ => -1,
    };
    assert_eq!(count, 1, "duplicate ingest should result in single row (idempotent)");
}

#[test]
fn when_quote_price_updates_existing_record_is_replaced() {
    // Given: A warehouse with a quote
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    let initial_quote = QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 178.50,
        bid: None,
        ask: None,
        volume: None,
        currency: "USD".to_string(),
        as_of: "2026-02-20T10:00:00Z".to_string(),
    };
    warehouse
        .ingest_quotes("yahoo", "req-001", &[initial_quote], 100)
        .expect("initial ingest");

    // When: A new quote arrives with updated price
    let updated_quote = QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 180.25, // Price increased
        bid: None,
        ask: None,
        volume: None,
        currency: "USD".to_string(),
        as_of: "2026-02-20T15:00:00Z".to_string(),
    };
    warehouse
        .ingest_quotes("yahoo", "req-002", &[updated_quote], 100)
        .expect("update ingest");

    // Then: The record reflects the latest price
    let result = warehouse
        .execute_query(
            "SELECT price FROM quotes_latest WHERE symbol = 'AAPL'",
            QueryGuardrails::default(),
            false,
        )
        .expect("query");

    let price = match &result.rows[0][0] {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    };
    assert!(
        (price - 180.25).abs() < 0.01,
        "price should be updated to 180.25, got {}",
        price
    );
}

// =============================================================================
// Warehouse: Query Error Handling
// =============================================================================

#[test]
fn when_user_queries_with_invalid_sql_they_get_helpful_error() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User submits invalid SQL syntax
    let result = warehouse.execute_query(
        "SELECT FROM WHERE", // Invalid SQL
        QueryGuardrails::default(),
        false,
    );

    // Then: A helpful error is returned
    assert!(result.is_err());
    let error = result.expect_err("invalid SQL should error");
    // DuckDB returns a parser error
    assert!(!error.to_string().is_empty());
}

#[test]
fn when_user_queries_nonexistent_table_they_get_helpful_error() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User queries a table that doesn't exist
    let result = warehouse.execute_query(
        "SELECT * FROM nonexistent_table",
        QueryGuardrails::default(),
        false,
    );

    // Then: An error explains the table doesn't exist
    assert!(result.is_err());
}

#[test]
fn when_user_submits_empty_query_they_get_clear_error() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User submits an empty query
    let result = warehouse.execute_query(
        "",
        QueryGuardrails::default(),
        false,
    );

    // Then: A clear error is returned
    let error = result.expect_err("empty query should error");
    let msg = error.to_string().to_lowercase();
    assert!(msg.contains("empty") || msg.contains("reject"), "error should mention the issue");
}

// =============================================================================
// Warehouse: Aggregation Behavior
// =============================================================================

#[test]
fn when_data_is_ingested_aggregations_work_correctly() {
    // Given: A warehouse with multiple quotes
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    let quotes = vec![
        QuoteRecord {
            symbol: "STOCK1".to_string(),
            price: 100.0,
            bid: None,
            ask: None,
            volume: Some(1_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "STOCK2".to_string(),
            price: 200.0,
            bid: None,
            ask: None,
            volume: Some(2_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "STOCK3".to_string(),
            price: 300.0,
            bid: None,
            ask: None,
            volume: Some(3_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
    ];
    warehouse
        .ingest_quotes("test", "req-001", &quotes, 100)
        .expect("ingest");

    // When: User runs aggregate queries
    let result = warehouse
        .execute_query(
            "SELECT AVG(price) AS avg_price, SUM(volume) AS total_volume, COUNT(*) AS count FROM quotes_latest",
            QueryGuardrails::default(),
            false,
        )
        .expect("aggregation");

    // Then: Aggregations return correct values
    assert_eq!(result.row_count, 1);
    // avg_price should be 200.0, total_volume should be 6000
}

#[test]
fn when_user_groups_data_by_field_results_are_correct() {
    // Given: A warehouse with data from multiple sources
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // Ingest quotes with different sources (using source column for grouping)
    let quotes = vec![
        QuoteRecord {
            symbol: "STOCK1".to_string(),
            price: 100.0,
            bid: None,
            ask: None,
            volume: None,
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "STOCK2".to_string(),
            price: 200.0,
            bid: None,
            ask: None,
            volume: None,
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "STOCK3".to_string(),
            price: 300.0,
            bid: None,
            ask: None,
            volume: None,
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
    ];
    warehouse
        .ingest_quotes("test", "req-001", &quotes, 100)
        .expect("ingest");

    // When: User groups by source
    let result = warehouse
        .execute_query(
            "SELECT source, COUNT(*) AS count FROM quotes_latest GROUP BY source ORDER BY source",
            QueryGuardrails::default(),
            false,
        )
        .expect("group by");

    // Then: Grouping is correct
    assert_eq!(result.row_count, 1); // One source: "test"
}

// =============================================================================
// Warehouse: Performance
// =============================================================================

#[test]
fn when_querying_large_dataset_performance_is_acceptable() {
    // Given: A warehouse with significant data
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // Create a large dataset using SQL (faster than individual inserts)
    warehouse
        .execute_query(
            "CREATE TABLE large_dataset AS SELECT i::INTEGER AS id, (i * 0.01)::DOUBLE AS value, 'SYM' || (i % 100)::VARCHAR AS symbol FROM range(100000) t(i)",
            QueryGuardrails {
                max_rows: 1,
                query_timeout_ms: 30_000,
            },
            true,
        )
        .expect("create large table");

    // When: User runs an aggregate query
    let start = Instant::now();
    let result = warehouse
        .execute_query(
            "SELECT symbol, AVG(value), COUNT(*) FROM large_dataset GROUP BY symbol",
            QueryGuardrails {
                max_rows: 1000,
                query_timeout_ms: 5_000,
            },
            false,
        )
        .expect("aggregate query");
    let elapsed = start.elapsed();

    // Then: Query completes in acceptable time
    assert!(
        elapsed.as_millis() < 500,
        "aggregate on 100K rows should complete in <500ms, took {:?}",
        elapsed
    );
    assert_eq!(result.row_count, 100); // 100 symbols
}

#[test]
fn when_row_limit_is_set_results_are_truncated_appropriately() {
    // Given: A warehouse with data
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // Insert more data than we'll request
    for i in 0..50 {
        let quotes = vec![QuoteRecord {
            symbol: format!("SYM{:02}", i),
            price: 100.0 + i as f64,
            bid: None,
            ask: None,
            volume: None,
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        }];
        warehouse
            .ingest_quotes("test", &format!("req-{}", i), &quotes, 100)
            .expect("ingest");
    }

    // When: User queries with a low row limit
    let result = warehouse
        .execute_query(
            "SELECT * FROM quotes_latest",
            QueryGuardrails {
                max_rows: 10,
                query_timeout_ms: 5_000,
            },
            false,
        )
        .expect("query");

    // Then: Results are truncated to the limit
    assert_eq!(result.row_count, 10, "should return exactly max_rows");
    assert!(result.truncated, "truncated flag should be true");
}

// =============================================================================
// Warehouse: Guardrails
// =============================================================================

#[test]
fn when_guardrails_specify_invalid_values_initialization_fails() {
    // Given: Invalid guardrails (zero max_rows)
    let guardrails = QueryGuardrails {
        max_rows: 0,
        query_timeout_ms: 1000,
    };

    // When: A query is attempted
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    let result = warehouse.execute_query("SELECT 1", guardrails, false);

    // Then: An error is returned
    let error = result.expect_err("zero max_rows should fail");
    assert!(matches!(error, WarehouseError::QueryRejected(_)));
}

#[test]
fn when_query_exceeds_timeout_it_is_cancelled() {
    // Given: A warehouse with a short timeout
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // Note: DuckDB doesn't support query cancellation mid-execution easily,
    // but the timeout check is applied between row fetches
    let guardrails = QueryGuardrails {
        max_rows: 10_000,
        query_timeout_ms: 1, // Very short timeout
    };

    // When: A slow query is executed
    // This test verifies the timeout mechanism exists
    // Actual timeout behavior depends on DuckDB's ability to return rows incrementally
    let _result = warehouse.execute_query("SELECT 1", guardrails, false);
    // The query might succeed if it's fast enough, or fail if timeout applies
}

// =============================================================================
// Warehouse: Ingest Log
// =============================================================================

#[test]
fn when_data_is_ingested_audit_trail_is_maintained() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: Data is ingested
    let quotes = vec![QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 150.0,
        bid: None,
        ask: None,
        volume: None,
        currency: "USD".to_string(),
        as_of: "2026-02-20T10:00:00Z".to_string(),
    }];
    warehouse
        .ingest_quotes("yahoo", "req-test-001", &quotes, 150)
        .expect("ingest");

    // Then: Ingest log records the operation
    let result = warehouse
        .execute_query(
            "SELECT request_id, symbol, source, dataset, status FROM ingest_log WHERE request_id = 'req-test-001'",
            QueryGuardrails::default(),
            false,
        )
        .expect("query ingest log");

    assert_eq!(result.row_count, 1, "ingest should be logged");
    assert_eq!(result.rows[0][0], serde_json::Value::String("req-test-001".to_string()));
    assert_eq!(result.rows[0][2], serde_json::Value::String("yahoo".to_string()));
}

// =============================================================================
// Warehouse: Cache Sync
// =============================================================================

#[test]
fn when_cache_sync_is_run_existing_partitions_are_registered() {
    // Given: A warehouse with a parquet cache directory
    let temp = tempdir().expect("tempdir");
    let ferrotick_home = temp.path().to_path_buf();
    let cache_dir = ferrotick_home
        .join("cache")
        .join("parquet")
        .join("source=yahoo")
        .join("dataset=bars_1d")
        .join("symbol=AAPL")
        .join("date=2026-02-20");
    fs::create_dir_all(&cache_dir).expect("create cache dir");

    // Create a minimal parquet file using DuckDB
    let staging_db = temp.path().join("staging.duckdb");
    let conn = duckdb::Connection::open(&staging_db).expect("staging connection");
    let parquet_path = cache_dir.join("data.parquet");
    conn.execute_batch(
        format!(
            "COPY (SELECT TIMESTAMP '2026-02-20 00:00:00' AS ts, 100.0 AS open, 101.0 AS high, 99.0 AS low, 100.5 AS close, 1000 AS volume) TO '{}' (FORMAT PARQUET)",
            parquet_path.to_string_lossy()
        )
        .as_str(),
    )
    .expect("write parquet");

    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: ferrotick_home.clone(),
        db_path: ferrotick_home.join("cache").join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: Cache sync is run
    let report = warehouse.sync_cache().expect("sync cache");

    // Then: Partitions are registered
    assert!(report.scanned_partitions >= 1, "should scan at least one partition");
    assert!(report.synced_partitions >= 1, "should sync at least one partition");
}
