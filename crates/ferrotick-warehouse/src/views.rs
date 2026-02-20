//! Database views for analytical queries.

use ::duckdb::Connection;

/// Create database views for common analytical queries.
///
/// Creates the following views:
/// - `vw_returns_daily`: Daily return percentages per symbol
/// - `vw_volatility_20d`: 20-day rolling volatility
/// - `vw_gaps_open`: Gap percentages between close and open
/// - `vw_source_latency`: Average latency by source and dataset
///
/// # Errors
/// Returns an error if the view creation SQL fails to execute.
pub fn create_views(connection: &Connection) -> Result<(), ::duckdb::Error> {
    connection.execute_batch(
        r"
CREATE OR REPLACE VIEW vw_returns_daily AS
SELECT
    symbol,
    CAST(ts AS DATE) AS date,
    CASE
        WHEN LAG(close) OVER (PARTITION BY symbol ORDER BY ts) IS NULL THEN NULL
        WHEN LAG(close) OVER (PARTITION BY symbol ORDER BY ts) = 0 THEN NULL
        ELSE (close / LAG(close) OVER (PARTITION BY symbol ORDER BY ts)) - 1.0
    END AS return_pct
FROM bars_1d;

CREATE OR REPLACE VIEW vw_volatility_20d AS
SELECT
    symbol,
    date,
    STDDEV_SAMP(return_pct) OVER (
        PARTITION BY symbol
        ORDER BY date
        ROWS BETWEEN 19 PRECEDING AND CURRENT ROW
    ) AS volatility
FROM vw_returns_daily
WHERE return_pct IS NOT NULL;

CREATE OR REPLACE VIEW vw_gaps_open AS
SELECT
    symbol,
    CAST(ts AS DATE) AS open_date,
    CAST(LAG(ts) OVER (PARTITION BY symbol ORDER BY ts) AS DATE) AS close_date,
    CASE
        WHEN LAG(close) OVER (PARTITION BY symbol ORDER BY ts) IS NULL THEN NULL
        WHEN LAG(close) OVER (PARTITION BY symbol ORDER BY ts) = 0 THEN NULL
        ELSE (open / LAG(close) OVER (PARTITION BY symbol ORDER BY ts)) - 1.0
    END AS gap_pct
FROM bars_1d;

CREATE OR REPLACE VIEW vw_source_latency AS
SELECT
    source,
    dataset,
    AVG(latency_ms)::DOUBLE AS avg_latency_ms
FROM ingest_log
WHERE latency_ms IS NOT NULL
GROUP BY source, dataset;
",
    )?;

    Ok(())
}
