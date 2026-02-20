use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BarRecord {
    pub id: String,
    pub symbol: String,
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QuoteRecord {
    pub id: String,
    pub symbol: String,
    pub price: f64,
    pub timestamp: String,
    pub volume: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FundamentalRecord {
    pub id: String,
    pub symbol: String,
    pub timestamp: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SqlColumn {
    pub name: String,
    pub data_type: String,
}