//! Git tools - Git repository operations
//!
//! This module provides tools for common git operations:
//! - `GitStatusTool` - Get repository status
//! - `GitCommitTool` - Create commits
//! - `GitBranchTool` - Manage branches
//! - `GitDiffTool` - Show diffs
//! - `GitPushTool` - Push to remote
//! - `GitCloneTool` - Clone repositories
//! - `GitLogTool` - View commit history
//!
//! All tools include security validations to prevent command injection.

mod branch;
mod clone;
mod commit;
mod diff;
mod log;
mod push;
pub mod security;
mod status;

#[cfg(test)]
mod tests;

// Re-export types
pub use branch::GitBranchTool;
pub use clone::GitCloneTool;
pub use commit::GitCommitTool;
pub use diff::GitDiffTool;
pub use log::GitLogTool;
pub use push::GitPushTool;
pub use status::GitStatusTool;

// Re-export RiskLevel and ToolCategory for tests
#[cfg(test)]
pub use crate::registry::{RiskLevel, ToolCategory};
