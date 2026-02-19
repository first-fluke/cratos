use super::types::ApprovalRequest;

/// Trait for approval callbacks
#[async_trait::async_trait]
pub trait ApprovalCallback: Send + Sync {
    /// Called when an approval is needed
    async fn request_approval(&self, request: &ApprovalRequest) -> crate::Result<()>;

    /// Called when an approval is resolved
    async fn notify_resolution(&self, request: &ApprovalRequest) -> crate::Result<()>;
}
