//! mDNS service discovery for LAN device pairing.
//!
//! Advertises `_cratos._tcp.local.` so mobile/desktop clients
//! can discover the server automatically on the local network.

use std::sync::Arc;
#[cfg(feature = "mdns")]
use tracing::info;
use tracing::warn;

/// Service type for mDNS advertisement.
pub const SERVICE_TYPE: &str = "_cratos._tcp.local.";

/// Configuration for mDNS service discovery.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Whether mDNS is enabled.
    pub enabled: bool,
    /// Human-readable service name shown to clients.
    pub service_name: String,
    /// Instance name (defaults to hostname).
    pub instance_name: String,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        let instance = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "cratos".to_string());
        Self {
            enabled: true,
            service_name: "Cratos AI".to_string(),
            instance_name: instance,
        }
    }
}

/// mDNS service discovery handle.
///
/// Registers the Cratos server on the local network so that clients
/// (mobile apps, CLI on other machines) can discover it automatically.
pub struct DiscoveryService {
    #[allow(dead_code)] // Used only when mdns feature is enabled
    config: DiscoveryConfig,
    #[cfg(feature = "mdns")]
    daemon: std::sync::Mutex<Option<mdns_sd::ServiceDaemon>>,
}

impl DiscoveryService {
    /// Create a new discovery service.
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "mdns")]
            daemon: std::sync::Mutex::new(None),
        }
    }

    /// Start advertising the service on the given port.
    ///
    /// TXT records include `api_version=v1` and `pair_endpoint=/api/v1/pair/start`.
    #[cfg(feature = "mdns")]
    pub fn start(&self, port: u16) -> crate::Result<()> {
        if !self.config.enabled {
            info!("mDNS discovery disabled by configuration");
            return Ok(());
        }

        let daemon = mdns_sd::ServiceDaemon::new()
            .map_err(|e| crate::Error::Internal(format!("mDNS daemon init failed: {e}")))?;

        let mut properties = std::collections::HashMap::new();
        properties.insert("api_version".to_string(), "v1".to_string());
        properties.insert(
            "pair_endpoint".to_string(),
            "/api/v1/pair/start".to_string(),
        );
        properties.insert("name".to_string(), self.config.service_name.clone());

        let service_info = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            &self.config.instance_name,
            &format!("{}.", self.config.instance_name),
            "",
            port,
            properties,
        )
        .map_err(|e| crate::Error::Internal(format!("mDNS ServiceInfo creation failed: {e}")))?;

        daemon
            .register(service_info)
            .map_err(|e| crate::Error::Internal(format!("mDNS register failed: {e}")))?;

        info!(
            service_type = SERVICE_TYPE,
            instance = %self.config.instance_name,
            port = port,
            "mDNS service registered"
        );

        let mut guard = self
            .daemon
            .lock()
            .map_err(|_| crate::Error::Internal("mDNS daemon lock poisoned".to_string()))?;
        *guard = Some(daemon);

        Ok(())
    }

    /// Start advertising (no-op when mdns feature is disabled).
    #[cfg(not(feature = "mdns"))]
    pub fn start(&self, _port: u16) -> crate::Result<()> {
        warn!("mDNS discovery requested but 'mdns' feature is not enabled");
        Ok(())
    }

    /// Stop the mDNS service.
    #[cfg(feature = "mdns")]
    pub fn stop(&self) {
        let mut guard = match self.daemon.lock() {
            Ok(g) => g,
            Err(e) => {
                warn!("mDNS daemon lock poisoned during stop: {}", e);
                return;
            }
        };
        if let Some(daemon) = guard.take() {
            let _ = daemon.shutdown();
            info!("mDNS service unregistered");
        }
    }

    /// Stop (no-op when mdns feature is disabled).
    #[cfg(not(feature = "mdns"))]
    pub fn stop(&self) {}

    /// Check if the service is currently running.
    pub fn is_running(&self) -> bool {
        #[cfg(feature = "mdns")]
        {
            match self.daemon.lock() {
                Ok(guard) => guard.is_some(),
                Err(_) => false,
            }
        }
        #[cfg(not(feature = "mdns"))]
        {
            false
        }
    }
}

/// Shared discovery service handle.
pub type SharedDiscoveryService = Arc<DiscoveryService>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DiscoveryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.service_name, "Cratos AI");
        assert!(!config.instance_name.is_empty());
    }

    #[test]
    fn test_service_type() {
        assert_eq!(SERVICE_TYPE, "_cratos._tcp.local.");
    }

    #[test]
    fn test_discovery_service_new() {
        let config = DiscoveryConfig {
            enabled: false,
            service_name: "Test".to_string(),
            instance_name: "test-host".to_string(),
        };
        let svc = DiscoveryService::new(config);
        assert!(!svc.is_running());
    }
}
