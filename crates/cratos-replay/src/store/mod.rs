//! Store - Event persistence using SQLite
//!
//! This module provides the storage layer for executions and events.
//! It uses sqlx for async SQLite access (embedded, no Docker required).

mod event_store;
mod events;
mod helpers;
mod query;
mod recorder;
mod traits;

#[cfg(test)]
mod tests;

pub use event_store::EventStore;
pub use events::*;
pub use helpers::{default_data_dir, default_db_path};
pub use query::ExecutionQuery;
pub use recorder::EventRecorder;
pub use traits::EventStoreTrait;
