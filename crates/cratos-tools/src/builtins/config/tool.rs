use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::path::PathBuf;
use std::time::Instant;
use tracing::debug;

use super::handler;
use super::types::{ConfigAction, ConfigInput, ConfigTarget};
use super::wol;
use crate::builtins::config_manager::ConfigManager;

/// Configuration tool for Cratos settings
pub struct ConfigTool {
    pub(crate) definition: ToolDefinition,
    pub(crate) data_dir: PathBuf,
}

impl ConfigTool {
    /// Create a new config tool
    #[must_use]
    pub fn new() -> Self {
        Self::with_data_dir(Self::default_data_dir())
    }

    /// Create with custom data directory
    #[must_use]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let description = "Cratos 설정 변경: LLM 모델, 언어, WoL 디바이스, 스케줄러 등";

        let definition = ToolDefinition::new("config", description)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["set", "get", "list", "delete"],
                        "description": "set=변경, get=조회, list=목록, delete=삭제"
                    },
                    "target": {
                        "type": "string",
                        "enum": ["llm_provider", "llm_model", "channel", "persona", "language", "theme", "wol_device", "scheduler"],
                        "description": "설정 대상"
                    },
                    "value": {
                        "type": "string",
                        "description": "새 값 (예: claude-sonnet-4, ko, sindri)"
                    },
                    "device_name": {
                        "type": "string",
                        "description": "WoL 디바이스 이름 (사용자가 부르는 이름)"
                    },
                    "mac_address": {
                        "type": "string",
                        "description": "MAC 주소 (AA:BB:CC:DD:EE:FF 형식)"
                    }
                },
                "required": ["action", "target"]
            }))
            .with_risk_level(RiskLevel::Medium)
            .with_category(ToolCategory::Utility);

        Self {
            definition,
            data_dir,
        }
    }

    /// Get default data directory
    fn default_data_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".cratos"))
            .unwrap_or_else(|| PathBuf::from(".cratos"))
    }

    /// Get ConfigManager instance
    fn config_manager(&self) -> Result<ConfigManager> {
        ConfigManager::new(&self.data_dir)
    }
}

impl Default for ConfigTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for ConfigTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        let params: ConfigInput = serde_json::from_value(input)
            .map_err(|e| Error::InvalidInput(format!("Invalid config parameters: {}", e)))?;

        debug!(
            action = ?params.action,
            target = ?params.target,
            value = ?params.value,
            device_name = ?params.device_name,
            mac_address = ?params.mac_address,
            "Processing config request"
        );

        let mut manager = self.config_manager()?;

        let result = match params.target {
            ConfigTarget::WolDevice => wol::handle_wol(&mut manager, &params).await,
            _ => match params.action {
                ConfigAction::Set => handler::handle_set(&mut manager, &params).await,
                ConfigAction::Get => handler::handle_get(&manager, &params).await,
                ConfigAction::List => handler::handle_list(&manager, &params).await,
                ConfigAction::Delete => handler::handle_delete(&mut manager, &params).await,
            },
        }?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(result, duration_ms))
    }
}
