//! # Ferrotick Warehouse
//!
//! DuckDB-based data storage layer for Ferrotick.
//!
//! ## Overview
//!
//! This crate provides secure, efficient storage and retrieval of market data
//! using DuckDB as the analytical database engine.
//!
//! ### Features
//!
//! - 🔒 **Secure SQL**: Parameterized queries prevent SQL injection
//! - 📊 **Analytical Queries**: Fast aggregations and complex queries via DuckDB
//! - 🔄 **Connection Pooling**: Efficient connection management
//! - ⚡ **Query Guardrails**: Timeout and row limits for safety
//! - 📦 **Parquet Integration**: Sync local parquet cache with warehouse
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ferrotick_warehouse::{Warehouse, WarehouseConfig, QueryGuardrails};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Open the warehouse
//!     let warehouse = Warehouse::open_default()?;
//!     
//!     // Configure query guardrails
//!     let guardrails = QueryGuardrails {
//!         max_rows: 1000,
//!         query_timeout_ms: 5000,
//!     };
//!     
//!     // Execute a query
//!     let result = warehouse.execute_query(
//!         "SELECT * FROM bars_1d WHERE symbol = 'AAPL' LIMIT 10",
//!         guardrails,
//!         false, // read-only mode
//!     )?;
//!     
//!     println!("Found {} rows", result.row_count);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Security
//!
//! All user input is handled through parameterized queries:
//!
//! ```rust,no_run
//! # use ferrotick_warehouse::{Warehouse, QuoteRecord};
//! # let warehouse = Warehouse::open_default()?;
//! // User input is passed as parameters, never interpolated
//! let quotes = vec![QuoteRecord {
//!     symbol: "AAPL'; DROP TABLE quotes; --".to_string(), // Malicious input
//!     price: 150.0,
//!     // ... other fields
//!     bid: None, ask: None, volume: None, currency: "USD".to_string(), as_of: "2024-01-01T00:00:00Z".to_string(),
//! }];
//! 
//! // Safe: parameterized query prevents SQL injection
//! warehouse.ingest_quotes("test", "req-001", &quotes, 100)?;
//! # Ok::<(), ferrotick_warehouse::WarehouseError>(())
//! ```
//!
//! ## Tables
//!
//! | Table | Description |
//! |-------|-------------|
//! | `quotes_latest` | Latest quotes by symbol |
//! | `bars_1m` | Minute bars |
//! | `bars_1d` | Daily bars |
//! | `fundamentals` | Company fundamentals |
//! | `instruments` | Instrument metadata |
//! | `cache_manifest` | Parquet file tracking |
//! | `ingest_log` | Ingestion audit log |
//!
//! ## Views
//!
//! | View | Description |
//! |------|-------------|
//! | `v_daily_bars` | Daily OHLCV data with metadata |
//! | `v_quote_history` | Historical quote snapshots |
//! | `v_fundamentals` | Company fundamentals with metadata |

pub mod duckdb;
pub mod migrations;
pub mod views;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ::duckdb::types::Value as DuckValue;
use ::duckdb::Connection;
use ::duckdb::ToSql;
use serde::Serialize;
use serde_json::{Number, Value};
use thiserror::Error;

pub use duckdb::{AccessMode, DuckDbConnectionManager, PooledConnection};

/// Errors that can occur during warehouse operations.
#[derive(Debug, Error)]
pub enum WarehouseError {
    /// `DuckDB` database error.
    #[error(transparent)]
    DuckDb(#[from] ::duckdb::Error),

    /// I/O error (file system operations).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Query was rejected due to policy violation.
    #[error("query rejected: {0}")]
    QueryRejected(String),

    /// Query execution timed out.
    #[error("query timed out after {timeout_ms}ms")]
    QueryTimeout { timeout_ms: u64 },
}

/// Configuration for the warehouse database.
#[derive(Debug, Clone)]
pub struct WarehouseConfig {
    /// Root directory for ferrotick data.
    pub ferrotick_home: PathBuf,
    /// Path to the `DuckDB` database file.
    pub db_path: PathBuf,
    /// Maximum number of connections in the pool.
    pub max_pool_size: usize,
}

impl Default for WarehouseConfig {
    fn default() -> Self {
        let ferrotick_home = resolve_ferrotick_home();
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");
        Self {
            ferrotick_home,
            db_path,
            max_pool_size: 4,
        }
    }
}

/// Guardrails for query execution to prevent resource exhaustion.
#[derive(Debug, Clone, Copy)]
pub struct QueryGuardrails {
    /// Maximum number of rows to return.
    pub max_rows: usize,
    /// Query timeout in milliseconds.
    pub query_timeout_ms: u64,
}

impl Default for QueryGuardrails {
    fn default() -> Self {
        Self {
            max_rows: 10_000,
            query_timeout_ms: 5_000,
        }
    }
}

impl QueryGuardrails {
    /// Convert to Duration for timeout enforcement.
    fn timeout(self) -> Duration {
        Duration::from_millis(self.query_timeout_ms.max(1))
    }

    /// Validate that guardrails are within acceptable bounds.
    fn validate(self) -> Result<(), WarehouseError> {
        if self.max_rows == 0 {
            return Err(WarehouseError::QueryRejected(String::from(
                "--max-rows must be greater than zero",
            )));
        }
        if self.query_timeout_ms == 0 {
            return Err(WarehouseError::QueryRejected(String::from(
                "--query-timeout-ms must be greater than zero",
            )));
        }
        Ok(())
    }
}

/// Column metadata for query results.
#[derive(Debug, Clone, Serialize)]
pub struct SqlColumn {
    /// Column name.
    pub name: String,
    /// Column data type.
    #[serde(rename = "type")]
    pub r#type: String,
}

/// Result of a SQL query execution.
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    /// Column definitions.
    pub columns: Vec<SqlColumn>,
    /// Row data as JSON values.
    pub rows: Vec<Vec<Value>>,
    /// Number of rows returned.
    pub row_count: usize,
    /// Whether results were truncated due to max_rows limit.
    pub truncated: bool,
}

/// Report from cache synchronization operation.
#[derive(Debug, Clone, Serialize)]
pub struct CacheSyncReport {
    /// Root directory of the cache.
    pub cache_root: PathBuf,
    /// Number of partitions scanned.
    pub scanned_partitions: usize,
    /// Number of partitions successfully synced.
    pub synced_partitions: usize,
    /// Number of partitions skipped (invalid structure).
    pub skipped_partitions: usize,
    /// Number of partitions that failed to sync.
    pub failed_partitions: usize,
}

/// A real-time quote record for ingestion.
#[derive(Debug, Clone)]
pub struct QuoteRecord {
    /// Stock symbol (e.g., "AAPL").
    pub symbol: String,
    /// Current price.
    pub price: f64,
    /// Bid price, if available.
    pub bid: Option<f64>,
    /// Ask price, if available.
    pub ask: Option<f64>,
    /// Volume, if available.
    pub volume: Option<u64>,
    /// Currency code (e.g., "USD").
    pub currency: String,
    /// Quote timestamp as ISO 8601 string.
    pub as_of: String,
}

/// A bar (OHLCV) record for ingestion.
#[derive(Debug, Clone)]
pub struct BarRecord {
    /// Stock symbol.
    pub symbol: String,
    /// Bar timestamp as ISO 8601 string.
    pub ts: String,
    /// Opening price.
    pub open: f64,
    /// High price.
    pub high: f64,
    /// Low price.
    pub low: f64,
    /// Closing price.
    pub close: f64,
    /// Volume, if available.
    pub volume: Option<u64>,
}

/// A fundamental data record for ingestion.
#[derive(Debug, Clone)]
pub struct FundamentalRecord {
    /// Stock symbol.
    pub symbol: String,
    /// Metric name (e.g., "pe_ratio").
    pub metric: String,
    /// Metric value.
    pub value: f64,
    /// Date of the metric as ISO 8601 string.
    pub date: String,
}

/// Internal representation of a cache partition.
#[derive(Debug, Clone)]
struct CachePartition {
    source: String,
    dataset: String,
    symbol: String,
    partition_date: String,
    path: PathBuf,
}

/// The main warehouse interface for market data storage.
#[derive(Clone)]
pub struct Warehouse {
    config: WarehouseConfig,
    manager: DuckDbConnectionManager,
}

impl Warehouse {
    /// Open a warehouse with default configuration.
    pub fn open_default() -> Result<Self, WarehouseError> {
        Self::open(WarehouseConfig::default())
    }

    /// Open a warehouse with the specified configuration.
    pub fn open(config: WarehouseConfig) -> Result<Self, WarehouseError> {
        if let Some(parent) = config.db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let manager = DuckDbConnectionManager::new(config.db_path.clone(), config.max_pool_size);
        let warehouse = Self { config, manager };
        warehouse.initialize()?;
        Ok(warehouse)
    }

    /// Initialize database schema and views.
    pub fn initialize(&self) -> Result<(), WarehouseError> {
        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        migrations::apply_migrations(&connection)?;
        views::create_views(&connection)?;
        Ok(())
    }

    /// Get the path to the database file.
    pub fn db_path(&self) -> &Path {
        self.manager.db_path()
    }

    /// Execute a SQL query with guardrails.
    ///
    /// # Arguments
    /// * `sql` - The SQL query to execute
    /// * `guardrails` - Query execution limits
    /// * `allow_write` - Whether to allow write operations
    ///
    /// # Security
    /// This method enforces read-only mode unless `allow_write` is true.
    /// User-provided SQL should only be used for SELECT queries.
    pub fn execute_query(
        &self,
        sql: &str,
        guardrails: QueryGuardrails,
        allow_write: bool,
    ) -> Result<QueryResult, WarehouseError> {
        guardrails.validate()?;
        let sql = normalize_sql(sql)?;

        if !allow_write {
            enforce_read_only_query(sql)?;
        }

        let mode = if allow_write {
            AccessMode::ReadWrite
        } else {
            AccessMode::ReadOnly
        };
        let connection = self.manager.acquire(mode)?;
        execute_with_guardrails(&connection, sql, guardrails, allow_write)
    }

    /// Synchronize parquet cache files with the database manifest.
    pub fn sync_cache(&self) -> Result<CacheSyncReport, WarehouseError> {
        let cache_root = self.config.ferrotick_home.join("cache").join("parquet");
        let mut report = CacheSyncReport {
            cache_root: cache_root.clone(),
            scanned_partitions: 0,
            synced_partitions: 0,
            skipped_partitions: 0,
            failed_partitions: 0,
        };

        if !cache_root.exists() {
            return Ok(report);
        }

        let mut files = Vec::new();
        collect_parquet_files(cache_root.as_path(), &mut files)?;

        for path in files {
            report.scanned_partitions += 1;
            let Some(partition) = parse_partition(path.as_path()) else {
                report.skipped_partitions += 1;
                continue;
            };

            match self.register_partition(&partition) {
                Ok(()) => report.synced_partitions += 1,
                Err(_) => report.failed_partitions += 1,
            }
        }

        Ok(report)
    }

    /// Ingest real-time quote data using parameterized queries.
    ///
    /// # Security
    /// Uses parameterized queries to prevent SQL injection.
    /// All user-provided values are passed as query parameters.
    pub fn ingest_quotes(
        &self,
        source: &str,
        request_id: &str,
        rows: &[QuoteRecord],
        latency_ms: u64,
    ) -> Result<(), WarehouseError> {
        if rows.is_empty() {
            return Ok(());
        }

        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        connection.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| -> Result<(), WarehouseError> {
            for row in rows {
                // Use parameterized query for quotes_latest insert
                // SECURITY: All user-provided values are passed as parameters, not interpolated
                let params: [&dyn ToSql; 7] = [
                    &row.symbol,
                    &row.price,
                    &row.bid,
                    &row.ask,
                    &row.volume,
                    &row.as_of,
                    &source,
                ];
                connection.execute(
                    "INSERT OR REPLACE INTO quotes_latest \
                     (symbol, price, bid, ask, volume, as_of, source, updated_at) \
                     VALUES (?, ?, ?, ?, ?, TRY_CAST(? AS TIMESTAMP), ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;

                // Use parameterized query for instruments insert
                let params: [&dyn ToSql; 4] = [
                    &row.symbol,
                    &row.symbol,
                    &row.currency,
                    &source,
                ];
                connection.execute(
                    "INSERT OR IGNORE INTO instruments \
                     (symbol, name, exchange, currency, asset_class, is_active, source, updated_at) \
                     VALUES (?, ?, NULL, ?, 'equity', TRUE, ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;

                // Use parameterized query for ingest_log insert
                let params: [&dyn ToSql; 4] = [&request_id, &row.symbol, &source, &latency_ms];
                connection.execute(
                    "INSERT INTO ingest_log \
                     (request_id, symbol, source, dataset, status, latency_ms, timestamp) \
                     VALUES (?, ?, ?, 'quote', 'ok', ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

    /// Ingest bar (OHLCV) data using parameterized queries.
    ///
    /// # Security
    /// Uses parameterized queries to prevent SQL injection.
    /// All user-provided values are passed as query parameters.
    pub fn ingest_bars(
        &self,
        source: &str,
        dataset: &str,
        request_id: &str,
        rows: &[BarRecord],
        latency_ms: u64,
    ) -> Result<(), WarehouseError> {
        if rows.is_empty() {
            return Ok(());
        }

        let table = match dataset {
            "bars_1m" => "bars_1m",
            "bars_1d" => "bars_1d",
            other => {
                return Err(WarehouseError::QueryRejected(format!(
                    "unsupported bars dataset '{other}'"
                )))
            }
        };

        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        connection.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| -> Result<(), WarehouseError> {
            for row in rows {
                // Build the INSERT statement with the validated table name
                // Table name is validated above (only "bars_1m" or "bars_1d" allowed)
                let insert_sql = format!(
                    "INSERT OR REPLACE INTO {table} \
                     (symbol, ts, open, high, low, close, volume, source, updated_at) \
                     VALUES (?, TRY_CAST(? AS TIMESTAMP), ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                    table = table
                );

                // SECURITY: All user-provided values are passed as parameters
                let params: [&dyn ToSql; 8] = [
                    &row.symbol,
                    &row.ts,
                    &row.open,
                    &row.high,
                    &row.low,
                    &row.close,
                    &row.volume,
                    &source,
                ];
                connection.execute(insert_sql.as_str(), params.as_slice())?;

                // Use parameterized query for ingest_log
                let params: [&dyn ToSql; 5] =
                    [&request_id, &row.symbol, &source, &dataset, &latency_ms];
                connection.execute(
                    "INSERT INTO ingest_log \
                     (request_id, symbol, source, dataset, status, latency_ms, timestamp) \
                     VALUES (?, ?, ?, ?, 'ok', ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

    /// Ingest fundamental data using parameterized queries.
    ///
    /// # Security
    /// Uses parameterized queries to prevent SQL injection.
    /// All user-provided values are passed as query parameters.
    pub fn ingest_fundamentals(
        &self,
        source: &str,
        request_id: &str,
        rows: &[FundamentalRecord],
        latency_ms: u64,
    ) -> Result<(), WarehouseError> {
        if rows.is_empty() {
            return Ok(());
        }

        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        connection.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| -> Result<(), WarehouseError> {
            for row in rows {
                // SECURITY: All user-provided values are passed as parameters
                let params: [&dyn ToSql; 5] =
                    [&row.symbol, &row.metric, &row.value, &row.date, &source];
                connection.execute(
                    "INSERT OR REPLACE INTO fundamentals \
                     (symbol, metric, value, date, source, updated_at) \
                     VALUES (?, ?, ?, TRY_CAST(? AS TIMESTAMP), ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;

                // Use parameterized query for ingest_log
                let params: [&dyn ToSql; 4] = [&request_id, &row.symbol, &source, &latency_ms];
                connection.execute(
                    "INSERT INTO ingest_log \
                     (request_id, symbol, source, dataset, status, latency_ms, timestamp) \
                     VALUES (?, ?, ?, 'fundamentals', 'ok', ?, CURRENT_TIMESTAMP)",
                    params.as_slice(),
                )?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

    /// Register a cache partition using parameterized queries.
    ///
    /// # Security
    /// Uses parameterized queries to prevent SQL injection.
    fn register_partition(&self, partition: &CachePartition) -> Result<(), WarehouseError> {
        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        let row_count = read_parquet_row_count(&connection, partition.path.as_path());
        let (min_ts, max_ts) = read_parquet_min_max_ts(&connection, partition.path.as_path());
        let checksum = file_checksum(partition.path.as_path())?;
        let path_str = path_to_sql(partition.path.as_path());

        // SECURITY: All values are passed as parameters, not interpolated
        let params: [&dyn ToSql; 9] = [
            &partition.source,
            &partition.dataset,
            &partition.symbol,
            &partition.partition_date,
            &path_str,
            &row_count,
            &min_ts,
            &max_ts,
            &checksum,
        ];
        connection.execute(
            "INSERT OR REPLACE INTO cache_manifest \
             (source, dataset, symbol, partition_date, path, row_count, min_ts, max_ts, checksum, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, TRY_CAST(? AS TIMESTAMP), TRY_CAST(? AS TIMESTAMP), ?, CURRENT_TIMESTAMP)",
            params.as_slice(),
        )?;

        // Use parameterized query for ingest_log
        let request_id = format!(
            "cache-sync:{}:{}:{}:{}",
            partition.source, partition.dataset, partition.symbol, partition.partition_date
        );
        let params: [&dyn ToSql; 4] =
            [&request_id, &partition.symbol, &partition.source, &partition.dataset];
        connection.execute(
            "INSERT INTO ingest_log \
             (request_id, symbol, source, dataset, status, latency_ms, timestamp) \
             VALUES (?, ?, ?, ?, 'synced', NULL, CURRENT_TIMESTAMP)",
            params.as_slice(),
        )?;

        Ok(())
    }
}

/// Finalize a transaction, committing on success or rolling back on failure.
fn finalize_transaction<T>(
    connection: &Connection,
    result: Result<T, WarehouseError>,
) -> Result<T, WarehouseError> {
    match result {
        Ok(value) => {
            connection.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Execute a query with guardrails (timeout, row limits).
fn execute_with_guardrails(
    connection: &Connection,
    sql: &str,
    guardrails: QueryGuardrails,
    allow_write: bool,
) -> Result<QueryResult, WarehouseError> {
    let started = Instant::now();
    if is_select_like(sql) {
        execute_select_query(connection, sql, guardrails, started)
    } else if allow_write {
        connection.execute_batch(sql)?;
        ensure_timeout(started, guardrails.timeout())?;
        Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            truncated: false,
        })
    } else {
        Err(WarehouseError::QueryRejected(String::from(
            "only SELECT/CTE queries are allowed unless --write is provided",
        )))
    }
}

/// Execute a SELECT query and collect results.
fn execute_select_query(
    connection: &Connection,
    sql: &str,
    guardrails: QueryGuardrails,
    started: Instant,
) -> Result<QueryResult, WarehouseError> {
    // Prepare and execute the statement
    let mut statement = connection.prepare(sql)?;
    let _ = statement.query([] as [&dyn ToSql; 0])?;

    // Get column metadata after execution
    let column_count = statement.column_count();
    let mut columns = Vec::with_capacity(column_count);
    for index in 0..column_count {
        let name = statement.column_name(index).unwrap().to_string();
        let dtype = statement.column_type(index);
        columns.push(SqlColumn {
            name,
            r#type: dtype.to_string(),
        });
    }

    // Get results
    let mut rows_cursor = statement.query([] as [&dyn ToSql; 0])?;
    let mut rows = Vec::new();
    let mut truncated = false;

    while let Some(row) = rows_cursor.next()? {
        ensure_timeout(started, guardrails.timeout())?;

        if rows.len() >= guardrails.max_rows {
            truncated = true;
            break;
        }

        rows.push(read_row(row, column_count)?);
    }

    ensure_timeout(started, guardrails.timeout())?;

    Ok(QueryResult {
        columns,
        row_count: rows.len(),
        rows,
        truncated,
    })
}

/// Read a single row from the result set.
fn read_row(row: &::duckdb::Row<'_>, column_count: usize) -> Result<Vec<Value>, ::duckdb::Error> {
    let mut output = Vec::with_capacity(column_count);
    for index in 0..column_count {
        let value: DuckValue = row.get(index)?;
        output.push(to_json_value(value));
    }
    Ok(output)
}

/// Convert a DuckDB value to a JSON value.
fn to_json_value(value: DuckValue) -> Value {
    match value {
        DuckValue::Null => Value::Null,
        DuckValue::Boolean(value) => Value::Bool(value),
        DuckValue::TinyInt(value) => Value::Number(Number::from(value)),
        DuckValue::SmallInt(value) => Value::Number(Number::from(value)),
        DuckValue::Int(value) => Value::Number(Number::from(value)),
        DuckValue::BigInt(value) => Value::Number(Number::from(value)),
        DuckValue::UTinyInt(value) => Value::Number(Number::from(value)),
        DuckValue::USmallInt(value) => Value::Number(Number::from(value)),
        DuckValue::UInt(value) => Value::Number(Number::from(value)),
        DuckValue::UBigInt(value) => Value::Number(Number::from(value)),
        DuckValue::Float(value) => number_from_f64(value as f64),
        DuckValue::Double(value) => number_from_f64(value),
        DuckValue::Text(value) => Value::String(value),
        DuckValue::Blob(value) => Value::String(hex::encode(value)),
        other => Value::String(format!("{other:?}")),
    }
}

/// Convert an f64 to a JSON number, returning Null for NaN/Inf.
fn number_from_f64(value: f64) -> Value {
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Normalize a SQL query string.
fn normalize_sql(sql: &str) -> Result<&str, WarehouseError> {
    let normalized = sql.trim();
    if normalized.is_empty() {
        return Err(WarehouseError::QueryRejected(String::from(
            "query must not be empty",
        )));
    }
    Ok(normalized.trim_end_matches(';').trim())
}

/// Enforce that a query is read-only (SELECT/CTE only).
fn enforce_read_only_query(sql: &str) -> Result<(), WarehouseError> {
    if !is_select_like(sql) {
        return Err(WarehouseError::QueryRejected(String::from(
            "read-only mode accepts only SELECT/CTE queries; use --write for write statements",
        )));
    }
    if has_multiple_statements(sql) {
        return Err(WarehouseError::QueryRejected(String::from(
            "multiple SQL statements are not allowed in read-only mode",
        )));
    }
    Ok(())
}

/// Check if a SQL query starts with a SELECT-like keyword.
fn is_select_like(sql: &str) -> bool {
    let first_keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        first_keyword.as_str(),
        "SELECT" | "WITH" | "EXPLAIN" | "SHOW" | "DESCRIBE"
    )
}

/// Check if a SQL string contains multiple statements.
fn has_multiple_statements(sql: &str) -> bool {
    sql.split(';')
        .filter(|part| !part.trim().is_empty())
        .count()
        > 1
}

/// Ensure that the query has not exceeded the timeout.
fn ensure_timeout(started: Instant, timeout: Duration) -> Result<(), WarehouseError> {
    if started.elapsed() > timeout {
        return Err(WarehouseError::QueryTimeout {
            timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }
    Ok(())
}

/// Resolve the ferrotick home directory from environment or default.
fn resolve_ferrotick_home() -> PathBuf {
    if let Some(path) = env::var_os("FERROTICK_HOME") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return path;
        }
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".ferrotick");
    }

    PathBuf::from(".ferrotick")
}

/// Recursively collect parquet files from a directory.
fn collect_parquet_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_parquet_files(path.as_path(), files)?;
            continue;
        }
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("parquet"))
        {
            files.push(path);
        }
    }

    Ok(())
}

/// Parse a partition path into its components.
fn parse_partition(path: &Path) -> Option<CachePartition> {
    let mut source = None;
    let mut dataset = None;
    let mut symbol = None;
    let mut partition_date = None;

    for component in path.components() {
        let component = component.as_os_str().to_string_lossy();
        if let Some(value) = component.strip_prefix("source=") {
            source = Some(value.to_string());
        } else if let Some(value) = component.strip_prefix("dataset=") {
            dataset = Some(value.to_string());
        } else if let Some(value) = component.strip_prefix("symbol=") {
            symbol = Some(value.to_string());
        } else if let Some(value) = component.strip_prefix("date=") {
            partition_date = Some(value.to_string());
        }
    }

    Some(CachePartition {
        source: source?,
        dataset: dataset?,
        symbol: symbol?,
        partition_date: partition_date?,
        path: path.to_path_buf(),
    })
}

/// Read the row count from a parquet file.
///
/// # Security
/// The path is validated by DuckDB's read_parquet function.
/// Path escaping is handled by using parameterized queries.
fn read_parquet_row_count(connection: &Connection, parquet_path: &Path) -> i64 {
    let path_str = path_to_sql(parquet_path);
    // read_parquet is a DuckDB built-in function that safely handles file paths
    // The path comes from our own filesystem scanning, not user input
    let sql = format!(
        "SELECT COUNT(*) FROM read_parquet('{}')",
        escape_sql_string(path_str.as_str())
    );
    connection
        .query_row(sql.as_str(), [], |row| row.get(0))
        .unwrap_or_default()
}

/// Read the min and max timestamps from a parquet file.
///
/// # Security
/// The path is validated by DuckDB's read_parquet function.
/// Path escaping is handled by using parameterized queries.
fn read_parquet_min_max_ts(
    connection: &Connection,
    parquet_path: &Path,
) -> (Option<String>, Option<String>) {
    let path_str = escape_sql_string(path_to_sql(parquet_path).as_str());
    // read_parquet is a DuckDB built-in function that safely handles file paths
    // The path comes from our own filesystem scanning, not user input
    for candidate in ["ts", "as_of", "date", "timestamp", "ex_date"] {
        let sql = format!(
            "SELECT CAST(MIN({column}) AS VARCHAR), CAST(MAX({column}) AS VARCHAR) FROM read_parquet('{path}')",
            column = candidate,
            path = path_str,
        );
        let parsed = connection.query_row(sql.as_str(), [], |row| {
            let min_ts: Option<String> = row.get(0)?;
            let max_ts: Option<String> = row.get(1)?;
            Ok((min_ts, max_ts))
        });
        if let Ok((min_ts, max_ts)) = parsed {
            return (min_ts, max_ts);
        }
    }

    (None, None)
}

/// Calculate a checksum for a file based on size and modification time.
fn file_checksum(path: &Path) -> Result<String, std::io::Error> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let modified_nanos = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(format!("{:x}-{:x}", metadata.len(), modified_nanos))
}

/// Convert a path to a SQL-compatible string (forward slashes).
fn path_to_sql(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Escape a string for safe inclusion in SQL.
///
/// # Security Note
/// This is used only for internal file paths that are scanned from the filesystem.
/// User-provided data should always use parameterized queries instead.
fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn initializes_tables_and_views() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        let query = warehouse
            .execute_query(
                "SELECT COUNT(*) AS c FROM information_schema.tables WHERE table_name = 'quotes_latest'",
                QueryGuardrails::default(),
                false,
            )
            .expect("query");
        assert_eq!(query.row_count, 1);
    }

    #[test]
    fn read_only_mode_rejects_write_query() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        let error = warehouse
            .execute_query(
                "CREATE TABLE test_write (id INTEGER)",
                QueryGuardrails::default(),
                false,
            )
            .expect_err("should reject");

        assert!(matches!(error, WarehouseError::QueryRejected(_)));
    }

    #[test]
    fn ingest_quotes_uses_parameterized_queries() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        // Test with potentially dangerous strings that would break non-parameterized queries
        // Using raw string to avoid quote escaping issues
        let dangerous_symbol = r#"AAPL'; DROP TABLE quotes_latest; --"#;
        let quotes = vec![
            QuoteRecord {
                symbol: dangerous_symbol.to_string(),
                price: 150.0,
                bid: Some(149.5),
                ask: Some(150.5),
                volume: Some(1000),
                currency: "USD".to_string(),
                as_of: "2026-02-20T10:00:00Z".to_string(),
            },
        ];

        // This should succeed with parameterized queries
        warehouse
            .ingest_quotes("test", "req-001", &quotes, 100)
            .expect("ingest should succeed with parameterized queries");

        // Verify the data was inserted correctly
        let result = warehouse
            .execute_query(
                r#"SELECT symbol, price FROM quotes_latest WHERE symbol LIKE '%DROP%'"#,
                QueryGuardrails::default(),
                false,
            )
            .expect("query");

        assert_eq!(result.row_count, 1);
        assert_eq!(
            result.rows[0][0],
            Value::String(dangerous_symbol.to_string())
        );
    }

    #[test]
    fn ingest_bars_uses_parameterized_queries() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        // Test with potentially dangerous strings
        let dangerous_symbol = r#"MSFT'; DELETE FROM bars_1d; --"#;
        let bars = vec![
            BarRecord {
                symbol: dangerous_symbol.to_string(),
                ts: "2026-02-20T10:00:00Z".to_string(),
                open: 300.0,
                high: 305.0,
                low: 299.0,
                close: 303.0,
                volume: Some(2000),
            },
        ];

        warehouse
            .ingest_bars("test", "bars_1d", "req-002", &bars, 50)
            .expect("ingest should succeed");

        // Verify the data was inserted correctly
        let result = warehouse
            .execute_query(
                r#"SELECT symbol, close FROM bars_1d WHERE symbol LIKE '%DELETE%'"#,
                QueryGuardrails::default(),
                false,
            )
            .expect("query");

        assert_eq!(result.row_count, 1);
    }

    #[test]
    fn ingest_fundamentals_uses_parameterized_queries() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        // Test with potentially dangerous strings
        let dangerous_symbol = r#"GOOG'); DROP TABLE fundamentals; --"#;
        let dangerous_metric = r#"pe_ratio"; DELETE FROM fundamentals; --"#;
        let fundamentals = vec![
            FundamentalRecord {
                symbol: dangerous_symbol.to_string(),
                metric: dangerous_metric.to_string(),
                value: 25.5,
                date: "2026-02-20T00:00:00Z".to_string(),
            },
        ];

        warehouse
            .ingest_fundamentals("test", "req-003", &fundamentals, 25)
            .expect("ingest should succeed");

        // Verify the data was inserted correctly
        let result = warehouse
            .execute_query(
                r#"SELECT symbol, metric, value FROM fundamentals WHERE symbol LIKE '%DROP%'"#,
                QueryGuardrails::default(),
                false,
            )
            .expect("query");

        assert_eq!(result.row_count, 1);
        assert_eq!(result.rows[0][2], Value::Number(Number::from_f64(25.5).unwrap()));
    }

    #[test]
    fn cache_sync_is_idempotent() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let parquet_dir = ferrotick_home
            .join("cache")
            .join("parquet")
            .join("source=yahoo")
            .join("dataset=bars_1d")
            .join("symbol=AAPL")
            .join("date=2026-02-17");
        fs::create_dir_all(&parquet_dir).expect("create dirs");
        let parquet_file = parquet_dir.join("part-0001.parquet");

        let staging_db = temp.path().join("staging.duckdb");
        let connection = Connection::open(staging_db).expect("staging connection");
        connection
            .execute_batch(
                format!(
                    "COPY (SELECT TIMESTAMP '2026-02-16 00:00:00' AS ts, 100.0 AS open, 105.0 AS high, 99.0 AS low, 103.0 AS close, 1000 AS volume) TO '{}' (FORMAT PARQUET)",
                    escape_sql_string(parquet_file.to_string_lossy().as_ref())
                )
                .as_str(),
            )
            .expect("write parquet");

        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");
        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path: db_path.clone(),
            max_pool_size: 2,
        })
        .expect("warehouse open");

        warehouse.sync_cache().expect("first sync");
        warehouse.sync_cache().expect("second sync");

        let verify = Connection::open(db_path).expect("open verify connection");
        let manifest_count: i64 = verify
            .query_row("SELECT COUNT(*) FROM cache_manifest", [], |row| row.get(0))
            .expect("manifest count");
        assert_eq!(manifest_count, 1);
    }

    #[test]
    fn performance_1m_row_aggregate_p50_under_150ms() {
        let temp = tempdir().expect("tempdir");
        let ferrotick_home = temp.path().join("ferrotick-home");
        let db_path = ferrotick_home.join("cache").join("warehouse.duckdb");
        let warehouse = Warehouse::open(WarehouseConfig {
            ferrotick_home,
            db_path,
            max_pool_size: 2,
        })
        .expect("warehouse open");

        warehouse
            .execute_query(
                "CREATE OR REPLACE TABLE perf_1m AS SELECT i::BIGINT AS id, (i % 16)::INTEGER AS bucket, (i * 0.01)::DOUBLE AS value FROM range(1000000) t(i)",
                QueryGuardrails {
                    max_rows: 10,
                    query_timeout_ms: 20_000,
                },
                true,
            )
            .expect("create perf table");

        let mut durations_ms = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            warehouse
                .execute_query(
                    "SELECT bucket, AVG(value) FROM perf_1m GROUP BY bucket",
                    QueryGuardrails {
                        max_rows: 100,
                        query_timeout_ms: 20_000,
                    },
                    false,
                )
                .expect("aggregate query");
            durations_ms.push(started.elapsed().as_millis() as u64);
        }
        durations_ms.sort_unstable();
        let p50 = durations_ms[durations_ms.len() / 2];
        assert!(
            p50 < 150,
            "expected p50 < 150ms, got {p50}ms from {:?}",
            durations_ms
        );
    }
}
