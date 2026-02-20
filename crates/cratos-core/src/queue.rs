//! Execution Queue Modes
//!
//! Controls how concurrent chat.send requests are handled:
//! - Sequential: One at a time, queued
//! - Concurrent: All run in parallel
//! - Collect: Debounce messages then batch-process

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::debug;

/// Queue mode for execution requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueMode {
    /// Process one request at a time (default, safe)
    Sequential,
    /// Allow concurrent execution (faster, more resource usage)
    Concurrent,
    /// Collect messages for a window then batch-process
    Collect,
}

impl Default for QueueMode {
    fn default() -> Self {
        Self::Sequential
    }
}

/// Configuration for the execution queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Queue mode
    #[serde(default)]
    pub mode: QueueMode,
    /// Maximum concurrent executions (for Concurrent mode)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Collect window duration in milliseconds (for Collect mode)
    #[serde(default = "default_collect_window_ms")]
    pub collect_window_ms: u64,
}

fn default_max_concurrent() -> usize {
    3
}

fn default_collect_window_ms() -> u64 {
    2000
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            mode: QueueMode::Sequential,
            max_concurrent: default_max_concurrent(),
            collect_window_ms: default_collect_window_ms(),
        }
    }
}

/// Execution queue that enforces the configured mode.
pub struct ExecutionQueue {
    config: QueueConfig,
    semaphore: Arc<Semaphore>,
}

impl ExecutionQueue {
    /// Create a new execution queue.
    pub fn new(config: QueueConfig) -> Self {
        let permits = match config.mode {
            QueueMode::Sequential => 1,
            QueueMode::Concurrent => config.max_concurrent,
            QueueMode::Collect => 1, // Collect mode processes batches sequentially
        };
        Self {
            config,
            semaphore: Arc::new(Semaphore::new(permits)),
        }
    }

    /// Acquire a permit to execute. Blocks until a slot is available.
    pub async fn acquire(&self) -> QueuePermit {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        debug!(mode = ?self.config.mode, "Execution slot acquired");
        QueuePermit { _permit: permit }
    }

    /// Get the current queue mode.
    pub fn mode(&self) -> QueueMode {
        self.config.mode
    }

    /// Get the collect window duration (only relevant for Collect mode).
    pub fn collect_window(&self) -> Duration {
        Duration::from_millis(self.config.collect_window_ms)
    }

    /// Get the number of available execution slots.
    pub fn available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// A permit that releases the execution slot when dropped.
pub struct QueuePermit {
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests;

