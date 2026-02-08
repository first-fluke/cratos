//! Configuration Manager
//!
//! Provides secure, user-friendly configuration management via natural language.
//! Handles all Cratos settings including WoL devices, LLM, channels, etc.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// User-facing configuration that can be modified via natural language
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    /// WoL device mappings (friendly name -> MAC address)
    #[serde(default)]
    pub wol_devices: HashMap<String, WolDevice>,

    /// LLM settings
    #[serde(default)]
    pub llm: LlmConfig,

    /// Response language
    #[serde(default)]
    pub language: String,

    /// Default persona
    #[serde(default)]
    pub persona: String,

    /// Default channel
    #[serde(default)]
    pub channel: String,

    /// UI theme
    #[serde(default)]
    pub theme: String,

    /// Scheduler settings
    #[serde(default)]
    pub scheduler: SchedulerConfig,
}

/// WoL device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WolDevice {
    /// MAC address (validated format)
    pub mac_address: String,
    /// Optional description
    #[serde(default)]
    pub description: String,
    /// Broadcast address (optional, defaults to 255.255.255.255)
    #[serde(default)]
    pub broadcast_address: Option<String>,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    /// Default provider
    #[serde(default)]
    pub default_provider: String,
    /// Default model
    #[serde(default)]
    pub default_model: String,
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerConfig {
    /// Whether scheduler is enabled
    #[serde(default)]
    pub enabled: bool,
}

/// Configuration manager with secure file operations
pub struct ConfigManager {
    /// Path to user config file
    config_path: PathBuf,
    /// Loaded configuration
    config: UserConfig,
}

impl ConfigManager {
    /// Default config filename
    const CONFIG_FILENAME: &'static str = "user_config.toml";

    /// Create a new config manager
    ///
    /// # Arguments
    /// * `data_dir` - Cratos data directory (e.g., ~/.cratos)
    pub fn new(data_dir: &Path) -> Result<Self> {
        let config_path = data_dir.join(Self::CONFIG_FILENAME);

        // Ensure data directory exists with secure permissions
        Self::ensure_secure_directory(data_dir)?;

        // Load existing config or create default
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            UserConfig::default()
        };

        Ok(Self {
            config_path,
            config,
        })
    }

    /// Ensure directory exists with secure permissions (0o700 on Unix)
    fn ensure_secure_directory(dir: &Path) -> Result<()> {
        if !dir.exists() {
            fs::create_dir_all(dir)
                .map_err(|e| Error::Config(format!("Failed to create config directory: {}", e)))?;
        }

        #[cfg(unix)]
        {
            let perms = fs::Permissions::from_mode(0o700);
            fs::set_permissions(dir, perms).map_err(|e| {
                Error::Config(format!("Failed to set directory permissions: {}", e))
            })?;
        }

        Ok(())
    }

    /// Load configuration from file
    fn load_config(path: &Path) -> Result<UserConfig> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))
    }

    /// Save configuration to file with secure permissions
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.config)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;

        // Write atomically using temp file
        let temp_path = self.config_path.with_extension("tmp");
        fs::write(&temp_path, &content)
            .map_err(|e| Error::Config(format!("Failed to write config: {}", e)))?;

        // Set secure permissions before rename
        #[cfg(unix)]
        {
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&temp_path, perms)
                .map_err(|e| Error::Config(format!("Failed to set file permissions: {}", e)))?;
        }

        // Atomic rename
        fs::rename(&temp_path, &self.config_path)
            .map_err(|e| Error::Config(format!("Failed to save config: {}", e)))?;

        info!(path = ?self.config_path, "Configuration saved");
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &UserConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut UserConfig {
        &mut self.config
    }

    // =========================================================================
    // WoL Device Management
    // =========================================================================

    /// Register a new WoL device
    ///
    /// # Security
    /// - Validates MAC address format
    /// - Sanitizes device name (no special characters)
    /// - Checks for duplicate names
    pub fn register_wol_device(
        &mut self,
        name: &str,
        mac_address: &str,
        description: Option<&str>,
    ) -> Result<()> {
        // Validate and sanitize device name
        let sanitized_name = Self::sanitize_device_name(name)?;

        // Validate MAC address format
        let normalized_mac = Self::validate_and_normalize_mac(mac_address)?;

        // Check for duplicates
        if self.config.wol_devices.contains_key(&sanitized_name) {
            return Err(Error::Config(format!(
                "Device '{}' already exists. Use update or delete first.",
                sanitized_name
            )));
        }

        // Check for duplicate MAC addresses
        for (existing_name, device) in &self.config.wol_devices {
            if device.mac_address == normalized_mac {
                return Err(Error::Config(format!(
                    "MAC address {} is already registered as '{}'",
                    normalized_mac, existing_name
                )));
            }
        }

        let device = WolDevice {
            mac_address: normalized_mac.clone(),
            description: description.unwrap_or("").to_string(),
            broadcast_address: None,
        };

        self.config
            .wol_devices
            .insert(sanitized_name.clone(), device);
        self.save()?;

        info!(
            name = %sanitized_name,
            mac = %normalized_mac,
            "WoL device registered"
        );

        Ok(())
    }

    /// Update an existing WoL device
    pub fn update_wol_device(
        &mut self,
        name: &str,
        mac_address: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        let device = self
            .config
            .wol_devices
            .get_mut(name)
            .ok_or_else(|| Error::Config(format!("Device '{}' not found", name)))?;

        if let Some(mac) = mac_address {
            device.mac_address = Self::validate_and_normalize_mac(mac)?;
        }

        if let Some(desc) = description {
            device.description = desc.to_string();
        }

        self.save()?;
        info!(name = %name, "WoL device updated");
        Ok(())
    }

    /// Delete a WoL device
    pub fn delete_wol_device(&mut self, name: &str) -> Result<()> {
        if self.config.wol_devices.remove(name).is_none() {
            return Err(Error::Config(format!("Device '{}' not found", name)));
        }

        self.save()?;
        info!(name = %name, "WoL device deleted");
        Ok(())
    }

    /// List all WoL devices
    pub fn list_wol_devices(&self) -> Vec<(String, WolDevice)> {
        self.config
            .wol_devices
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get a WoL device by name
    pub fn get_wol_device(&self, name: &str) -> Option<&WolDevice> {
        self.config.wol_devices.get(name)
    }

    // =========================================================================
    // Validation Helpers
    // =========================================================================

    /// Sanitize device name - allow only safe characters
    ///
    /// # Security
    /// Prevents injection attacks and file system issues
    fn sanitize_device_name(name: &str) -> Result<String> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(Error::InvalidInput(
                "Device name cannot be empty".to_string(),
            ));
        }

        if trimmed.len() > 50 {
            return Err(Error::InvalidInput(
                "Device name too long (max 50 characters)".to_string(),
            ));
        }

        // Allow: letters (any language), numbers, spaces, hyphens, underscores
        // Disallow: special characters that could cause issues
        let sanitized: String = trimmed
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .collect();

        if sanitized.is_empty() {
            return Err(Error::InvalidInput(
                "Device name must contain at least one alphanumeric character".to_string(),
            ));
        }

        // Check for suspicious patterns
        let lower = sanitized.to_lowercase();
        if lower.contains("..") || lower.contains("//") || lower.contains("\\") {
            warn!(name = %name, "Suspicious device name rejected");
            return Err(Error::InvalidInput("Invalid device name".to_string()));
        }

        Ok(sanitized)
    }

    /// Validate and normalize MAC address
    ///
    /// Accepts:
    /// - AA:BB:CC:DD:EE:FF (colon-separated)
    /// - AA-BB-CC-DD-EE-FF (dash-separated)
    /// - AABBCCDDEEFF (no separator)
    ///
    /// Returns normalized format: AA:BB:CC:DD:EE:FF
    fn validate_and_normalize_mac(mac: &str) -> Result<String> {
        // Remove all separators and whitespace
        let cleaned: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();

        if cleaned.len() != 12 {
            return Err(Error::InvalidInput(format!(
                "Invalid MAC address '{}'. Expected 12 hex digits (e.g., AA:BB:CC:DD:EE:FF)",
                mac
            )));
        }

        // Validate all characters are hex
        if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::InvalidInput(format!(
                "Invalid characters in MAC address '{}'",
                mac
            )));
        }

        // Format as uppercase with colons
        let bytes: Vec<&str> = (0..6).map(|i| &cleaned[i * 2..i * 2 + 2]).collect();

        Ok(bytes.join(":").to_uppercase())
    }
}

/// User-friendly instructions for finding MAC address
pub struct MacAddressGuide;

impl MacAddressGuide {
    /// Get instructions for finding MAC address based on OS
    pub fn instructions() -> String {
        r#"**MAC 주소 찾는 방법:**

**Windows:**
1. `Win + R` → `cmd` 입력 → Enter
2. `ipconfig /all` 입력
3. "물리적 주소" 또는 "Physical Address" 찾기
   예: `00-1A-2B-3C-4D-5E`

**Mac:**
1. 시스템 설정 → 네트워크
2. 사용 중인 연결 선택 → 세부사항
3. 하드웨어 탭 → MAC 주소

**공유기 관리 페이지:**
1. 브라우저에서 `192.168.0.1` 또는 `192.168.1.1` 접속
2. 연결된 기기 목록에서 확인

**참고:** MAC 주소는 `AA:BB:CC:DD:EE:FF` 형식의 12자리 16진수입니다."#
            .to_string()
    }

    /// Get short hint
    pub fn short_hint() -> &'static str {
        "MAC 주소는 네트워크 카드의 고유 주소예요. 'ipconfig /all' (Windows) 또는 시스템 설정 → 네트워크 (Mac)에서 찾을 수 있어요."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (ConfigManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_sanitize_device_name_valid() {
        assert_eq!(
            ConfigManager::sanitize_device_name("원격피씨").unwrap(),
            "원격피씨"
        );
        assert_eq!(
            ConfigManager::sanitize_device_name("My PC").unwrap(),
            "My PC"
        );
        assert_eq!(
            ConfigManager::sanitize_device_name("server-1").unwrap(),
            "server-1"
        );
        assert_eq!(
            ConfigManager::sanitize_device_name("pc_home").unwrap(),
            "pc_home"
        );
    }

    #[test]
    fn test_sanitize_device_name_strips_dangerous() {
        // Special characters are stripped
        assert_eq!(
            ConfigManager::sanitize_device_name("my<pc>").unwrap(),
            "mypc"
        );
        assert_eq!(
            ConfigManager::sanitize_device_name("test;rm -rf").unwrap(),
            "testrm -rf"
        );
    }

    #[test]
    fn test_sanitize_device_name_rejects_empty() {
        assert!(ConfigManager::sanitize_device_name("").is_err());
        assert!(ConfigManager::sanitize_device_name("   ").is_err());
        assert!(ConfigManager::sanitize_device_name("!!!").is_err());
    }

    #[test]
    fn test_sanitize_device_name_strips_path_chars() {
        // Path traversal chars (./) are stripped, leaving alphanumeric content
        assert_eq!(
            ConfigManager::sanitize_device_name("../etc/passwd").unwrap(),
            "etcpasswd"
        );
        // Backslash is stripped
        assert_eq!(
            ConfigManager::sanitize_device_name("..\\windows").unwrap(),
            "windows"
        );
    }

    #[test]
    fn test_validate_mac_colon_format() {
        assert_eq!(
            ConfigManager::validate_and_normalize_mac("AA:BB:CC:DD:EE:FF").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
        assert_eq!(
            ConfigManager::validate_and_normalize_mac("aa:bb:cc:dd:ee:ff").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn test_validate_mac_dash_format() {
        assert_eq!(
            ConfigManager::validate_and_normalize_mac("AA-BB-CC-DD-EE-FF").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn test_validate_mac_no_separator() {
        assert_eq!(
            ConfigManager::validate_and_normalize_mac("AABBCCDDEEFF").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn test_validate_mac_invalid() {
        assert!(ConfigManager::validate_and_normalize_mac("invalid").is_err());
        assert!(ConfigManager::validate_and_normalize_mac("AA:BB:CC").is_err());
        assert!(ConfigManager::validate_and_normalize_mac("GG:HH:II:JJ:KK:LL").is_err());
    }

    #[test]
    fn test_register_wol_device() {
        let (mut manager, _dir) = create_test_manager();

        manager
            .register_wol_device("원격피씨", "AA:BB:CC:DD:EE:FF", Some("내 데스크톱"))
            .unwrap();

        let device = manager.get_wol_device("원격피씨").unwrap();
        assert_eq!(device.mac_address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(device.description, "내 데스크톱");
    }

    #[test]
    fn test_register_duplicate_name_rejected() {
        let (mut manager, _dir) = create_test_manager();

        manager
            .register_wol_device("피씨", "AA:BB:CC:DD:EE:FF", None)
            .unwrap();

        let result = manager.register_wol_device("피씨", "11:22:33:44:55:66", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_duplicate_mac_rejected() {
        let (mut manager, _dir) = create_test_manager();

        manager
            .register_wol_device("피씨1", "AA:BB:CC:DD:EE:FF", None)
            .unwrap();

        let result = manager.register_wol_device("피씨2", "AA:BB:CC:DD:EE:FF", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_wol_device() {
        let (mut manager, _dir) = create_test_manager();

        manager
            .register_wol_device("테스트", "AA:BB:CC:DD:EE:FF", None)
            .unwrap();

        assert!(manager.get_wol_device("테스트").is_some());

        manager.delete_wol_device("테스트").unwrap();

        assert!(manager.get_wol_device("테스트").is_none());
    }

    #[test]
    fn test_list_wol_devices() {
        let (mut manager, _dir) = create_test_manager();

        manager
            .register_wol_device("피씨1", "AA:BB:CC:DD:EE:FF", None)
            .unwrap();
        manager
            .register_wol_device("피씨2", "11:22:33:44:55:66", None)
            .unwrap();

        let devices = manager.list_wol_devices();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn test_config_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create and save
        {
            let mut manager = ConfigManager::new(temp_dir.path()).unwrap();
            manager
                .register_wol_device("테스트", "AA:BB:CC:DD:EE:FF", None)
                .unwrap();
        }

        // Reload and verify
        {
            let manager = ConfigManager::new(temp_dir.path()).unwrap();
            let device = manager.get_wol_device("테스트").unwrap();
            assert_eq!(device.mac_address, "AA:BB:CC:DD:EE:FF");
        }
    }

    #[test]
    fn test_mac_guide_instructions() {
        let guide = MacAddressGuide::instructions();
        assert!(guide.contains("Windows"));
        assert!(guide.contains("Mac"));
        assert!(guide.contains("ipconfig"));
    }
}
