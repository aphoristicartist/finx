use thiserror::Error;

#[derive(Error, Debug)]
pub enum WarehouseError {
    #[error("Database connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Record not found: {0}")]
    RecordNotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}