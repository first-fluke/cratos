//! Configuration Tool - LLM-driven structured input
//!
//! This tool receives structured parameters directly from the LLM.
//! NO pattern matching or natural language parsing - the LLM extracts
//! structured parameters based on the JSON Schema.

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info};

use super::config_manager::{ConfigManager, MacAddressGuide};

/// Configuration action types - LLM selects directly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigAction {
    /// Set/update a configuration value
    Set,
    /// Get/read a configuration value
    Get,
    /// List all configuration options or devices
    List,
    /// Delete a configuration (e.g., WoL device)
    Delete,
}

/// Configuration target types - LLM selects directly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigTarget {
    /// LLM provider (openai, anthropic, etc.)
    LlmProvider,
    /// LLM model (gpt-4, claude-3, etc.)
    LlmModel,
    /// Default channel (telegram, slack, etc.)
    Channel,
    /// Default persona (cratos, sindri, etc.)
    Persona,
    /// Response language (en, ko, etc.)
    Language,
    /// UI theme
    Theme,
    /// WoL device registration/management
    WolDevice,
    /// Scheduler settings
    Scheduler,
}

/// Load available persona names from config directory
fn load_persona_names() -> Vec<String> {
    let mut names = Vec::new();
    // Try to find the config directory relative to CWD
    let paths = vec![
        "config/pantheon",
        "../config/pantheon",
        "../../config/pantheon",
    ];

    for path_str in paths {
        let path = std::path::Path::new(path_str);
        if path.exists() && path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "toml") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
            // If we found a valid directory, stop searching
            if !names.is_empty() {
                break;
            }
        }
    }

    // Fallback if no config found (e.g. tests)
    if names.is_empty() {
        names = vec![
            "cratos".to_string(),
            "sindri".to_string(),
            "athena".to_string(),
            "heimdall".to_string(),
            "mimir".to_string(),
        ];
    }

    names.sort();
    names
}

impl ConfigTarget {
    /// Get human-readable name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::LlmProvider => "LLM Provider",
            Self::LlmModel => "LLM Model",
            Self::Channel => "Channel",
            Self::Persona => "Persona",
            Self::Language => "Language",
            Self::Theme => "Theme",
            Self::WolDevice => "WoL Device",
            Self::Scheduler => "Scheduler",
        }
    }

    /// Get available options for this target
    pub fn available_options(&self) -> Vec<String> {
        match self {
            Self::LlmProvider => vec![
                "openai",
                "anthropic",
                "groq",
                "deepseek",
                "gemini",
                "ollama",
                "openrouter",
                "novita",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            Self::LlmModel => vec![
                "gpt-4o",
                "gpt-4o-mini",
                "claude-sonnet-4",
                "claude-3-5-sonnet",
                "llama-3.3-70b",
                "deepseek-chat",
                "gemini-2.0-flash",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            Self::Channel => vec!["telegram", "slack", "discord", "cli"]
                .into_iter()
                .map(String::from)
                .collect(),
            Self::Persona => load_persona_names(),
            Self::Language => vec!["en", "ko", "ja", "zh"]
                .into_iter()
                .map(String::from)
                .collect(),
            Self::Theme => vec!["dark", "light", "system"]
                .into_iter()
                .map(String::from)
                .collect(),
            Self::WolDevice => vec![], // Dynamic, managed by ConfigManager
            Self::Scheduler => vec!["enable", "disable"]
                .into_iter()
                .map(String::from)
                .collect(),
        }
    }
}

/// Input parameters for config tool - directly from LLM
#[derive(Debug, Deserialize)]
pub struct ConfigInput {
    /// Action to perform
    pub action: ConfigAction,
    /// Configuration target
    pub target: ConfigTarget,
    /// Value for set/update operations
    #[serde(default)]
    pub value: Option<String>,
    /// Device name for WoL operations
    #[serde(default)]
    pub device_name: Option<String>,
    /// MAC address for WoL registration
    #[serde(default)]
    pub mac_address: Option<String>,
}

/// Configuration tool for Cratos settings
pub struct ConfigTool {
    definition: ToolDefinition,
    data_dir: PathBuf,
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
        // Simple, clear description - no LLM coaching
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

        // Direct deserialization - no pattern matching
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

        let result = match params.target {
            ConfigTarget::WolDevice => self.handle_wol(&params).await,
            _ => match params.action {
                ConfigAction::Set => self.handle_set(&params).await,
                ConfigAction::Get => self.handle_get(&params).await,
                ConfigAction::List => self.handle_list(&params).await,
                ConfigAction::Delete => self.handle_delete(&params).await,
            },
        }?;

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(result, duration_ms))
    }
}

impl ConfigTool {
    /// Handle WoL device operations
    async fn handle_wol(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        match params.action {
            ConfigAction::Set => self.handle_wol_set(params).await,
            ConfigAction::List => self.handle_wol_list().await,
            ConfigAction::Get => self.handle_wol_get(params).await,
            ConfigAction::Delete => self.handle_wol_delete(params).await,
        }
    }

    /// Register or update a WoL device
    async fn handle_wol_set(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        let device_name = params.device_name.as_ref().ok_or_else(|| {
            Error::InvalidInput("device_name is required for WoL registration".to_string())
        })?;

        // If no MAC address, provide guidance
        let Some(mac_address) = &params.mac_address else {
            return Ok(serde_json::json!({
                "status": "needs_info",
                "target": "wol_device",
                "device_name": device_name,
                "message": format!("'{}'을(를) 등록하려면 MAC 주소가 필요해요.", device_name),
                "guidance": MacAddressGuide::instructions(),
                "next_step": "MAC 주소를 알려주시면 등록할게요. (예: AA:BB:CC:DD:EE:FF)"
            }));
        };

        // Register the device
        let mut manager = self.config_manager()?;
        manager.register_wol_device(device_name, mac_address, None)?;

        info!(
            device = %device_name,
            mac = %mac_address,
            "WoL device registered via config tool"
        );

        Ok(serde_json::json!({
            "status": "success",
            "action": "set",
            "target": "wol_device",
            "device_name": device_name,
            "mac_address": mac_address,
            "message": format!("'{}' 디바이스가 등록되었어요! 이제 '{}' 켜줘 라고 말하면 됩니다.", device_name, device_name)
        }))
    }

    /// List all WoL devices
    async fn handle_wol_list(&self) -> Result<serde_json::Value> {
        let manager = self.config_manager()?;
        let devices = manager.list_wol_devices();

        if devices.is_empty() {
            return Ok(serde_json::json!({
                "status": "success",
                "action": "list",
                "target": "wol_device",
                "devices": [],
                "message": "등록된 WoL 디바이스가 없어요. 디바이스를 등록하려면 이름과 MAC 주소를 알려주세요."
            }));
        }

        let device_list: Vec<serde_json::Value> = devices
            .iter()
            .map(|(name, device)| {
                serde_json::json!({
                    "name": name,
                    "mac_address": device.mac_address,
                    "description": device.description
                })
            })
            .collect();

        Ok(serde_json::json!({
            "status": "success",
            "action": "list",
            "target": "wol_device",
            "devices": device_list,
            "message": format!("등록된 WoL 디바이스 {}개", devices.len())
        }))
    }

    /// Get a specific WoL device
    async fn handle_wol_get(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        let device_name = params
            .device_name
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("device_name is required".to_string()))?;

        let manager = self.config_manager()?;
        let device = manager
            .get_wol_device(device_name)
            .ok_or_else(|| Error::NotFound(format!("Device '{}' not found", device_name)))?;

        Ok(serde_json::json!({
            "status": "success",
            "action": "get",
            "target": "wol_device",
            "device_name": device_name,
            "mac_address": device.mac_address,
            "description": device.description,
            "message": format!("'{}': {}", device_name, device.mac_address)
        }))
    }

    /// Delete a WoL device
    async fn handle_wol_delete(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        let device_name = params
            .device_name
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("device_name is required".to_string()))?;

        let mut manager = self.config_manager()?;
        manager.delete_wol_device(device_name)?;

        info!(device = %device_name, "WoL device deleted");

        Ok(serde_json::json!({
            "status": "success",
            "action": "delete",
            "target": "wol_device",
            "device_name": device_name,
            "message": format!("'{}' 디바이스가 삭제되었어요.", device_name)
        }))
    }

    /// Handle general set operation
    async fn handle_set(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        let value = params.value.as_ref().ok_or_else(|| {
            Error::InvalidInput(format!(
                "value is required for setting {}. Available options: {:?}",
                params.target.display_name(),
                params.target.available_options()
            ))
        })?;

        // Validate value against available options
        let options = params.target.available_options();
        if !options.is_empty() {
            let value_lower = value.to_lowercase();
            let matched = options
                .iter()
                .find(|&opt| opt.to_lowercase() == value_lower);

            if matched.is_none() {
                return Err(Error::InvalidInput(format!(
                    "Invalid value '{}' for {}. Available: {:?}",
                    value,
                    params.target.display_name(),
                    options
                )));
            }
        }

        // Update configuration via ConfigManager
        let mut manager = self.config_manager()?;
        let config = manager.config_mut();

        match params.target {
            ConfigTarget::LlmProvider => config.llm.default_provider = value.clone(),
            ConfigTarget::LlmModel => config.llm.default_model = value.clone(),
            ConfigTarget::Language => config.language = value.clone(),
            ConfigTarget::Persona => config.persona = value.clone(),
            ConfigTarget::Scheduler => {
                config.scheduler.enabled = value == "enable" || value == "enabled";
            }
            ConfigTarget::Channel => config.channel = value.clone(),
            ConfigTarget::Theme => config.theme = value.clone(),
            _ => {}
        }

        manager.save()?;

        info!(
            target = ?params.target,
            value = %value,
            "Configuration updated"
        );

        Ok(serde_json::json!({
            "status": "success",
            "action": "set",
            "target": params.target.display_name(),
            "value": value,
            "message": format!("{} → {}", params.target.display_name(), value)
        }))
    }

    /// Handle get operation
    async fn handle_get(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        let manager = self.config_manager()?;
        let config = manager.config();

        let current_value = match params.target {
            ConfigTarget::LlmProvider => {
                if config.llm.default_provider.is_empty() {
                    "auto".to_string()
                } else {
                    config.llm.default_provider.clone()
                }
            }
            ConfigTarget::LlmModel => {
                if config.llm.default_model.is_empty() {
                    "auto".to_string()
                } else {
                    config.llm.default_model.clone()
                }
            }
            ConfigTarget::Language => {
                if config.language.is_empty() {
                    "en".to_string()
                } else {
                    config.language.clone()
                }
            }
            ConfigTarget::Persona => {
                if config.persona.is_empty() {
                    "cratos".to_string()
                } else {
                    config.persona.clone()
                }
            }
            ConfigTarget::Scheduler => {
                if config.scheduler.enabled {
                    "enabled".to_string()
                } else {
                    "disabled".to_string()
                }
            }
            ConfigTarget::Channel => {
                if config.channel.is_empty() {
                    "telegram".to_string()
                } else {
                    config.channel.clone()
                }
            }
            ConfigTarget::Theme => {
                if config.theme.is_empty() {
                    "dark".to_string()
                } else {
                    config.theme.clone()
                }
            }
            ConfigTarget::WolDevice => {
                return self.handle_wol_list().await;
            }
        };

        Ok(serde_json::json!({
            "status": "success",
            "action": "get",
            "target": params.target.display_name(),
            "current_value": current_value,
            "available_options": params.target.available_options(),
            "message": format!("{}: {}", params.target.display_name(), current_value)
        }))
    }

    /// Handle list operation
    async fn handle_list(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        if params.target == ConfigTarget::WolDevice {
            return self.handle_wol_list().await;
        }

        let options = params.target.available_options();

        Ok(serde_json::json!({
            "status": "success",
            "action": "list",
            "target": params.target.display_name(),
            "options": options,
            "message": format!("{} 옵션: {}", params.target.display_name(), options.join(", "))
        }))
    }

    /// Handle delete operation (mostly for WoL devices)
    async fn handle_delete(&self, params: &ConfigInput) -> Result<serde_json::Value> {
        if params.target == ConfigTarget::WolDevice {
            return self.handle_wol_delete(params).await;
        }

        // For other targets, "delete" means "reset to default"
        let default_value = match params.target {
            ConfigTarget::LlmProvider => "auto",
            ConfigTarget::LlmModel => "auto",
            ConfigTarget::Language => "en",
            ConfigTarget::Persona => "cratos",
            ConfigTarget::Channel => "telegram",
            ConfigTarget::Theme => "system",
            ConfigTarget::Scheduler => "disabled",
            ConfigTarget::WolDevice => unreachable!(),
        };

        // Update via set with default value
        let reset_params = ConfigInput {
            action: ConfigAction::Set,
            target: params.target,
            value: Some(default_value.to_string()),
            device_name: None,
            mac_address: None,
        };

        let mut result = self.handle_set(&reset_params).await?;
        if let Some(obj) = result.as_object_mut() {
            obj.insert("action".to_string(), serde_json::json!("delete"));
            obj.insert(
                "message".to_string(),
                serde_json::json!(format!(
                    "{} 초기화됨 → {}",
                    params.target.display_name(),
                    default_value
                )),
            );
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_tool() -> (ConfigTool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let tool = ConfigTool::with_data_dir(temp_dir.path().to_path_buf());
        (tool, temp_dir)
    }

    #[tokio::test]
    async fn test_wol_registration_without_mac() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "set",
            "target": "wol_device",
            "device_name": "원격피씨"
        });

        let result = tool.execute(input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["status"], "needs_info");
        assert!(output["guidance"].as_str().unwrap().contains("MAC"));
    }

    #[tokio::test]
    async fn test_wol_registration_with_mac() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "set",
            "target": "wol_device",
            "device_name": "원격피씨",
            "mac_address": "AA:BB:CC:DD:EE:FF"
        });

        let result = tool.execute(input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["status"], "success");
        assert_eq!(output["device_name"], "원격피씨");
    }

    #[tokio::test]
    async fn test_wol_list_empty() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "list",
            "target": "wol_device"
        });

        let result = tool.execute(input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["status"], "success");
        assert_eq!(output["devices"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_set_language() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "set",
            "target": "language",
            "value": "ko"
        });

        let result = tool.execute(input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["status"], "success");
        assert_eq!(output["value"], "ko");
    }

    #[tokio::test]
    async fn test_set_invalid_value() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "set",
            "target": "language",
            "value": "invalid_language"
        });

        let result = tool.execute(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_language() {
        let (tool, _dir) = create_test_tool();

        // First set
        let set_input = serde_json::json!({
            "action": "set",
            "target": "language",
            "value": "ko"
        });
        tool.execute(set_input).await.unwrap();

        // Then get
        let get_input = serde_json::json!({
            "action": "get",
            "target": "language"
        });

        let result = tool.execute(get_input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["current_value"], "ko");
    }

    #[tokio::test]
    async fn test_list_options() {
        let (tool, _dir) = create_test_tool();

        let input = serde_json::json!({
            "action": "list",
            "target": "persona"
        });

        let result = tool.execute(input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["status"], "success");
        let options = output["options"].as_array().unwrap();
        assert!(options.iter().any(|v| v == "cratos"));
        assert!(options.iter().any(|v| v == "sindri"));
    }

    #[tokio::test]
    async fn test_delete_resets_to_default() {
        let (tool, _dir) = create_test_tool();

        // Set a value
        let set_input = serde_json::json!({
            "action": "set",
            "target": "language",
            "value": "ko"
        });
        tool.execute(set_input).await.unwrap();

        // Delete (reset)
        let delete_input = serde_json::json!({
            "action": "delete",
            "target": "language"
        });

        let result = tool.execute(delete_input).await.unwrap();
        let output: serde_json::Value = result.output.clone();

        assert_eq!(output["value"], "en"); // Default
    }

    #[tokio::test]
    async fn test_set_and_get_channel() {
        let (tool, _dir) = create_test_tool();

        let set_input = serde_json::json!({
            "action": "set",
            "target": "channel",
            "value": "slack"
        });
        tool.execute(set_input).await.unwrap();

        let get_input = serde_json::json!({
            "action": "get",
            "target": "channel"
        });
        let result = tool.execute(get_input).await.unwrap();
        let output: serde_json::Value = result.output.clone();
        assert_eq!(output["current_value"], "slack");
    }

    #[tokio::test]
    async fn test_set_and_get_theme() {
        let (tool, _dir) = create_test_tool();

        let set_input = serde_json::json!({
            "action": "set",
            "target": "theme",
            "value": "light"
        });
        tool.execute(set_input).await.unwrap();

        let get_input = serde_json::json!({
            "action": "get",
            "target": "theme"
        });
        let result = tool.execute(get_input).await.unwrap();
        let output: serde_json::Value = result.output.clone();
        assert_eq!(output["current_value"], "light");
    }

    #[test]
    fn test_config_tool_definition() {
        let tool = ConfigTool::new();
        assert_eq!(tool.definition().name, "config");
        assert_eq!(tool.definition().risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_config_target_options() {
        assert!(!ConfigTarget::LlmProvider.available_options().is_empty());
        assert!(!ConfigTarget::LlmModel.available_options().is_empty());
        assert!(!ConfigTarget::Persona.available_options().is_empty());
    }

    #[test]
    fn test_serde_deserialization() {
        let json = serde_json::json!({
            "action": "set",
            "target": "llm_model",
            "value": "claude-sonnet-4"
        });

        let input: ConfigInput = serde_json::from_value(json).unwrap();
        assert_eq!(input.action, ConfigAction::Set);
        assert_eq!(input.target, ConfigTarget::LlmModel);
        assert_eq!(input.value, Some("claude-sonnet-4".to_string()));
    }
}
