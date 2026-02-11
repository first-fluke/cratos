//! PIN-based device pairing.
//!
//! Flow:
//! 1. Server generates a 6-digit PIN and holds it in memory (TTL 5 minutes)
//! 2. User views the PIN on the server (CLI or TUI)
//! 3. Client submits the PIN + its Ed25519 public key
//! 4. Server verifies PIN → registers the device public key
//!
//! After pairing, the device can authenticate via challenge-response
//! (see [`crate::device_auth`]).
//!
//! Devices are persisted to SQLite so they survive server restarts.

use crate::device_auth::DeviceAuthError;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// A pending pairing session.
struct PairingSession {
    pin: String,
    created_at: DateTime<Utc>,
}

/// Paired device record.
#[derive(Debug, Clone)]
pub struct PairedDevice {
    /// Device ID (auto-generated UUID)
    pub device_id: String,
    /// Human-readable device name
    pub device_name: String,
    /// Ed25519 public key (32 bytes)
    pub public_key: Vec<u8>,
    /// When the device was paired
    pub paired_at: DateTime<Utc>,
}

/// PIN pairing result.
#[derive(Debug, Clone)]
pub struct PairingResult {
    /// Whether pairing was successful
    pub success: bool,
    /// Device ID (if successful)
    pub device_id: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Manages PIN pairing sessions and registered devices.
///
/// PIN sessions are always in-memory (short TTL). Registered devices can be
/// optionally persisted to SQLite via `new_with_db`.
pub struct PairingManager {
    sessions: RwLock<HashMap<String, PairingSession>>,
    /// In-memory device store (used as cache when DB is present, or primary when not)
    devices: RwLock<HashMap<String, PairedDevice>>,
    /// Optional SQLite pool for persistent storage
    db: Option<sqlx::Pool<sqlx::Sqlite>>,
    pin_ttl_secs: i64,
}

impl PairingManager {
    /// Create a new pairing manager with default TTL (5 minutes), in-memory only.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            devices: RwLock::new(HashMap::new()),
            db: None,
            pin_ttl_secs: 300,
        }
    }

    /// Create a pairing manager backed by SQLite for device persistence.
    pub async fn new_with_db(db: sqlx::Pool<sqlx::Sqlite>) -> Result<Self, sqlx::Error> {
        // Create paired_devices table if it doesn't exist
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS paired_devices (
                device_id TEXT PRIMARY KEY,
                device_name TEXT NOT NULL,
                public_key BLOB NOT NULL,
                paired_at TEXT NOT NULL
            )",
        )
        .execute(&db)
        .await?;

        // Load existing devices into memory cache
        let rows = sqlx::query_as::<_, (String, String, Vec<u8>, String)>(
            "SELECT device_id, device_name, public_key, paired_at FROM paired_devices",
        )
        .fetch_all(&db)
        .await?;

        let mut devices = HashMap::new();
        for (device_id, device_name, public_key, paired_at_str) in rows {
            let paired_at = chrono::DateTime::parse_from_rfc3339(&paired_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            devices.insert(
                device_id.clone(),
                PairedDevice {
                    device_id,
                    device_name,
                    public_key,
                    paired_at,
                },
            );
        }

        debug!(count = devices.len(), "Loaded paired devices from SQLite");

        Ok(Self {
            sessions: RwLock::new(HashMap::new()),
            devices: RwLock::new(devices),
            db: Some(db),
            pin_ttl_secs: 300,
        })
    }

    /// Start a new pairing session. Returns a 6-digit PIN.
    pub async fn start_pairing(&self) -> String {
        let pin = generate_pin();
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = PairingSession {
            pin: pin.clone(),
            created_at: Utc::now(),
        };
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session);
        pin
    }

    /// Verify a PIN and register a device.
    ///
    /// Returns `PairingResult` with the device_id on success.
    pub async fn verify_pin(
        &self,
        pin: &str,
        device_name: &str,
        public_key: &[u8],
    ) -> PairingResult {
        if public_key.len() != 32 {
            return PairingResult {
                success: false,
                device_id: None,
                error: Some("Public key must be 32 bytes".to_string()),
            };
        }

        let mut sessions = self.sessions.write().await;

        // Find matching session
        let matching_id = sessions
            .iter()
            .find(|(_, s)| s.pin == pin)
            .map(|(id, _)| id.clone());

        let session_id = match matching_id {
            Some(id) => id,
            None => {
                return PairingResult {
                    success: false,
                    device_id: None,
                    error: Some("Invalid PIN".to_string()),
                };
            }
        };

        let session = sessions.remove(&session_id).unwrap();

        // Check TTL (use milliseconds for sub-second precision)
        let elapsed_ms = (Utc::now() - session.created_at).num_milliseconds();
        let ttl_ms = self.pin_ttl_secs * 1000;
        if elapsed_ms >= ttl_ms {
            return PairingResult {
                success: false,
                device_id: None,
                error: Some("PIN expired".to_string()),
            };
        }

        drop(sessions);

        // Register device
        let device_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let device = PairedDevice {
            device_id: device_id.clone(),
            device_name: device_name.to_string(),
            public_key: public_key.to_vec(),
            paired_at: now,
        };

        // Persist to SQLite if available
        if let Some(ref db) = self.db {
            if let Err(e) = sqlx::query(
                "INSERT INTO paired_devices (device_id, device_name, public_key, paired_at) VALUES (?, ?, ?, ?)",
            )
            .bind(&device_id)
            .bind(device_name)
            .bind(public_key)
            .bind(now.to_rfc3339())
            .execute(db)
            .await
            {
                warn!(error = %e, "Failed to persist paired device to SQLite");
            }
        }

        let mut devices = self.devices.write().await;
        devices.insert(device_id.clone(), device);

        PairingResult {
            success: true,
            device_id: Some(device_id),
            error: None,
        }
    }

    /// Get a paired device by ID.
    pub async fn get_device(&self, device_id: &str) -> Option<PairedDevice> {
        let devices = self.devices.read().await;
        devices.get(device_id).cloned()
    }

    /// Get a paired device's public key (for challenge-response auth).
    pub async fn get_device_public_key(&self, device_id: &str) -> Result<Vec<u8>, DeviceAuthError> {
        let devices = self.devices.read().await;
        devices
            .get(device_id)
            .map(|d| d.public_key.clone())
            .ok_or(DeviceAuthError::DeviceNotFound)
    }

    /// List all paired devices.
    pub async fn list_devices(&self) -> Vec<PairedDevice> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }

    /// Remove a paired device.
    pub async fn unpair_device(&self, device_id: &str) -> bool {
        // Remove from SQLite if available
        if let Some(ref db) = self.db {
            if let Err(e) = sqlx::query("DELETE FROM paired_devices WHERE device_id = ?")
                .bind(device_id)
                .execute(db)
                .await
            {
                warn!(error = %e, "Failed to delete paired device from SQLite");
            }
        }

        let mut devices = self.devices.write().await;
        devices.remove(device_id).is_some()
    }

    /// Clean expired pairing sessions.
    pub async fn cleanup_sessions(&self) -> usize {
        let cutoff = Utc::now() - chrono::Duration::seconds(self.pin_ttl_secs);
        let mut sessions = self.sessions.write().await;
        let before = sessions.len();
        sessions.retain(|_, s| s.created_at > cutoff);
        before - sessions.len()
    }
}

impl Default for PairingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a 6-digit PIN.
fn generate_pin() -> String {
    use rand::Rng;
    let pin: u32 = rand::thread_rng().gen_range(100_000..1_000_000);
    format!("{:06}", pin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_pairing_generates_6_digit_pin() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }

    #[tokio::test]
    async fn test_verify_pin_success() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr
            .verify_pin(&pin, "my-phone", vk.as_bytes())
            .await;

        assert!(result.success);
        assert!(result.device_id.is_some());
    }

    #[tokio::test]
    async fn test_verify_wrong_pin_fails() {
        let mgr = PairingManager::new();
        let _pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin("999999", "phone", vk.as_bytes()).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_verify_expired_pin_fails() {
        let mgr = PairingManager {
            pin_ttl_secs: 0,
            ..Default::default()
        };
        let pin = mgr.start_pairing().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;

        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("PIN expired"));
    }

    #[tokio::test]
    async fn test_list_and_unpair_devices() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;
        let device_id = result.device_id.unwrap();

        let devices = mgr.list_devices().await;
        assert_eq!(devices.len(), 1);

        let removed = mgr.unpair_device(&device_id).await;
        assert!(removed);

        let devices = mgr.list_devices().await;
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_sessions() {
        let mgr = PairingManager {
            pin_ttl_secs: 0,
            ..Default::default()
        };
        mgr.start_pairing().await;
        mgr.start_pairing().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let cleaned = mgr.cleanup_sessions().await;
        assert_eq!(cleaned, 2);
    }

    #[test]
    fn test_generate_pin_format() {
        for _ in 0..100 {
            let pin = generate_pin();
            assert_eq!(pin.len(), 6);
            let num: u32 = pin.parse().unwrap();
            assert!(num >= 100_000 && num < 1_000_000);
        }
    }

    #[tokio::test]
    async fn test_invalid_public_key_length() {
        let mgr = PairingManager::new();
        let pin = mgr.start_pairing().await;

        let result = mgr.verify_pin(&pin, "phone", &[0u8; 16]).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("32 bytes"));
    }

    #[tokio::test]
    async fn test_sqlite_persistence() {
        // Create in-memory SQLite for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mgr = PairingManager::new_with_db(pool.clone()).await.unwrap();

        let pin = mgr.start_pairing().await;
        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "test-device", vk.as_bytes()).await;
        assert!(result.success);

        let device_id = result.device_id.unwrap();

        // Verify device is in SQLite directly
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM paired_devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        // Create a new manager from the same DB (simulates restart)
        let mgr2 = PairingManager::new_with_db(pool).await.unwrap();
        let devices = mgr2.list_devices().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, device_id);
        assert_eq!(devices[0].device_name, "test-device");
    }

    #[tokio::test]
    async fn test_unpair_sqlite() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let mgr = PairingManager::new_with_db(pool.clone()).await.unwrap();

        let pin = mgr.start_pairing().await;
        let (_, vk) = crate::device_auth::generate_device_keypair();
        let result = mgr.verify_pin(&pin, "phone", vk.as_bytes()).await;
        let device_id = result.device_id.unwrap();

        mgr.unpair_device(&device_id).await;

        // Verify device is gone from SQLite
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM paired_devices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, 0);

        // Also gone from memory
        let devices = mgr.list_devices().await;
        assert!(devices.is_empty());
    }
}
