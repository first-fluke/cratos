use cratos_core::Orchestrator;
use serenity::all::CreateEmbed;
use std::sync::Arc;
use uuid::Uuid;

/// Discord Slash Command Handlers
pub struct DiscordCommands {
    orchestrator: Arc<Orchestrator>,
}

impl DiscordCommands {
    pub fn new(orchestrator: Arc<Orchestrator>) -> Self {
        Self { orchestrator }
    }

    /// Build a rich embed for the /status command
    pub fn handle_status(&self) -> CreateEmbed {
        let count = self.orchestrator.active_execution_count().unwrap_or(0);
        let provider = self.orchestrator.provider_name().to_string();
        let color = if count == 0 { 0x00ff00 } else { 0xffaa00 };
        CreateEmbed::new()
            .title("Cratos Status")
            .field("Active Executions", count.to_string(), true)
            .field("Provider", provider, true)
            .color(color)
    }

    pub fn handle_sessions(&self) -> String {
        let count = self.orchestrator.active_execution_count().unwrap_or(0);
        if count == 0 {
            "No active sessions.".to_string()
        } else {
            format!("{} active session(s) running.", count)
        }
    }

    pub fn handle_tools(&self) -> String {
        let tools = self.orchestrator.list_tool_names();
        if tools.is_empty() {
            "No tools available.".to_string()
        } else {
            format!(
                "**Available tools ({}):**\n{}",
                tools.len(),
                tools.join(", ")
            )
        }
    }

    pub fn handle_cancel(&self, id: &str) -> String {
        if id.is_empty() {
            return "Please provide an execution ID.".to_string();
        }
        match Uuid::parse_str(id) {
            Ok(execution_id) => {
                if self.orchestrator.cancel_execution(execution_id) {
                    format!("Execution `{}` cancelled.", id)
                } else {
                    format!("Execution `{}` not found or already completed.", id)
                }
            }
            Err(_) => "Invalid execution ID format.".to_string(),
        }
    }

    pub async fn handle_approve(&self, id: &str) -> String {
        if id.is_empty() {
            return "Please provide a request ID.".to_string();
        }
        let request_id = match Uuid::parse_str(id) {
            Ok(uid) => uid,
            Err(_) => return "Invalid request ID format.".to_string(),
        };
        match self.orchestrator.approval_manager() {
            Some(mgr) => {
                if mgr.approve_by(request_id, "discord").await.is_some() {
                    format!("Approval request `{}` approved.", id)
                } else {
                    format!("Failed to approve `{}` (not found or invalid state).", id)
                }
            }
            None => "Approval manager not configured.".to_string(),
        }
    }

    pub async fn handle_deny(&self, id: &str) -> String {
        if id.is_empty() {
            return "Please provide a request ID.".to_string();
        }
        let request_id = match Uuid::parse_str(id) {
            Ok(uid) => uid,
            Err(_) => return "Invalid request ID format.".to_string(),
        };
        match self.orchestrator.approval_manager() {
            Some(mgr) => {
                if mgr.reject_by(request_id, "discord").await.is_some() {
                    format!("Approval request `{}` denied.", id)
                } else {
                    format!("Failed to deny `{}` (not found or invalid state).", id)
                }
            }
            None => "Approval manager not configured.".to_string(),
        }
    }
}
