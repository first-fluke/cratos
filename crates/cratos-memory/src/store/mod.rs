//! GraphStore — SQLite persistence for the entity-turn graph.

use crate::error::{Error, Result};

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::{debug, info};

mod crud;
mod migrations;
mod query;

#[cfg(test)]
mod tests;

/// SQLite-backed graph store.
#[derive(Clone)]
pub struct GraphStore {
    pub(crate) pool: SqlitePool,
}

impl GraphStore {
    /// Open (or create) a graph store at the given path.
    pub async fn from_path(db_path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Internal(format!("mkdir: {e}")))?;
        }
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        // Enable WAL for read/write concurrency
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;

        let store = Self { pool };
        store.run_migrations().await?;
        info!("Graph store initialized at {}", db_path.display());
        Ok(store)
    }

    /// In-memory store (for tests).
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.run_migrations().await?;
        debug!("In-memory graph store initialized");
        Ok(store)
    }
}
