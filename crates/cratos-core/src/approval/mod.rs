//! Approval - User approval flow handling
//!
//! This module provides the approval system for high-risk operations.
//! When a tool or action requires user confirmation, this system
//! handles the approval workflow.

use std::sync::Arc;

/// Approval manager implementation and request lifecycle.
pub mod manager;
/// Approval callback trait for channel-specific approval UIs.
pub mod traits;
/// Approval request/response types and error definitions.
pub mod types;

pub use manager::ApprovalManager;
pub use traits::ApprovalCallback;
pub use types::{ApprovalError, ApprovalRequest, ApprovalStatus};

/// Shared approval manager type
pub type SharedApprovalManager = Arc<ApprovalManager>;

#[cfg(test)]
mod tests;
