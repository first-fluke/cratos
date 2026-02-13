//! Trait for event storage backends

use crate::error::Result;
use crate::event::{Event, Execution};
use uuid::Uuid;

/// Trait for event storage backends
///
/// This trait allows different storage implementations (SQLite, in-memory, etc.)
/// to be used interchangeably.
#[async_trait::async_trait]
pub trait EventStoreTrait: Send + Sync {
    /// Create an execution record (must be called before appending events)
    async fn create_execution(&self, execution: &Execution) -> Result<()>;

    /// Append an event to the store
    async fn append(&self, event: Event) -> Result<()>;

    /// Get events for an execution
    async fn get_events(&self, execution_id: Uuid) -> Result<Vec<Event>>;

    /// Update execution status and output
    async fn update_execution_status(
        &self,
        id: Uuid,
        status: &str,
        output_text: Option<&str>,
    ) -> Result<()>;

    /// Get the event store name (for logging)
    fn name(&self) -> &str;
}
