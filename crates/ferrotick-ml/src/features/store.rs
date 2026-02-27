use std::fs;
use std::path::Path;

use duckdb::params;
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_warehouse::{AccessMode, QueryGuardrails, Warehouse};
use polars::prelude::*;

use crate::features::FeatureRow;
use crate::{MlError, MlResult};

const CREATE_FEATURES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS features (
    symbol VARCHAR,
    timestamp TIMESTAMP,
    rsi DOUBLE,
    macd DOUBLE,
    macd_signal DOUBLE,
    bb_upper DOUBLE,
    bb_lower DOUBLE,
    atr DOUBLE,
    return_1d DOUBLE,
    return_5d DOUBLE,
    return_20d DOUBLE,
    rolling_mean_20 DOUBLE,
    rolling_std_20 DOUBLE,
    lag_1 DOUBLE,
    lag_2 DOUBLE,
    lag_3 DOUBLE,
    rolling_momentum DOUBLE,
    PRIMARY KEY (symbol, timestamp)
);
"#;

const INSERT_FEATURE_SQL: &str = r#"
INSERT OR REPLACE INTO features
(symbol, timestamp, rsi, macd, macd_signal, bb_upper, bb_lower, atr,
 return_1d, return_5d, return_20d, rolling_mean_20, rolling_std_20,
 lag_1, lag_2, lag_3, rolling_momentum)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
"#;

pub struct FeatureStore {
    warehouse: Warehouse,
}

impl FeatureStore {
    pub fn open_default() -> MlResult<Self> {
        Ok(Self {
            warehouse: Warehouse::open_default()?,
        })
    }

    pub const fn new(warehouse: Warehouse) -> Self {
        Self { warehouse }
    }

    pub fn ensure_table(&self) -> MlResult<()> {
        self.warehouse.execute_query(
            CREATE_FEATURES_TABLE_SQL,
            QueryGuardrails {
                max_rows: 1,
                query_timeout_ms: 30_000,
            },
            true,
        )?;
        Ok(())
    }

    pub fn load_daily_bars(
        &self,
        symbol: &Symbol,
        start: Option<UtcDateTime>,
        end: Option<UtcDateTime>,
    ) -> MlResult<Vec<Bar>> {
        // SECURITY: Use parameterized query instead of string interpolation
        let sql = r#"
            SELECT strftime(ts, '%Y-%m-%dT%H:%M:%SZ') AS ts, open, high, low, close, volume
            FROM bars_1d
            WHERE symbol = ? AND (? IS NULL OR ts >= TRY_CAST(? AS TIMESTAMP)) AND (? IS NULL OR ts <= TRY_CAST(? AS TIMESTAMP))
            ORDER BY ts ASC
        "#;

        let connection = self.warehouse.acquire_connection(AccessMode::ReadOnly)
            .map_err(|e| MlError::Store(e.to_string()))?;

        let mut stmt = connection.prepare(sql)
            .map_err(|e| MlError::Store(e.to_string()))?;

        let start_str = start.as_ref().map(|s| s.format_rfc3339());
        let end_str = end.as_ref().map(|e| e.format_rfc3339());

        let rows = stmt.query_map(params![
            symbol.as_str(),
            &start_str,
            &start_str,
            &end_str,
            &end_str,
        ], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, Option<u64>>(5)?,
            ))
        })
        .map_err(|e| MlError::Store(e.to_string()))?;

        let mut bars = Vec::new();
        for row_result in rows {
            let (ts, open, high, low, close, volume) = row_result
                .map_err(|e| MlError::Store(e.to_string()))?;

            bars.push(Bar::new(
                UtcDateTime::parse(&ts)?,
                open,
                high,
                low,
                close,
                volume,
                None,
            )?);
        }

        Ok(bars)
    }

    pub fn upsert_features(&self, rows: &[FeatureRow]) -> MlResult<usize> {
        if rows.is_empty() {
            return Ok(0);
        }

        self.ensure_table()?;

        // SECURITY: Use parameterized queries and batch transaction for performance
        let connection = self.warehouse.acquire_connection(AccessMode::ReadWrite)
            .map_err(|e| MlError::Store(e.to_string()))?;

        connection.execute_batch("BEGIN TRANSACTION")
            .map_err(|e| MlError::Store(e.to_string()))?;

        let result = (|| -> Result<(), duckdb::Error> {
            let mut stmt = connection.prepare(INSERT_FEATURE_SQL)?;

            for row in rows {
                stmt.execute(params![
                    &row.symbol,
                    &row.timestamp,
                    &row.rsi,
                    &row.macd,
                    &row.macd_signal,
                    &row.bb_upper,
                    &row.bb_lower,
                    &row.atr,
                    &row.return_1d,
                    &row.return_5d,
                    &row.return_20d,
                    &row.rolling_mean_20,
                    &row.rolling_std_20,
                    &row.lag_1,
                    &row.lag_2,
                    &row.lag_3,
                    &row.rolling_momentum,
                ])?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                connection.execute_batch("COMMIT")
                    .map_err(|e| MlError::Store(e.to_string()))?;
                Ok(rows.len())
            }
            Err(e) => {
                connection.execute_batch("ROLLBACK")
                    .unwrap_or_else(|rollback_err| eprintln!("rollback failed: {}", rollback_err));
                Err(MlError::Store(e.to_string()))
            }
        }
    }

    pub fn load_features(
        &self,
        symbol: &str,
        start: Option<UtcDateTime>,
        end: Option<UtcDateTime>,
    ) -> MlResult<Vec<FeatureRow>> {
        let parsed_symbol = Symbol::parse(symbol)?;

        // SECURITY: Use parameterized query instead of string interpolation
        let sql = r#"
            SELECT symbol, strftime(timestamp, '%Y-%m-%dT%H:%M:%SZ') AS timestamp,
                   rsi, macd, macd_signal, bb_upper, bb_lower, atr,
                   return_1d, return_5d, return_20d, rolling_mean_20, rolling_std_20,
                   lag_1, lag_2, lag_3, rolling_momentum
            FROM features
            WHERE symbol = ? AND (? IS NULL OR timestamp >= TRY_CAST(? AS TIMESTAMP)) AND (? IS NULL OR timestamp <= TRY_CAST(? AS TIMESTAMP))
            ORDER BY timestamp ASC
        "#;

        let connection = self.warehouse.acquire_connection(AccessMode::ReadOnly)
            .map_err(|e| MlError::Store(e.to_string()))?;

        let mut stmt = connection.prepare(sql)
            .map_err(|e| MlError::Store(e.to_string()))?;

        let start_str = start.as_ref().map(|s| s.format_rfc3339());
        let end_str = end.as_ref().map(|e| e.format_rfc3339());

        let rows = stmt.query_map(params![
            parsed_symbol.as_str(),
            &start_str,
            &start_str,
            &end_str,
            &end_str,
        ], |row| {
            Ok(FeatureRow {
                symbol: row.get(0)?,
                timestamp: row.get(1)?,
                rsi: row.get(2)?,
                macd: row.get(3)?,
                macd_signal: row.get(4)?,
                bb_upper: row.get(5)?,
                bb_lower: row.get(6)?,
                atr: row.get(7)?,
                return_1d: row.get(8)?,
                return_5d: row.get(9)?,
                return_20d: row.get(10)?,
                rolling_mean_20: row.get(11)?,
                rolling_std_20: row.get(12)?,
                lag_1: row.get(13)?,
                lag_2: row.get(14)?,
                lag_3: row.get(15)?,
                rolling_momentum: row.get(16)?,
            })
        })
        .map_err(|e| MlError::Store(e.to_string()))?;

        let mut result = Vec::new();
        for row_result in rows {
            result.push(row_result.map_err(|e| MlError::Store(e.to_string()))?);
        }

        Ok(result)
    }

    pub async fn export_features_parquet(
        &self,
        symbol: &str,
        start: UtcDateTime,
        end: UtcDateTime,
        path: &Path,
    ) -> MlResult<()> {
        let rows = self.load_features(symbol, Some(start), Some(end))?;
        if rows.is_empty() {
            return Err(MlError::NoData(format!(
                "no feature rows found for symbol={} between {} and {}",
                symbol,
                start,
                end
            )));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut df = DataFrame::new(vec![
            Series::new("symbol", rows.iter().map(|row| row.symbol.clone()).collect::<Vec<_>>()),
            Series::new("timestamp", rows.iter().map(|row| row.timestamp.clone()).collect::<Vec<_>>()),
            Series::new("rsi", rows.iter().map(|row| row.rsi).collect::<Vec<_>>()),
            Series::new("macd", rows.iter().map(|row| row.macd).collect::<Vec<_>>()),
            Series::new("macd_signal", rows.iter().map(|row| row.macd_signal).collect::<Vec<_>>()),
            Series::new("bb_upper", rows.iter().map(|row| row.bb_upper).collect::<Vec<_>>()),
            Series::new("bb_lower", rows.iter().map(|row| row.bb_lower).collect::<Vec<_>>()),
            Series::new("atr", rows.iter().map(|row| row.atr).collect::<Vec<_>>()),
            Series::new("return_1d", rows.iter().map(|row| row.return_1d).collect::<Vec<_>>()),
            Series::new("return_5d", rows.iter().map(|row| row.return_5d).collect::<Vec<_>>()),
            Series::new("return_20d", rows.iter().map(|row| row.return_20d).collect::<Vec<_>>()),
            Series::new("rolling_mean_20", rows.iter().map(|row| row.rolling_mean_20).collect::<Vec<_>>()),
            Series::new("rolling_std_20", rows.iter().map(|row| row.rolling_std_20).collect::<Vec<_>>()),
            Series::new("lag_1", rows.iter().map(|row| row.lag_1).collect::<Vec<_>>()),
            Series::new("lag_2", rows.iter().map(|row| row.lag_2).collect::<Vec<_>>()),
            Series::new("lag_3", rows.iter().map(|row| row.lag_3).collect::<Vec<_>>()),
            Series::new("rolling_momentum", rows.iter().map(|row| row.rolling_momentum).collect::<Vec<_>>()),
        ])?;

        let file = std::fs::File::create(path)?;
        ParquetWriter::new(file)
            .with_compression(ParquetCompression::Snappy)
            .finish(&mut df)?;

        Ok(())
    }
}
