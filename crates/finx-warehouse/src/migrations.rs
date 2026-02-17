use ::duckdb::Connection;

struct Migration {
    version: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0001_core_tables",
        sql: r#"
CREATE TABLE IF NOT EXISTS instruments (
    symbol TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    exchange TEXT,
    currency TEXT NOT NULL,
    asset_class TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS quotes_latest (
    symbol TEXT PRIMARY KEY,
    price DOUBLE NOT NULL,
    bid DOUBLE,
    ask DOUBLE,
    volume BIGINT,
    as_of TIMESTAMP NOT NULL,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS bars_1m (
    symbol TEXT NOT NULL,
    ts TIMESTAMP NOT NULL,
    open DOUBLE NOT NULL,
    high DOUBLE NOT NULL,
    low DOUBLE NOT NULL,
    close DOUBLE NOT NULL,
    volume BIGINT,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, ts)
);

CREATE TABLE IF NOT EXISTS bars_1d (
    symbol TEXT NOT NULL,
    ts TIMESTAMP NOT NULL,
    open DOUBLE NOT NULL,
    high DOUBLE NOT NULL,
    low DOUBLE NOT NULL,
    close DOUBLE NOT NULL,
    volume BIGINT,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, ts)
);

CREATE TABLE IF NOT EXISTS fundamentals (
    symbol TEXT NOT NULL,
    metric TEXT NOT NULL,
    value DOUBLE NOT NULL,
    date TIMESTAMP NOT NULL,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, metric, date)
);

CREATE TABLE IF NOT EXISTS corporate_actions (
    symbol TEXT NOT NULL,
    type TEXT NOT NULL,
    date TIMESTAMP NOT NULL,
    details TEXT,
    source TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(symbol, type, date)
);

CREATE TABLE IF NOT EXISTS cache_manifest (
    source TEXT NOT NULL,
    dataset TEXT NOT NULL,
    symbol TEXT NOT NULL,
    partition_date DATE NOT NULL,
    path TEXT NOT NULL,
    row_count BIGINT NOT NULL,
    min_ts TIMESTAMP,
    max_ts TIMESTAMP,
    checksum TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(source, dataset, symbol, partition_date, path)
);

CREATE TABLE IF NOT EXISTS ingest_log (
    request_id TEXT NOT NULL,
    symbol TEXT,
    source TEXT NOT NULL,
    dataset TEXT NOT NULL,
    status TEXT NOT NULL,
    latency_ms BIGINT,
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#,
    },
    Migration {
        version: "0002_indexes",
        sql: r#"
CREATE INDEX IF NOT EXISTS idx_quotes_latest_as_of ON quotes_latest(as_of);
CREATE INDEX IF NOT EXISTS idx_bars_1m_symbol_ts ON bars_1m(symbol, ts);
CREATE INDEX IF NOT EXISTS idx_bars_1d_symbol_ts ON bars_1d(symbol, ts);
CREATE INDEX IF NOT EXISTS idx_fundamentals_symbol_date ON fundamentals(symbol, date);
CREATE INDEX IF NOT EXISTS idx_cache_manifest_dataset_symbol ON cache_manifest(dataset, symbol);
CREATE INDEX IF NOT EXISTS idx_ingest_log_source_dataset_ts ON ingest_log(source, dataset, timestamp);
"#,
    },
];

pub fn apply_migrations(connection: &Connection) -> Result<(), ::duckdb::Error> {
    connection.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#,
    )?;

    for migration in MIGRATIONS {
        let query = format!(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = '{}'",
            escape_sql_string(migration.version)
        );
        let applied_count: i64 = connection.query_row(query.as_str(), [], |row| row.get(0))?;

        if applied_count == 0 {
            connection.execute_batch(migration.sql)?;
            let insert = format!(
                "INSERT INTO schema_migrations (version) VALUES ('{}')",
                escape_sql_string(migration.version)
            );
            connection.execute_batch(insert.as_str())?;
        }
    }

    Ok(())
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

