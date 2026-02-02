//! Cratos Replay - Replay Engine
//!
//! This crate provides the replay/audit system for Cratos:
//! - Event: Event types and schemas
//! - Store: Event persistence (PostgreSQL)
//! - Viewer: Event query and replay API

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod event;
pub mod store;
pub mod viewer;

pub use error::{Error, Result};
pub use event::{Event, EventType, Execution, ExecutionStatus, TimelineEntry};
pub use store::{EventRecorder, EventStore, EventStoreTrait, ExecutionQuery};
pub use viewer::{
    ExecutionDetail, ExecutionStats, ExecutionSummary, ExecutionViewer, ReplayOptions,
};
