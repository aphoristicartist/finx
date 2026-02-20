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

#[derive(Debug, Error)]
pub enum WarehouseError {
    #[error(transparent)]
    DuckDb(#[from] ::duckdb::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("query rejected: {0}")]
    QueryRejected(String),

    #[error("query timed out after {timeout_ms}ms")]
    QueryTimeout { timeout_ms: u64 },
}

#[derive(Debug, Clone)]
pub struct WarehouseConfig {
    pub finx_home: PathBuf,
    pub db_path: PathBuf,
    pub max_pool_size: usize,
}

impl Default for WarehouseConfig {
    fn default() -> Self {
        let finx_home = resolve_finx_home();
        let db_path = finx_home.join("cache").join("warehouse.duckdb");
        Self {
            finx_home,
            db_path,
            max_pool_size: 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QueryGuardrails {
    pub max_rows: usize,
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
    fn timeout(self) -> Duration {
        Duration::from_millis(self.query_timeout_ms.max(1))
    }

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

#[derive(Debug, Clone, Serialize)]
pub struct SqlColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<SqlColumn>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheSyncReport {
    pub cache_root: PathBuf,
    pub scanned_partitions: usize,
    pub synced_partitions: usize,
    pub skipped_partitions: usize,
    pub failed_partitions: usize,
}

#[derive(Debug, Clone)]
pub struct QuoteRecord {
    pub symbol: String,
    pub price: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub volume: Option<u64>,
    pub currency: String,
    pub as_of: String,
}

#[derive(Debug, Clone)]
pub struct BarRecord {
    pub symbol: String,
    pub ts: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FundamentalRecord {
    pub symbol: String,
    pub metric: String,
    pub value: f64,
    pub date: String,
}

#[derive(Debug, Clone)]
struct CachePartition {
    source: String,
    dataset: String,
    symbol: String,
    partition_date: String,
    path: PathBuf,
}

#[derive(Clone)]
pub struct Warehouse {
    config: WarehouseConfig,
    manager: DuckDbConnectionManager,
}

impl Warehouse {
    pub fn open_default() -> Result<Self, WarehouseError> {
        Self::open(WarehouseConfig::default())
    }

    pub fn open(config: WarehouseConfig) -> Result<Self, WarehouseError> {
        if let Some(parent) = config.db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let manager = DuckDbConnectionManager::new(config.db_path.clone(), config.max_pool_size);
        let warehouse = Self { config, manager };
        warehouse.initialize()?;
        Ok(warehouse)
    }

    pub fn initialize(&self) -> Result<(), WarehouseError> {
        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        migrations::apply_migrations(&connection)?;
        views::create_views(&connection)?;
        Ok(())
    }

    pub fn db_path(&self) -> &Path {
        self.manager.db_path()
    }

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

    pub fn sync_cache(&self) -> Result<CacheSyncReport, WarehouseError> {
        let cache_root = self.config.finx_home.join("cache").join("parquet");
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
                let sql = format!(
                    r#"
INSERT OR REPLACE INTO quotes_latest (
    symbol, price, bid, ask, volume, as_of, source, updated_at
) VALUES (
    '{symbol}', {price}, {bid}, {ask}, {volume},
    TRY_CAST('{as_of}' AS TIMESTAMP), '{source}', CURRENT_TIMESTAMP
);
INSERT OR IGNORE INTO instruments (
    symbol, name, exchange, currency, asset_class, is_active, source, updated_at
) VALUES (
    '{symbol}', '{symbol}', NULL, '{currency}', 'equity', TRUE, '{source}', CURRENT_TIMESTAMP
);
INSERT INTO ingest_log (request_id, symbol, source, dataset, status, latency_ms, timestamp)
VALUES ('{request_id}', '{symbol}', '{source}', 'quote', 'ok', {latency_ms}, CURRENT_TIMESTAMP);
"#,
                    symbol = escape_sql_string(row.symbol.as_str()),
                    price = row.price,
                    bid = sql_option_f64(row.bid),
                    ask = sql_option_f64(row.ask),
                    volume = sql_option_u64(row.volume),
                    as_of = escape_sql_string(row.as_of.as_str()),
                    currency = escape_sql_string(row.currency.as_str()),
                    source = escape_sql_string(source),
                    request_id = escape_sql_string(request_id),
                    latency_ms = latency_ms,
                );
                connection.execute_batch(sql.as_str())?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

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
                let sql = format!(
                    r#"
INSERT OR REPLACE INTO {table} (
    symbol, ts, open, high, low, close, volume, source, updated_at
) VALUES (
    '{symbol}', TRY_CAST('{ts}' AS TIMESTAMP), {open}, {high}, {low}, {close},
    {volume}, '{source}', CURRENT_TIMESTAMP
);
INSERT INTO ingest_log (request_id, symbol, source, dataset, status, latency_ms, timestamp)
VALUES ('{request_id}', '{symbol}', '{source}', '{dataset}', 'ok', {latency_ms}, CURRENT_TIMESTAMP);
"#,
                    table = table,
                    symbol = escape_sql_string(row.symbol.as_str()),
                    ts = escape_sql_string(row.ts.as_str()),
                    open = row.open,
                    high = row.high,
                    low = row.low,
                    close = row.close,
                    volume = sql_option_u64(row.volume),
                    source = escape_sql_string(source),
                    dataset = escape_sql_string(dataset),
                    request_id = escape_sql_string(request_id),
                    latency_ms = latency_ms,
                );
                connection.execute_batch(sql.as_str())?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

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
                let sql = format!(
                    r#"
INSERT OR REPLACE INTO fundamentals (
    symbol, metric, value, date, source, updated_at
) VALUES (
    '{symbol}', '{metric}', {value}, TRY_CAST('{date}' AS TIMESTAMP), '{source}', CURRENT_TIMESTAMP
);
INSERT INTO ingest_log (request_id, symbol, source, dataset, status, latency_ms, timestamp)
VALUES ('{request_id}', '{symbol}', '{source}', 'fundamentals', 'ok', {latency_ms}, CURRENT_TIMESTAMP);
"#,
                    symbol = escape_sql_string(row.symbol.as_str()),
                    metric = escape_sql_string(row.metric.as_str()),
                    value = row.value,
                    date = escape_sql_string(row.date.as_str()),
                    source = escape_sql_string(source),
                    request_id = escape_sql_string(request_id),
                    latency_ms = latency_ms,
                );
                connection.execute_batch(sql.as_str())?;
            }

            Ok(())
        })();

        finalize_transaction(&connection, result)
    }

    fn register_partition(&self, partition: &CachePartition) -> Result<(), WarehouseError> {
        let connection = self.manager.acquire(AccessMode::ReadWrite)?;
        let row_count = read_parquet_row_count(&connection, partition.path.as_path());
        let (min_ts, max_ts) = read_parquet_min_max_ts(&connection, partition.path.as_path());
        let checksum = file_checksum(partition.path.as_path())?;

        let sql = format!(
            r#"
INSERT OR REPLACE INTO cache_manifest (
    source, dataset, symbol, partition_date, path, row_count, min_ts, max_ts, checksum, updated_at
) VALUES (
    '{source}', '{dataset}', '{symbol}', '{partition_date}', '{path}', {row_count},
    {min_ts}, {max_ts}, '{checksum}', CURRENT_TIMESTAMP
);
INSERT INTO ingest_log (request_id, symbol, source, dataset, status, latency_ms, timestamp)
VALUES (
    'cache-sync:{source}:{dataset}:{symbol}:{partition_date}', '{symbol}', '{source}',
    '{dataset}', 'synced', NULL, CURRENT_TIMESTAMP
);
"#,
            source = escape_sql_string(partition.source.as_str()),
            dataset = escape_sql_string(partition.dataset.as_str()),
            symbol = escape_sql_string(partition.symbol.as_str()),
            partition_date = escape_sql_string(partition.partition_date.as_str()),
            path = escape_sql_string(path_to_sql(partition.path.as_path()).as_str()),
            row_count = row_count,
            min_ts = sql_option_timestamp(min_ts.as_deref()),
            max_ts = sql_option_timestamp(max_ts.as_deref()),
            checksum = escape_sql_string(checksum.as_str()),
        );
        connection.execute_batch(sql.as_str())?;
        Ok(())
    }
}

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

fn read_row(row: &::duckdb::Row<'_>, column_count: usize) -> Result<Vec<Value>, ::duckdb::Error> {
    let mut output = Vec::with_capacity(column_count);
    for index in 0..column_count {
        let value: DuckValue = row.get(index)?;
        output.push(to_json_value(value));
    }
    Ok(output)
}

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

fn number_from_f64(value: f64) -> Value {
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn normalize_sql(sql: &str) -> Result<&str, WarehouseError> {
    let normalized = sql.trim();
    if normalized.is_empty() {
        return Err(WarehouseError::QueryRejected(String::from(
            "query must not be empty",
        )));
    }
    Ok(normalized.trim_end_matches(';').trim())
}

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

fn has_multiple_statements(sql: &str) -> bool {
    sql.split(';')
        .filter(|part| !part.trim().is_empty())
        .count()
        > 1
}

fn ensure_timeout(started: Instant, timeout: Duration) -> Result<(), WarehouseError> {
    if started.elapsed() > timeout {
        return Err(WarehouseError::QueryTimeout {
            timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }
    Ok(())
}

fn resolve_finx_home() -> PathBuf {
    if let Some(path) = env::var_os("FINX_HOME") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return path;
        }
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".finx");
    }

    PathBuf::from(".finx")
}

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

fn read_parquet_row_count(connection: &Connection, parquet_path: &Path) -> i64 {
    let sql = format!(
        "SELECT COUNT(*) FROM read_parquet('{}')",
        escape_sql_string(path_to_sql(parquet_path).as_str())
    );
    connection
        .query_row(sql.as_str(), [], |row| row.get(0))
        .unwrap_or_default()
}

fn read_parquet_min_max_ts(
    connection: &Connection,
    parquet_path: &Path,
) -> (Option<String>, Option<String>) {
    let path = escape_sql_string(path_to_sql(parquet_path).as_str());
    for candidate in ["ts", "as_of", "date", "timestamp", "ex_date"] {
        let sql = format!(
            "SELECT CAST(MIN({column}) AS VARCHAR), CAST(MAX({column}) AS VARCHAR) FROM read_parquet('{path}')",
            column = candidate,
            path = path,
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

fn file_checksum(path: &Path) -> Result<String, std::io::Error> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let modified_nanos = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(format!("{:x}-{:x}", metadata.len(), modified_nanos))
}

fn path_to_sql(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn sql_option_f64(value: Option<f64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => String::from("NULL"),
    }
}

fn sql_option_u64(value: Option<u64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => String::from("NULL"),
    }
}

fn sql_option_timestamp(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("TRY_CAST('{}' AS TIMESTAMP)", escape_sql_string(value)),
        None => String::from("NULL"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn initializes_tables_and_views() {
        let temp = tempdir().expect("tempdir");
        let finx_home = temp.path().join("finx-home");
        let db_path = finx_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            finx_home,
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
        let finx_home = temp.path().join("finx-home");
        let db_path = finx_home.join("cache").join("warehouse.duckdb");

        let warehouse = Warehouse::open(WarehouseConfig {
            finx_home,
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
    fn cache_sync_is_idempotent() {
        let temp = tempdir().expect("tempdir");
        let finx_home = temp.path().join("finx-home");
        let parquet_dir = finx_home
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

        let db_path = finx_home.join("cache").join("warehouse.duckdb");
        let warehouse = Warehouse::open(WarehouseConfig {
            finx_home,
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
        let finx_home = temp.path().join("finx-home");
        let db_path = finx_home.join("cache").join("warehouse.duckdb");
        let warehouse = Warehouse::open(WarehouseConfig {
            finx_home,
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
