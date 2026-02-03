//! Cratos Replay - Replay Engine
//!
//! This crate provides the replay/audit system for Cratos:
//! - Event: Event types and schemas
//! - Store: Event persistence (SQLite)
//! - Viewer: Event query and replay API
//! - Search: Semantic search over execution history (feature: search)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event;
#[cfg(feature = "search")]
pub mod search;
pub mod store;
pub mod viewer;

pub use error::{Error, Result};
pub use event::{Event, EventType, Execution, ExecutionStatus, TimelineEntry};
pub use store::{
    default_data_dir, default_db_path, EventRecorder, EventStore, EventStoreTrait, ExecutionQuery,
};
pub use viewer::{
    ExecutionDetail, ExecutionStats, ExecutionSummary, ExecutionViewer, ReplayOptions,
};

// Re-export search types when feature is enabled
#[cfg(feature = "search")]
pub use search::{
    create_execution_index, ExecutionSearcher, ExecutionSearchResult, SearchEmbedder,
    SearcherConfig,
};
