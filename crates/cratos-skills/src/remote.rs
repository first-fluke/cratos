//! Remote skill registry client.
//!
//! Default registry: GitHub repo with `index.json` + individual `.skill.json` files.
//! Format: `https://raw.githubusercontent.com/<owner>/<repo>/main/index.json`
//!
//! # Operations
//!
//! - **search**: Query the index for skills matching a keyword
//! - **fetch**: Download a specific skill by name
//! - **publish**: Upload a skill to the registry (requires auth token)

use crate::{Error, PortableSkill, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default registry URL (placeholder — users should configure their own).
pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/cratos-skills/registry/main";

/// A skill entry in the remote registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Skill name (unique within registry)
    pub name: String,
    /// Semantic version (e.g., "1.0.0")
    pub version: String,
    /// Short description
    pub description: String,
    /// Author name or handle
    pub author: String,
    /// Category (e.g., "devops", "data", "general")
    pub category: String,
    /// URL to download the `.skill.json` file
    pub download_url: String,
    /// SHA-256 checksum of the skill file
    pub checksum: String,
    /// Download count (informational)
    #[serde(default)]
    pub downloads: u64,
}

/// Remote skill registry client.
pub struct RemoteRegistry {
    base_url: String,
    client: reqwest::Client,
}

impl RemoteRegistry {
    /// Create a new registry client with a custom base URL.
    pub fn new(base_url: &str) -> Self {
        let url = base_url.trim_end_matches('/').to_string();
        Self {
            base_url: url,
            client: reqwest::Client::new(),
        }
    }

    /// Create a client pointing to the default registry.
    pub fn default_registry() -> Self {
        Self::new(DEFAULT_REGISTRY_URL)
    }

    /// Fetch the full registry index.
    pub async fn fetch_index(&self) -> Result<Vec<RegistryEntry>> {
        let url = format!("{}/index.json", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch registry index: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::Internal(format!(
                "Registry returned HTTP {}",
                resp.status()
            )));
        }

        let entries: Vec<RegistryEntry> = resp
            .json()
            .await
            .map_err(|e| Error::Internal(format!("Failed to parse registry index: {e}")))?;

        Ok(entries)
    }

    /// Search the registry for skills matching a query string.
    ///
    /// Matches against name, description, category, and author (case-insensitive).
    pub async fn search(&self, query: &str) -> Result<Vec<RegistryEntry>> {
        let index = self.fetch_index().await?;
        let query_lower = query.to_lowercase();

        let results: Vec<RegistryEntry> = index
            .into_iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&query_lower)
                    || e.description.to_lowercase().contains(&query_lower)
                    || e.category.to_lowercase().contains(&query_lower)
                    || e.author.to_lowercase().contains(&query_lower)
            })
            .collect();

        Ok(results)
    }

    /// Download and parse a skill from the registry.
    ///
    /// Verifies the SHA-256 checksum against the index entry.
    pub async fn fetch_skill(&self, name: &str) -> Result<PortableSkill> {
        let index = self.fetch_index().await?;

        let entry = index
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| Error::NotFound(format!("Skill '{}' not found in registry", name)))?;

        let resp = self
            .client
            .get(&entry.download_url)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to download skill: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::Internal(format!(
                "Download returned HTTP {}",
                resp.status()
            )));
        }

        let body = resp
            .bytes()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read skill body: {e}")))?;

        // Verify checksum
        let mut hasher = Sha256::new();
        hasher.update(&body);
        let computed: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        if computed != entry.checksum {
            return Err(Error::Internal(format!(
                "Checksum mismatch: expected {}, got {}",
                entry.checksum, computed
            )));
        }

        let skill: PortableSkill = serde_json::from_slice(&body)
            .map_err(|e| Error::Internal(format!("Failed to parse skill JSON: {e}")))?;

        Ok(skill)
    }

    /// Publish a skill to the registry.
    ///
    /// This posts the skill JSON to `{base_url}/publish` with a Bearer token.
    /// The registry server is expected to handle storage and index update.
    pub async fn publish(&self, skill: &PortableSkill, token: &str) -> Result<()> {
        let url = format!("{}/publish", self.base_url);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(skill)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to publish skill: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Internal(format!(
                "Publish failed (HTTP {}): {}",
                status, body
            )));
        }

        Ok(())
    }
}

/// Compute SHA-256 checksum of a byte slice (hex string).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_entry_serialization() {
        let entry = RegistryEntry {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "A test skill".to_string(),
            author: "tester".to_string(),
            category: "general".to_string(),
            download_url: "https://example.com/test.skill.json".to_string(),
            checksum: "abc123".to_string(),
            downloads: 42,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: RegistryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-skill");
        assert_eq!(deserialized.downloads, 42);
    }

    #[test]
    fn test_remote_registry_new() {
        let reg = RemoteRegistry::new("https://example.com/skills/");
        assert_eq!(reg.base_url, "https://example.com/skills");
    }

    #[test]
    fn test_default_registry() {
        let reg = RemoteRegistry::default_registry();
        assert!(reg.base_url.contains("cratos-skills"));
    }

    #[test]
    fn test_sha256_hex() {
        let hash = sha256_hex(b"hello");
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_registry_entry_default_downloads() {
        let json = r#"{"name":"x","version":"1","description":"d","author":"a","category":"c","download_url":"u","checksum":"c"}"#;
        let entry: RegistryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.downloads, 0);
    }
}
