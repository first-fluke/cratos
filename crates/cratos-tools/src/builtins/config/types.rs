use serde::{Deserialize, Serialize};

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
pub fn load_persona_names() -> Vec<String> {
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
