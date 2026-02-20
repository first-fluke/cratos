//! Scheduler task storage using SQLite
//!
//! Persists scheduled tasks for durability across restarts.

mod migrations;
mod queries;

#[cfg(test)]
mod tests;

use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::Path;

use crate::scheduler::types::{Result, SchedulerError};

/// SQLite-based scheduler store
pub struct SchedulerStore {
    pub(super) pool: Pool<Sqlite>,
}

impl SchedulerStore {
    /// Create a new store from database path
    pub async fn from_path(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SchedulerError::InvalidConfig(format!("Failed to create directory: {}", e))
            })?;
        }

        let url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }
}
