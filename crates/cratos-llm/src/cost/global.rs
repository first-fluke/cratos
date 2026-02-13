//! Global Cost Tracker
//!
//! This module provides a global singleton cost tracker instance.

use super::tracker::CostTracker;
use std::sync::Arc;

lazy_static::lazy_static! {
    /// Global cost tracker instance
    static ref GLOBAL_TRACKER: Arc<CostTracker> = Arc::new(CostTracker::new());
}

/// Get the global cost tracker
#[must_use]
pub fn global_tracker() -> Arc<CostTracker> {
    Arc::clone(&GLOBAL_TRACKER)
}
