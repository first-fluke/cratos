use std::collections::HashSet;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Capabilities that a node can possess and expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCapability {
    /// Access to the device camera
    Camera,
    /// Access to the device microphone
    Microphone,
    /// Ability to capture screenshots
    ScreenCapture,
    /// Ability to record the screen
    ScreenRecord,
    /// Access to geolocation
    Location,
    /// Ability to send system notifications
    Notification,
    /// Read access to clipboard
    ClipboardRead,
    /// Write access to clipboard
    ClipboardWrite,
    /// Access to filesystem (read/write depending on policy)
    FileSystem,
    /// Execute arbitrary shell commands
    Execute,
    /// Control other applications
    AppControl,
}

impl NodeCapability {
    /// Get the sensitivity level of the capability (1-7).
    /// Higher values require more strict permissions.
    pub fn sensitivity_level(&self) -> u8 {
        match self {
            Self::Notification => 1,
            Self::ClipboardRead | Self::ClipboardWrite => 2,
            Self::FileSystem => 3,
            Self::Location => 4,
            Self::Camera | Self::Microphone => 5,
            Self::ScreenCapture | Self::ScreenRecord => 6,
            Self::Execute | Self::AppControl => 7,
        }
    }

    /// Check if the capability typically requires explicit user approval.
    pub fn requires_approval(&self) -> bool {
        self.sensitivity_level() >= 4
    }
}

/// Operating system platform of the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Apple macOS
    MacOS,
    /// Apple iOS
    IOS,
    /// Google Android
    Android,
    /// GNU/Linux
    Linux,
    /// Microsoft Windows
    Windows,
    /// Web Browser
    Web,
    /// Other or unknown platform
    Other,
}

impl Default for Platform {
    fn default() -> Self {
        Self::Other
    }
}

/// Connection and approval status of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    /// Connected and approved
    Online,
    /// Disconnected or timed out
    Offline,
    /// Waiting for admin approval
    Pending,
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A registered device node within the Cratos network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for the node record
    pub id: Uuid,
    /// Physical Device ID (Hardware UUID)
    pub device_id: String,
    /// Human-readable name of the node
    pub name: String,
    /// Operating system platform
    pub platform: Platform,
    /// List of supported capabilities
    pub capabilities: HashSet<NodeCapability>,
    /// List of CLI commands this node declares support for
    pub declared_commands: Vec<String>,
    /// Public key for signature verification
    pub public_key: String,
    /// User ID of the node owner
    pub owner_user_id: String,
    /// Current status (Online, Offline, Pending)
    pub status: NodeStatus,
    /// When the node was first registered
    pub registered_at: DateTime<Utc>,
    /// When the node last sent a heartbeat
    pub last_seen: DateTime<Utc>,
    /// Active WebSocket connection ID (internal use only)
    #[serde(skip)]
    pub connection_id: Option<String>,
}

/// Errors related to node operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeError {
    /// Node was not found in the registry
    NotFound(Uuid),
    /// Node is currently offline
    Offline(Uuid),
    /// Operation denied by security policy
    PolicyDenied(String),
    /// Caller is not authorized to access this node
    Unauthorized,
    /// Node signature verification failed
    SignatureInvalid(String),
    /// Missing required signature
    SignatureMissing,
    /// Underlying database error
    DatabaseError(String),
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeError::NotFound(id) => write!(f, "node {} not found", id),
            NodeError::Offline(id) => write!(f, "node {} is offline", id),
            NodeError::PolicyDenied(p) => write!(f, "policy denied: {}", p),
            NodeError::Unauthorized => write!(f, "unauthorized"),
            NodeError::SignatureInvalid(msg) => write!(f, "signature invalid: {}", msg),
            NodeError::SignatureMissing => write!(f, "signature missing"),
            NodeError::DatabaseError(msg) => write!(f, "database error: {}", msg),
        }
    }
}

impl std::error::Error for NodeError {}

/// Server -> Node Message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodeMessage {
    /// Request node to execute a capability
    InvokeCapability {
        /// Unique ID for this request
        request_id: Uuid,
        /// Capability to invoke
        capability: NodeCapability,
        /// Parameters for the capability
        params: Value,
        /// Optional approval token if required
        approval_token: Option<String>,
    },
    /// Request node to list its capabilities
    ListCapabilities,
    /// Periodic heartbeat ping
    Heartbeat {
        /// Server timestamp
        timestamp: DateTime<Utc>,
    },
    /// Signal to disconnect with reason
    Disconnect {
        /// Reason for disconnection
        reason: String,
    },
}

/// Node -> Server Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodeResponse {
    /// Result of a capability invocation
    CapabilityResult {
        /// Corresponding request ID
        request_id: Uuid,
        /// Whether the operation succeeded
        success: bool,
        /// Return data if successful
        data: Option<Value>,
        /// Error message if failed
        error: Option<String>,
    },
    /// List of capabilities supported by the node
    Capabilities {
        /// All supported capabilities
        capabilities: Vec<NodeCapability>,
        /// Subset of capabilities that are granted/active
        granted: Vec<NodeCapability>,
    },
    /// Acknowledgment of heartbeat
    HeartbeatAck {
        /// Server timestamp echoed back
        timestamp: DateTime<Utc>,
    },
    /// Request permission for a capability
    PermissionRequest {
        /// Request ID
        request_id: Uuid,
        /// Capability requested
        capability: NodeCapability,
        /// Reason for request
        reason: String,
    },
}

/// Parameters required to register a new node.
#[derive(Debug, Deserialize)]
pub struct NodeRegisterParams {
    /// Human-readable name
    pub name: String,
    /// OS Platform
    pub platform: Platform,
    /// Supported capabilities
    pub capabilities: HashSet<NodeCapability>,
    /// List of commands this node can run
    pub declared_commands: Vec<String>,
    /// Unique hardware ID
    pub device_id: String,
    /// Public key for auth
    pub public_key: String,
    /// Cryptographic signature of the challenge
    pub signature: String,
    /// Challenge string signed by the node
    pub challenge: String,
}

/// Summary information for a node (used in list views).
#[derive(Debug, Clone, Serialize)]
pub struct NodeSummary {
    /// Node UUID
    pub id: Uuid,
    /// Node Name
    pub name: String,
    /// OS Platform
    pub platform: Platform,
    /// Connection Status
    pub status: NodeStatus,
    /// Supported Capabilities
    pub capabilities: HashSet<NodeCapability>,
    /// Last Heartbeat Timestamp
    pub last_heartbeat: DateTime<Utc>,
}
