//! `DuckDB` connection pool management.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ::duckdb::Connection;

/// Access mode for database connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// Read-only access.
    ReadOnly,
    /// Read-write access.
    ReadWrite,
}

struct PoolState {
    read_only: Vec<Connection>,
    read_write: Vec<Connection>,
}

impl PoolState {
    fn new() -> Self {
        Self {
            read_only: Vec::new(),
            read_write: Vec::new(),
        }
    }
}

struct PoolInner {
    db_path: PathBuf,
    max_pool_size: usize,
    state: Mutex<PoolState>,
}

/// A connection pool manager for `DuckDB` connections.
#[derive(Clone)]
pub struct DuckDbConnectionManager {
    inner: Arc<PoolInner>,
}

impl DuckDbConnectionManager {
    /// Create a new connection pool manager.
    ///
    /// # Arguments
    /// * `path` - Path to the `DuckDB` database file
    /// * `max_pool_size` - Maximum number of connections to keep in the pool
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, max_pool_size: usize) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                db_path: path.into(),
                max_pool_size: max_pool_size.max(1),
                state: Mutex::new(PoolState::new()),
            }),
        }
    }

    /// Acquire a connection from the pool.
    ///
    /// # Arguments
    /// * `mode` - Access mode for the connection
    ///
    /// # Errors
    /// Returns an error if:
    /// - The database file cannot be opened
    /// - Connection configuration fails
    ///
    /// # Panics
    /// Panics if the connection pool mutex is poisoned (indicating a previous panic
    /// while holding the lock).
    pub fn acquire(&self, mode: AccessMode) -> Result<PooledConnection, ::duckdb::Error> {
        let mut state = self
            .inner
            .state
            .lock()
            .expect("duckdb connection pool mutex poisoned");
        let connection = match mode {
            AccessMode::ReadOnly => state.read_only.pop(),
            AccessMode::ReadWrite => state.read_write.pop(),
        };
        drop(state);

        let connection = match connection {
            Some(connection) => connection,
            None => open_connection(self.inner.db_path.as_path(), mode)?,
        };

        Ok(PooledConnection {
            mode,
            pool: Arc::clone(&self.inner),
            connection: Some(connection),
        })
    }

    /// Get the path to the database file.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        self.inner.db_path.as_path()
    }
}

/// A pooled connection that returns to the pool when dropped.
pub struct PooledConnection {
    mode: AccessMode,
    pool: Arc<PoolInner>,
    connection: Option<Connection>,
}

impl Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection
            .as_ref()
            .expect("pooled connection unexpectedly missing")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection
            .as_mut()
            .expect("pooled connection unexpectedly missing")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        let Some(connection) = self.connection.take() else {
            return;
        };

        let mut state = self
            .pool
            .state
            .lock()
            .expect("duckdb connection pool mutex poisoned");
        match self.mode {
            AccessMode::ReadOnly => {
                if state.read_only.len() < self.pool.max_pool_size {
                    state.read_only.push(connection);
                }
            }
            AccessMode::ReadWrite => {
                if state.read_write.len() < self.pool.max_pool_size {
                    state.read_write.push(connection);
                }
            }
        }
    }
}

/// Open a new database connection.
///
/// # Errors
/// Returns an error if the database file cannot be opened or configured.
fn open_connection(path: &Path, mode: AccessMode) -> Result<Connection, ::duckdb::Error> {
    let connection = Connection::open(path)?;
    configure_connection(&connection, mode)?;
    Ok(connection)
}

/// Configure a database connection with appropriate settings.
///
/// # Errors
/// Returns an error if configuration SQL fails to execute.
fn configure_connection(connection: &Connection, mode: AccessMode) -> Result<(), ::duckdb::Error> {
    connection.execute_batch("PRAGMA disable_progress_bar;")?;
    if mode == AccessMode::ReadOnly {
        // This statement can fail on older embedded versions; guardrails in the query layer
        // still enforce read-only semantics.
        let _ = connection.execute_batch("SET access_mode = 'READ_ONLY';");
    }
    Ok(())
}
