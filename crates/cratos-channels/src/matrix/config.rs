use crate::error::{Error, Result};
use serde::Deserialize;

/// Default device name for Matrix client
pub const DEFAULT_DEVICE_NAME: &str = "Cratos Bot";

/// Matrix adapter configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixConfig {
    /// Matrix homeserver URL (e.g., "https://matrix.org")
    pub homeserver_url: String,

    /// Bot user ID (e.g., "@bot:matrix.org")
    pub user_id: String,

    /// Password for login
    pub password: String,

    /// Device display name
    #[serde(default = "default_device_name")]
    pub device_name: String,

    /// Allowed room IDs (empty = allow all)
    #[serde(default)]
    pub allowed_rooms: Vec<String>,
}

fn default_device_name() -> String {
    DEFAULT_DEVICE_NAME.to_string()
}

impl MatrixConfig {
    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let homeserver_url = std::env::var("MATRIX_HOMESERVER_URL")
            .map_err(|_| Error::Network("MATRIX_HOMESERVER_URL not set".to_string()))?;

        let user_id = std::env::var("MATRIX_USER_ID")
            .map_err(|_| Error::Network("MATRIX_USER_ID not set".to_string()))?;

        let password = std::env::var("MATRIX_PASSWORD")
            .map_err(|_| Error::Network("MATRIX_PASSWORD not set".to_string()))?;

        let allowed_rooms = std::env::var("MATRIX_ALLOWED_ROOMS")
            .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
            .unwrap_or_default();

        Ok(Self {
            homeserver_url,
            user_id,
            password,
            device_name: default_device_name(),
            allowed_rooms,
        })
    }

    /// Check if a room is allowed
    pub fn is_room_allowed(&self, room_id: &str) -> bool {
        self.allowed_rooms.is_empty() || self.allowed_rooms.iter().any(|r| r == room_id)
    }
}
