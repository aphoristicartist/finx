use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

use super::error::WarehouseError;
use super::models::{BarRecord, QuoteRecord, SqlColumn};

#[derive(Debug, Clone)]
pub struct WarehouseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct QueryGuardrails {
    pub max_results: usize,
    pub time_range_days: i64,
}

#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    pub data: Vec<T>,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct CacheSyncReport {
    pub symbols_synced: usize,
    pub bars_synced: usize,
    pub errors: Vec<String>,
}

pub struct Warehouse {
    pool: SqlitePool,
}

impl Warehouse {
    pub async fn new(config: WarehouseConfig) -> std::result::Result<Self, WarehouseError> {
        let pool = SqlitePool::connect(&config.url).await.map_err(|e| {
            WarehouseError::ConnectionError(e.to_string())
        })?;

        // Initialize database schema
        Self::initialize_schema(&pool).await?;

        Ok(Self { pool })
    }

    async fn initialize_schema(pool: &SqlitePool) -> std::result::Result<(), WarehouseError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bar_records (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume INTEGER
            )
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS quote_records (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                price REAL NOT NULL,
                timestamp TEXT NOT NULL,
                volume INTEGER
            )
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS fundamental_records (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                data TEXT NOT NULL
            )
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_bar_records(
        &self,
        symbol: &str,
        limit: Option<usize>,
    ) -> std::result::Result<Vec<BarRecord>, WarehouseError> {
        let limit = limit.unwrap_or(100) as i32;
        let query = sqlx::query_as::<_, BarRecord>(
            "SELECT * FROM bar_records WHERE symbol = ? ORDER BY timestamp DESC LIMIT ?"
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(query)
    }

    pub async fn get_quote_records(
        &self,
        symbol: &str,
        limit: Option<usize>,
    ) -> std::result::Result<Vec<QuoteRecord>, WarehouseError> {
        let limit = limit.unwrap_or(100) as i32;
        let query = sqlx::query_as::<_, QuoteRecord>(
            "SELECT * FROM quote_records WHERE symbol = ? ORDER BY timestamp DESC LIMIT ?"
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(query)
    }

    pub async fn upsert_bar_record(&self, record: BarRecord) -> std::result::Result<(), WarehouseError> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO bar_records (id, symbol, timestamp, open, high, low, close, volume)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(record.id)
        .bind(&record.symbol)
        .bind(record.timestamp)
        .bind(record.open)
        .bind(record.high)
        .bind(record.low)
        .bind(record.close)
        .bind(record.volume)
        .execute(&self.pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(())
    }

    pub async fn upsert_quote_record(&self, record: QuoteRecord) -> std::result::Result<(), WarehouseError> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO quote_records (id, symbol, price, timestamp, volume)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(record.id)
        .bind(&record.symbol)
        .bind(record.price)
        .bind(record.timestamp)
        .bind(record.volume)
        .execute(&self.pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(())
    }

    pub async fn get_schema(&self) -> std::result::Result<Vec<SqlColumn>, WarehouseError> {
        let columns = sqlx::query_as::<_, SqlColumn>(
            "SELECT name, type FROM pragma_table_info('bar_records')"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| WarehouseError::QueryError(e.to_string()))?;

        Ok(columns)
    }
}

impl Warehouse {
    pub async fn sync_quotes(&self, symbol: &str) -> std::result::Result<CacheSyncReport, WarehouseError> {
        let mut report = CacheSyncReport {
            symbols_synced: 0,
            bars_synced: 0,
            errors: Vec::new(),
        };

        let records = self.get_quote_records(symbol, None).await?;

        for record in records {
            if let Err(e) = self.upsert_quote_record(record).await {
                report.errors.push(format!("Failed to upsert quote for {}: {}", symbol, e));
            } else {
                report.bars_synced += 1;
            }
        }

        report.symbols_synced = 1;
        Ok(report)
    }
}