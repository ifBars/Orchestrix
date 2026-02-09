//! Database layer for persistent storage.
//!
//! This module provides:
//! - SQLite database management with connection pooling
//! - Schema migrations for versioned upgrades
//! - Query functions for all data operations
//! - Error types for database failures
//!
//! # Schema
//!
//! Core tables:
//! - `tasks`: Top-level work items
//! - `runs`: Execution instances of tasks
//! - `sub_agents`: Parallel execution units
//! - `tool_calls`: Audit log of all tool invocations
//! - `events`: Event sourcing for real-time updates
//! - `artifacts`: Generated files and outputs
//!
//! See `migrations.rs` for complete schema definition.

mod migrations;
pub mod queries;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration failed: {0}")]
    Migration(String),
    #[error("not found: {0}")]
    NotFound(String),
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open (or create) a database file at `path`, enable WAL mode, and run migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    /// Open an in-memory database for tests.
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, DbError> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Acquire a lock on the connection for queries.
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }
}
