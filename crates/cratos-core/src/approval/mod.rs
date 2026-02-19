//! Approval - User approval flow handling
//!
//! This module provides the approval system for high-risk operations.
//! When a tool or action requires user confirmation, this system
//! handles the approval workflow.

use std::sync::Arc;

pub mod manager;
pub mod traits;
pub mod types;

pub use manager::ApprovalManager;
pub use traits::ApprovalCallback;
pub use types::{ApprovalError, ApprovalRequest, ApprovalStatus};

/// Shared approval manager type
pub type SharedApprovalManager = Arc<ApprovalManager>;

#[cfg(test)]
mod tests;
