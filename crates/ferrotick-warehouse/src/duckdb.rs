use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ::duckdb::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
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

#[derive(Clone)]
pub struct DuckDbConnectionManager {
    inner: Arc<PoolInner>,
}

impl DuckDbConnectionManager {
    pub fn new(path: impl Into<PathBuf>, max_pool_size: usize) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                db_path: path.into(),
                max_pool_size: max_pool_size.max(1),
                state: Mutex::new(PoolState::new()),
            }),
        }
    }

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

    pub fn db_path(&self) -> &Path {
        self.inner.db_path.as_path()
    }
}

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

fn open_connection(path: &Path, mode: AccessMode) -> Result<Connection, ::duckdb::Error> {
    let connection = Connection::open(path)?;
    configure_connection(&connection, mode)?;
    Ok(connection)
}

fn configure_connection(connection: &Connection, mode: AccessMode) -> Result<(), ::duckdb::Error> {
    connection.execute_batch("PRAGMA disable_progress_bar;")?;
    if mode == AccessMode::ReadOnly {
        // This statement can fail on older embedded versions; guardrails in the query layer
        // still enforce read-only semantics.
        let _ = connection.execute_batch("SET access_mode = 'READ_ONLY';");
    }
    Ok(())
}
