//! Wake-on-LAN Tool
//!
//! Sends magic packets to wake up devices on the local network.
//! Supports both MAC addresses and named devices from configuration.

use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::Instant;
use tracing::{debug, info};

/// Default WoL port
const DEFAULT_WOL_PORT: u16 = 9;

/// Broadcast address for WoL
const BROADCAST_ADDR: &str = "255.255.255.255";

/// Magic packet header (6 bytes of 0xFF)
const MAGIC_HEADER: [u8; 6] = [0xFF; 6];

/// Magic packet size (6 header + 16*6 MAC repetitions = 102 bytes)
const MAGIC_PACKET_SIZE: usize = 102;

/// Wake-on-LAN tool
pub struct WolTool {
    definition: ToolDefinition,
    /// Named devices from configuration (name -> MAC address)
    devices: HashMap<String, String>,
}

impl WolTool {
    /// Create a new WoL tool without named devices
    #[must_use]
    pub fn new() -> Self {
        Self::with_devices(HashMap::new())
    }

    /// Create a WoL tool with named devices from configuration
    ///
    /// # Arguments
    /// * `devices` - Map of device names to MAC addresses
    ///   e.g., {"원격피씨": "AA:BB:CC:DD:EE:FF", "서버": "11:22:33:44:55:66"}
    #[must_use]
    pub fn with_devices(devices: HashMap<String, String>) -> Self {
        // Build device description for LLM
        let device_desc = if devices.is_empty() {
            String::new()
        } else {
            let device_list: Vec<String> =
                devices.keys().map(|name| format!("'{}'", name)).collect();
            format!(
                ". Available named devices: {}. You can use device name instead of MAC address.",
                device_list.join(", ")
            )
        };

        let description = format!(
            "Send Wake-on-LAN magic packet to wake up a device{}",
            device_desc
        );

        let mac_desc = if devices.is_empty() {
            "MAC address of the target device (e.g., 'AA:BB:CC:DD:EE:FF' or 'AA-BB-CC-DD-EE-FF')"
                .to_string()
        } else {
            let device_list: Vec<String> =
                devices.keys().map(|name| format!("'{}'", name)).collect();
            format!(
                "MAC address OR device name. Available devices: {}. Examples: 'AA:BB:CC:DD:EE:FF' or '원격피씨'",
                device_list.join(", ")
            )
        };

        let definition = ToolDefinition::new("wol", &description)
            .with_parameters(serde_json::json!({
                "type": "object",
                "properties": {
                    "mac_address": {
                        "type": "string",
                        "description": mac_desc
                    },
                    "broadcast_address": {
                        "type": "string",
                        "description": "Optional broadcast address (default: 255.255.255.255)"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Optional WoL port (default: 9)"
                    }
                },
                "required": ["mac_address"]
            }))
            .with_risk_level(RiskLevel::Medium)
            .with_category(ToolCategory::Utility);

        Self {
            definition,
            devices,
        }
    }

    /// Resolve device name to MAC address
    fn resolve_mac(&self, input: &str) -> Option<String> {
        // First check if it's a known device name
        if let Some(mac) = self.devices.get(input) {
            return Some(mac.clone());
        }
        // Also check case-insensitive
        for (name, mac) in &self.devices {
            if name.eq_ignore_ascii_case(input) {
                return Some(mac.clone());
            }
        }
        // Not a device name, return as-is (might be MAC address)
        None
    }
}

impl Default for WolTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Input parameters for WoL
#[derive(Debug, Deserialize)]
struct WolInput {
    mac_address: String,
    #[serde(default)]
    broadcast_address: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

/// Parse a MAC address string into bytes
///
/// Accepts formats:
/// - AA:BB:CC:DD:EE:FF (colon-separated)
/// - AA-BB-CC-DD-EE-FF (dash-separated)
/// - AABBCCDDEEFF (no separator)
fn parse_mac_address(mac_str: &str) -> Result<[u8; 6]> {
    let cleaned: String = mac_str.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    if cleaned.len() != 12 {
        return Err(Error::InvalidInput(format!(
            "Invalid MAC address '{}'. Expected 12 hex digits (e.g., 'AA:BB:CC:DD:EE:FF')",
            mac_str
        )));
    }

    let mut mac = [0u8; 6];
    for i in 0..6 {
        let hex = &cleaned[i * 2..i * 2 + 2];
        mac[i] = u8::from_str_radix(hex, 16)
            .map_err(|_| Error::InvalidInput(format!("Invalid hex in MAC address: '{}'", hex)))?;
    }

    Ok(mac)
}

/// Create a magic packet for Wake-on-LAN
///
/// The packet consists of:
/// - 6 bytes of 0xFF (header)
/// - 16 repetitions of the target MAC address (96 bytes)
/// - Total: 102 bytes
fn create_magic_packet(mac: &[u8; 6]) -> [u8; MAGIC_PACKET_SIZE] {
    let mut packet = [0u8; MAGIC_PACKET_SIZE];

    // Write header (6 bytes of 0xFF)
    packet[..6].copy_from_slice(&MAGIC_HEADER);

    // Write MAC address 16 times
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(mac);
    }

    packet
}

/// Send a Wake-on-LAN magic packet
fn send_wol_packet(mac: &[u8; 6], broadcast: &str, port: u16) -> Result<()> {
    let packet = create_magic_packet(mac);
    let destination = format!("{}:{}", broadcast, port);

    debug!(
        mac = %format_mac(mac),
        destination = %destination,
        "Sending WoL magic packet"
    );

    // Create UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| Error::Network(format!("Failed to create UDP socket: {}", e)))?;

    // Enable broadcast
    socket
        .set_broadcast(true)
        .map_err(|e| Error::Network(format!("Failed to enable broadcast: {}", e)))?;

    // Send the magic packet
    socket
        .send_to(&packet, &destination)
        .map_err(|e| Error::Network(format!("Failed to send magic packet: {}", e)))?;

    info!(
        mac = %format_mac(mac),
        destination = %destination,
        "WoL magic packet sent"
    );

    Ok(())
}

/// Format MAC address bytes as a string
fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

#[async_trait::async_trait]
impl Tool for WolTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let start = Instant::now();

        // Parse input
        let params: WolInput = serde_json::from_value(input)
            .map_err(|e| Error::InvalidInput(format!("Invalid WoL parameters: {}", e)))?;

        // Resolve device name to MAC address if needed
        let mac_str = self
            .resolve_mac(&params.mac_address)
            .unwrap_or_else(|| params.mac_address.clone());

        // Parse MAC address
        let mac = parse_mac_address(&mac_str).map_err(|e| {
            if self.devices.is_empty() {
                e
            } else {
                let device_list: Vec<&str> = self.devices.keys().map(|s| s.as_str()).collect();
                Error::InvalidInput(format!(
                    "Unknown device '{}'. Available devices: {}. Or provide a MAC address like 'AA:BB:CC:DD:EE:FF'",
                    params.mac_address,
                    device_list.join(", ")
                ))
            }
        })?;

        // Get broadcast address and port
        let broadcast = params
            .broadcast_address
            .as_deref()
            .unwrap_or(BROADCAST_ADDR);
        let port = params.port.unwrap_or(DEFAULT_WOL_PORT);

        // Validate broadcast address format
        if broadcast.parse::<std::net::IpAddr>().is_err() {
            return Err(Error::InvalidInput(format!(
                "Invalid broadcast address: '{}'",
                broadcast
            )));
        }

        // Send the magic packet
        send_wol_packet(&mac, broadcast, port)?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ToolResult::success(
            serde_json::json!({
                "status": "sent",
                "mac_address": format_mac(&mac),
                "broadcast_address": broadcast,
                "port": port,
                "message": format!(
                    "Wake-on-LAN magic packet sent to {} via {}:{}",
                    format_mac(&mac),
                    broadcast,
                    port
                )
            }),
            duration_ms,
        ))
    }

    fn validate_input(&self, input: &serde_json::Value) -> Result<()> {
        if !input.is_object() {
            return Err(Error::InvalidInput("Input must be an object".to_string()));
        }

        let mac_or_device = input
            .get("mac_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("mac_address is required".to_string()))?;

        // Check if it's a known device name
        if self.resolve_mac(mac_or_device).is_some() {
            return Ok(());
        }

        // Validate MAC address format
        parse_mac_address(mac_or_device)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mac_colon_separated() {
        let mac = parse_mac_address("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_dash_separated() {
        let mac = parse_mac_address("AA-BB-CC-DD-EE-FF").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_no_separator() {
        let mac = parse_mac_address("AABBCCDDEEFF").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_lowercase() {
        let mac = parse_mac_address("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_invalid() {
        assert!(parse_mac_address("invalid").is_err());
        assert!(parse_mac_address("AA:BB:CC").is_err());
        assert!(parse_mac_address("AA:BB:CC:DD:EE:GG").is_err());
    }

    #[test]
    fn test_create_magic_packet() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let packet = create_magic_packet(&mac);

        // Check header
        assert_eq!(&packet[0..6], &[0xFF; 6]);

        // Check MAC repetitions
        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }

        // Check total size
        assert_eq!(packet.len(), 102);
    }

    #[test]
    fn test_format_mac() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        assert_eq!(format_mac(&mac), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn test_wol_tool_definition() {
        let tool = WolTool::new();
        assert_eq!(tool.definition().name, "wol");
        assert_eq!(tool.definition().risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_wol_tool_with_devices() {
        let mut devices = HashMap::new();
        devices.insert("원격피씨".to_string(), "AA:BB:CC:DD:EE:FF".to_string());
        devices.insert("서버".to_string(), "11:22:33:44:55:66".to_string());

        let tool = WolTool::with_devices(devices);

        // Check that device names are in the description
        let desc = &tool.definition().description;
        assert!(desc.contains("원격피씨"));
        assert!(desc.contains("서버"));
    }

    #[test]
    fn test_resolve_device_name() {
        let mut devices = HashMap::new();
        devices.insert("원격피씨".to_string(), "AA:BB:CC:DD:EE:FF".to_string());
        devices.insert("MyServer".to_string(), "11:22:33:44:55:66".to_string());

        let tool = WolTool::with_devices(devices);

        // Exact match
        assert_eq!(
            tool.resolve_mac("원격피씨"),
            Some("AA:BB:CC:DD:EE:FF".to_string())
        );

        // Case-insensitive match
        assert_eq!(
            tool.resolve_mac("myserver"),
            Some("11:22:33:44:55:66".to_string())
        );

        // Unknown device returns None
        assert_eq!(tool.resolve_mac("unknown"), None);

        // MAC address returns None (not a device name)
        assert_eq!(tool.resolve_mac("AA:BB:CC:DD:EE:FF"), None);
    }

    #[tokio::test]
    async fn test_wol_validate_input() {
        let tool = WolTool::new();

        // Valid input
        let valid_input = serde_json::json!({
            "mac_address": "AA:BB:CC:DD:EE:FF"
        });
        assert!(tool.validate_input(&valid_input).is_ok());

        // Invalid input - missing mac_address
        let invalid_input = serde_json::json!({});
        assert!(tool.validate_input(&invalid_input).is_err());

        // Invalid input - bad MAC format
        let invalid_mac = serde_json::json!({
            "mac_address": "invalid"
        });
        assert!(tool.validate_input(&invalid_mac).is_err());
    }

    #[tokio::test]
    async fn test_wol_validate_with_device_name() {
        let mut devices = HashMap::new();
        devices.insert("원격피씨".to_string(), "AA:BB:CC:DD:EE:FF".to_string());

        let tool = WolTool::with_devices(devices);

        // Valid input with device name
        let valid_input = serde_json::json!({
            "mac_address": "원격피씨"
        });
        assert!(tool.validate_input(&valid_input).is_ok());

        // Valid input with MAC address
        let valid_mac = serde_json::json!({
            "mac_address": "11:22:33:44:55:66"
        });
        assert!(tool.validate_input(&valid_mac).is_ok());

        // Invalid - unknown device name (not a valid MAC either)
        let invalid = serde_json::json!({
            "mac_address": "unknown_device"
        });
        assert!(tool.validate_input(&invalid).is_err());
    }
}
