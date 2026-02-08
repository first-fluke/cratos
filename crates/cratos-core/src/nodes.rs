//! Node Registry — remote machine management.
//!
//! Nodes are remote machines that can execute commands. Each node has:
//! - An owner (the user who registered it)
//! - A set of declared commands it can run
//! - A heartbeat mechanism for liveness detection
//!
//! **Security**: Nodes are authenticated via token + ownership verification.
//! Commands are dual-gated by ToolPolicy (denylist + node declaration).

use crate::auth::{AuthContext, Scope};
use crate::device_auth;
// Error types used via NodeError, not the crate-level Error/Result
use crate::tool_policy::{PolicyDenial, ToolPolicy};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Platform type of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// macOS
    Darwin,
    /// Linux
    Linux,
    /// Windows
    Windows,
    /// Unknown/Other
    Other,
}

/// Status of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    /// Node is online and responding to heartbeats
    Online,
    /// Node hasn't sent a heartbeat recently
    Offline,
    /// Node is registered but hasn't connected yet
    Pending,
}

/// A registered remote node.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// Unique node ID
    pub id: Uuid,
    /// Human-readable node name
    pub name: String,
    /// Platform type
    pub platform: Platform,
    /// Node capabilities (e.g., "rust", "python", "docker")
    pub capabilities: Vec<String>,
    /// Commands this node declares it can run
    pub declared_commands: Vec<String>,
    /// Current status
    pub status: NodeStatus,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
    /// WS connection ID (if connected via gateway)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    /// Ed25519 public key for device authentication (32 bytes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<Vec<u8>>,
    /// Owner user ID
    pub owner_user_id: String,
    /// When the node was registered
    pub created_at: DateTime<Utc>,
}

/// Parameters for registering a new node.
#[derive(Debug, Deserialize)]
pub struct NodeRegisterParams {
    /// Human-readable name
    pub name: String,
    /// Platform
    pub platform: Platform,
    /// Capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Commands this node can execute
    #[serde(default)]
    pub declared_commands: Vec<String>,
    /// Ed25519 public key (32 bytes, optional — if absent, token-only auth)
    #[serde(default)]
    pub public_key: Option<Vec<u8>>,
    /// Server-issued challenge (required if public_key is provided)
    #[serde(default)]
    pub challenge: Option<Vec<u8>>,
    /// Ed25519 signature of the challenge (required if public_key is provided)
    #[serde(default)]
    pub signature: Option<Vec<u8>>,
}

/// Summary view for node listings.
#[derive(Debug, Clone, Serialize)]
pub struct NodeSummary {
    /// Unique node ID
    pub id: Uuid,
    /// Human-readable node name
    pub name: String,
    /// Operating system platform
    pub platform: Platform,
    /// Current node status
    pub status: NodeStatus,
    /// Supported capabilities
    pub capabilities: Vec<String>,
    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
}

/// Result of a remote invocation.
#[derive(Debug, Clone, Serialize)]
pub struct InvokeResult {
    /// Whether the command succeeded
    pub success: bool,
    /// Command output (stdout)
    pub stdout: String,
    /// Error output (stderr)
    pub stderr: String,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Duration in ms
    pub duration_ms: u64,
}

/// Node error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeError {
    /// Node not found
    NotFound(Uuid),
    /// Node is offline
    Offline(Uuid),
    /// Command denied by tool policy
    PolicyDenied(PolicyDenial),
    /// Requester not authorized for this node
    Unauthorized,
    /// Ed25519 signature verification failed
    SignatureInvalid(String),
    /// Missing required signature fields when public_key is provided
    SignatureMissing,
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "node {} not found", id),
            Self::Offline(id) => write!(f, "node {} is offline", id),
            Self::PolicyDenied(p) => write!(f, "policy denied: {}", p),
            Self::Unauthorized => write!(f, "not authorized for this node"),
            Self::SignatureInvalid(msg) => write!(f, "signature verification failed: {}", msg),
            Self::SignatureMissing => write!(f, "public_key provided but challenge/signature missing"),
        }
    }
}

impl std::error::Error for NodeError {}

/// Node Registry — manages remote node registration, heartbeat, and invocation.
pub struct NodeRegistry {
    nodes: RwLock<HashMap<Uuid, Node>>,
    policy: ToolPolicy,
    /// Seconds before a node is considered offline
    heartbeat_timeout_secs: i64,
}

impl NodeRegistry {
    /// Create a new registry with default policy.
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            policy: ToolPolicy::default(),
            heartbeat_timeout_secs: 60,
        }
    }

    /// Create with custom tool policy.
    pub fn with_policy(policy: ToolPolicy) -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            policy,
            heartbeat_timeout_secs: 60,
        }
    }

    /// Register a new node.
    ///
    /// If `public_key` is provided, `challenge` and `signature` are required.
    /// The signature is verified using Ed25519 before the node is accepted.
    pub async fn register(
        &self,
        params: NodeRegisterParams,
        auth: &AuthContext,
    ) -> std::result::Result<Node, NodeError> {
        // Verify Ed25519 signature if public key is provided
        let public_key = if let Some(ref pk) = params.public_key {
            let challenge = params.challenge.as_ref().ok_or(NodeError::SignatureMissing)?;
            let signature = params.signature.as_ref().ok_or(NodeError::SignatureMissing)?;

            device_auth::verify_signature(pk, challenge, signature)
                .map_err(|e| NodeError::SignatureInvalid(e.to_string()))?;

            Some(pk.clone())
        } else {
            None
        };

        let now = Utc::now();
        let node = Node {
            id: Uuid::new_v4(),
            name: params.name,
            platform: params.platform,
            capabilities: params.capabilities,
            declared_commands: params.declared_commands,
            status: NodeStatus::Pending,
            last_heartbeat: now,
            connection_id: None,
            public_key,
            owner_user_id: auth.user_id.clone(),
            created_at: now,
        };

        let mut nodes = self.nodes.write().await;
        nodes.insert(node.id, node.clone());
        Ok(node)
    }

    /// Record a heartbeat from a node.
    pub async fn heartbeat(
        &self,
        node_id: Uuid,
        auth: &AuthContext,
    ) -> std::result::Result<(), NodeError> {
        let mut nodes = self.nodes.write().await;
        let node = nodes.get_mut(&node_id).ok_or(NodeError::NotFound(node_id))?;
        self.check_node_ownership(node, auth)?;

        node.last_heartbeat = Utc::now();
        node.status = NodeStatus::Online;
        Ok(())
    }

    /// List nodes visible to the requester.
    pub async fn list_nodes(&self, auth: &AuthContext) -> Vec<NodeSummary> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|n| auth.has_scope(&Scope::Admin) || n.owner_user_id == auth.user_id)
            .map(|n| NodeSummary {
                id: n.id,
                name: n.name.clone(),
                platform: n.platform,
                status: n.status,
                capabilities: n.capabilities.clone(),
                last_heartbeat: n.last_heartbeat,
            })
            .collect()
    }

    /// Get a node by ID with ownership check.
    pub async fn get_node(
        &self,
        node_id: Uuid,
        auth: &AuthContext,
    ) -> std::result::Result<Node, NodeError> {
        let nodes = self.nodes.read().await;
        let node = nodes.get(&node_id).ok_or(NodeError::NotFound(node_id))?;
        self.check_node_ownership(node, auth)?;
        Ok(node.clone())
    }

    /// Check if a command is allowed on a node (policy + declaration).
    pub async fn check_command(
        &self,
        node_id: Uuid,
        command: &str,
        auth: &AuthContext,
    ) -> std::result::Result<(), NodeError> {
        let nodes = self.nodes.read().await;
        let node = nodes.get(&node_id).ok_or(NodeError::NotFound(node_id))?;
        self.check_node_ownership(node, auth)?;

        if node.status == NodeStatus::Offline {
            return Err(NodeError::Offline(node_id));
        }

        self.policy
            .is_allowed(command, &node.declared_commands)
            .map_err(NodeError::PolicyDenied)
    }

    /// Check liveness of all nodes (mark offline if heartbeat stale).
    pub async fn check_liveness(&self) {
        let cutoff =
            Utc::now() - chrono::Duration::seconds(self.heartbeat_timeout_secs);
        let mut nodes = self.nodes.write().await;
        for node in nodes.values_mut() {
            if node.status == NodeStatus::Online && node.last_heartbeat < cutoff {
                node.status = NodeStatus::Offline;
            }
        }
    }

    /// Remove a node.
    pub async fn remove(
        &self,
        node_id: Uuid,
        auth: &AuthContext,
    ) -> std::result::Result<(), NodeError> {
        let nodes = self.nodes.read().await;
        let node = nodes.get(&node_id).ok_or(NodeError::NotFound(node_id))?;
        self.check_node_ownership(node, auth)?;
        drop(nodes);

        self.nodes.write().await.remove(&node_id);
        Ok(())
    }

    fn check_node_ownership(
        &self,
        node: &Node,
        auth: &AuthContext,
    ) -> std::result::Result<(), NodeError> {
        if auth.has_scope(&Scope::Admin) || node.owner_user_id == auth.user_id {
            Ok(())
        } else {
            Err(NodeError::Unauthorized)
        }
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthMethod, Scope};

    fn user_auth(user: &str) -> AuthContext {
        AuthContext {
            user_id: user.to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::NodeManage, Scope::ExecutionWrite],
            session_id: None,
            device_id: None,
        }
    }

    fn admin_auth() -> AuthContext {
        AuthContext {
            user_id: "admin".to_string(),
            method: AuthMethod::ApiKey,
            scopes: vec![Scope::Admin],
            session_id: None,
            device_id: None,
        }
    }

    fn test_params() -> NodeRegisterParams {
        NodeRegisterParams {
            name: "dev-mac".to_string(),
            platform: Platform::Darwin,
            capabilities: vec!["rust".to_string()],
            declared_commands: vec!["git".to_string(), "cargo".to_string(), "ls".to_string()],
            public_key: None,
            challenge: None,
            signature: None,
        }
    }

    #[tokio::test]
    async fn test_register_node() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();
        assert_eq!(node.name, "dev-mac");
        assert_eq!(node.owner_user_id, "alice");
        assert_eq!(node.status, NodeStatus::Pending);
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();

        registry.heartbeat(node.id, &auth).await.unwrap();

        let updated = registry.get_node(node.id, &auth).await.unwrap();
        assert_eq!(updated.status, NodeStatus::Online);
    }

    #[tokio::test]
    async fn test_heartbeat_wrong_user() {
        let registry = NodeRegistry::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        let node = registry.register(test_params(), &alice).await.unwrap();
        let result = registry.heartbeat(node.id, &bob).await;
        assert!(matches!(result, Err(NodeError::Unauthorized)));
    }

    #[tokio::test]
    async fn test_list_nodes_ownership() {
        let registry = NodeRegistry::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        registry.register(test_params(), &alice).await.unwrap();

        let alice_nodes = registry.list_nodes(&alice).await;
        assert_eq!(alice_nodes.len(), 1);

        let bob_nodes = registry.list_nodes(&bob).await;
        assert_eq!(bob_nodes.len(), 0);

        let admin_nodes = registry.list_nodes(&admin_auth()).await;
        assert_eq!(admin_nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_check_command_allowed() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();
        registry.heartbeat(node.id, &auth).await.unwrap();

        assert!(registry.check_command(node.id, "git status", &auth).await.is_ok());
        assert!(registry.check_command(node.id, "cargo build", &auth).await.is_ok());
    }

    #[tokio::test]
    async fn test_check_command_denied_not_declared() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();
        registry.heartbeat(node.id, &auth).await.unwrap();

        let result = registry.check_command(node.id, "npm install", &auth).await;
        assert!(matches!(result, Err(NodeError::PolicyDenied(PolicyDenial::NotDeclared(_)))));
    }

    #[tokio::test]
    async fn test_check_command_denied_by_policy() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let mut params = test_params();
        params.declared_commands.push("dd".to_string());
        let node = registry.register(params, &auth).await.unwrap();
        registry.heartbeat(node.id, &auth).await.unwrap();

        let result = registry.check_command(node.id, "dd if=/dev/zero", &auth).await;
        assert!(matches!(result, Err(NodeError::PolicyDenied(PolicyDenial::DenyListed(_)))));
    }

    #[tokio::test]
    async fn test_offline_node_rejects_commands() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();
        // Don't heartbeat — stays Pending (treated as offline for commands)

        // Status is Pending, not Offline, so we need to actually test offline
        registry.heartbeat(node.id, &auth).await.unwrap();

        // Force offline by manipulating liveness
        {
            let mut nodes = registry.nodes.write().await;
            nodes.get_mut(&node.id).unwrap().status = NodeStatus::Offline;
        }

        let result = registry.check_command(node.id, "git status", &auth).await;
        assert!(matches!(result, Err(NodeError::Offline(_))));
    }

    #[tokio::test]
    async fn test_remove_node() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");
        let node = registry.register(test_params(), &auth).await.unwrap();

        registry.remove(node.id, &auth).await.unwrap();

        let result = registry.get_node(node.id, &auth).await;
        assert!(matches!(result, Err(NodeError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_remove_node_wrong_user() {
        let registry = NodeRegistry::new();
        let alice = user_auth("alice");
        let bob = user_auth("bob");

        let node = registry.register(test_params(), &alice).await.unwrap();
        let result = registry.remove(node.id, &bob).await;
        assert!(matches!(result, Err(NodeError::Unauthorized)));
    }

    #[tokio::test]
    async fn test_register_with_valid_signature() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");

        let (signing_key, verifying_key) = device_auth::generate_device_keypair();
        let challenge = device_auth::generate_challenge();
        let signature = device_auth::sign_challenge(&signing_key, &challenge);

        let params = NodeRegisterParams {
            name: "signed-node".to_string(),
            platform: Platform::Darwin,
            capabilities: vec![],
            declared_commands: vec!["git".to_string()],
            public_key: Some(verifying_key.as_bytes().to_vec()),
            challenge: Some(challenge.to_vec()),
            signature: Some(signature),
        };

        let node = registry.register(params, &auth).await.unwrap();
        assert_eq!(node.name, "signed-node");
        assert!(node.public_key.is_some());
    }

    #[tokio::test]
    async fn test_register_with_invalid_signature() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");

        let (_, verifying_key) = device_auth::generate_device_keypair();
        let challenge = device_auth::generate_challenge();

        let params = NodeRegisterParams {
            name: "bad-node".to_string(),
            platform: Platform::Linux,
            capabilities: vec![],
            declared_commands: vec![],
            public_key: Some(verifying_key.as_bytes().to_vec()),
            challenge: Some(challenge.to_vec()),
            signature: Some(vec![0u8; 64]), // Invalid signature
        };

        let result = registry.register(params, &auth).await;
        assert!(matches!(result, Err(NodeError::SignatureInvalid(_))));
    }

    #[tokio::test]
    async fn test_register_with_key_but_no_signature() {
        let registry = NodeRegistry::new();
        let auth = user_auth("alice");

        let (_, verifying_key) = device_auth::generate_device_keypair();

        let params = NodeRegisterParams {
            name: "incomplete-node".to_string(),
            platform: Platform::Linux,
            capabilities: vec![],
            declared_commands: vec![],
            public_key: Some(verifying_key.as_bytes().to_vec()),
            challenge: None,
            signature: None,
        };

        let result = registry.register(params, &auth).await;
        assert!(matches!(result, Err(NodeError::SignatureMissing)));
    }
}
