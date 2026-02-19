use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

use crate::auth::{AuthContext, Scope};
use crate::nodes::crypto;
use crate::nodes::types::*;
use crate::tool_policy::ToolPolicy;

/// Registry for managing permitted nodes and their sessions.
/// Persists node state in SQLite and manages active WebSocket sessions.
pub struct NodeRegistry {
    db: Pool<Sqlite>,
    sessions: Arc<DashMap<String, NodeSession>>, // device_id -> session
    policy: ToolPolicy,
    heartbeat_timeout_secs: i64,
}

struct NodeSession {
    pub connection_id: Option<String>,
}

impl NodeRegistry {
    /// Create a new NodeRegistry with the given database pool.
    pub fn new(db: Pool<Sqlite>) -> Self {
        Self {
            db,
            sessions: Arc::new(DashMap::new()),
            policy: ToolPolicy::default(),
            heartbeat_timeout_secs: 60,
        }
    }

    /// Set a custom tool policy.
    pub fn with_policy(mut self, policy: ToolPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Register a new node or update an existing one.
    pub async fn register(
        &self,
        params: NodeRegisterParams,
        auth: &AuthContext,
    ) -> Result<Node, NodeError> {
        // Verify signature if needed
        if !params.public_key.is_empty() {
            crypto::verify_signature(&params.public_key, &params.challenge, &params.signature)
                .map_err(NodeError::SignatureInvalid)?;
        }

        let node_id = Uuid::new_v4();
        let now = Utc::now();

        let capabilities_json = serde_json::to_string(&params.capabilities).map_err(|e| {
            NodeError::DatabaseError(format!("Failed to serialize capabilities: {}", e))
        })?;
        let commands_json = serde_json::to_string(&params.declared_commands).map_err(|e| {
            NodeError::DatabaseError(format!("Failed to serialize commands: {}", e))
        })?;
        // Simple serialization to string, removing quotes
        let platform_str = serde_json::to_string(&params.platform)
            .map_err(|e| NodeError::DatabaseError(format!("Failed to serialize platform: {}", e)))?
            .replace("\"", "");

        sqlx::query(
            r#"
            INSERT INTO nodes (id, device_id, name, platform, capabilities, declared_commands, public_key, owner_user_id, status, registered_at, last_seen)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT(device_id) DO UPDATE SET
                name = excluded.name,
                capabilities = excluded.capabilities,
                declared_commands = excluded.declared_commands,
                public_key = excluded.public_key,
                last_seen = excluded.last_seen
            "#
        )
        .bind(node_id.to_string())
        .bind(&params.device_id)
        .bind(&params.name)
        .bind(&platform_str)
        .bind(&capabilities_json)
        .bind(&commands_json)
        .bind(&params.public_key)
        .bind(&auth.user_id)
        .bind("pending") // Initial status
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.db)
        .await
        .map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        // Verify and return the node
        self.get_node_by_device_id(&params.device_id, auth).await
    }

    /// Process a heartbeat from a node.
    pub async fn heartbeat(&self, node_id: Uuid, auth: &AuthContext) -> Result<(), NodeError> {
        let _node = self.get_node(node_id, auth).await?;
        let now = Utc::now();

        sqlx::query("UPDATE nodes SET last_seen = $1, status = 'online' WHERE id = $2")
            .bind(now.to_rfc3339())
            .bind(node_id.to_string())
            .execute(&self.db)
            .await
            .map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// List all registered nodes visible to the user.
    pub async fn list_nodes(&self, auth: &AuthContext) -> Result<Vec<NodeSummary>, NodeError> {
        let is_admin = auth.has_scope(&Scope::Admin);

        let rows = if is_admin {
            sqlx::query("SELECT id, name, platform, status, capabilities, last_seen FROM nodes")
                .fetch_all(&self.db)
                .await
        } else {
             sqlx::query("SELECT id, name, platform, status, capabilities, last_seen FROM nodes WHERE owner_user_id = $1")
                .bind(&auth.user_id)
                .fetch_all(&self.db)
                .await
        }.map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        let mut summaries = Vec::new();
        for row in rows {
            let id_str: String = row.get("id");
            let cap_str: String = row.get("capabilities");
            let last_seen_str: String = row.get("last_seen");
            let platform_str: String = row.get("platform");
            let status_str: String = row.get("status");

            // Helper to parse JSON safely
            let platform =
                serde_json::from_str(&format!("\"{}\"", platform_str)).unwrap_or(Platform::Other);

            let capabilities = serde_json::from_str(&cap_str).unwrap_or_default();

            let last_heartbeat = DateTime::parse_from_rfc3339(&last_seen_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            summaries.push(NodeSummary {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get("name"),
                platform,
                status: match status_str.as_str() {
                    "online" => NodeStatus::Online,
                    "offline" => NodeStatus::Offline,
                    _ => NodeStatus::Pending,
                },
                capabilities,
                last_heartbeat,
            });
        }

        Ok(summaries)
    }

    /// Get detailed information about a node.
    pub async fn get_node(&self, node_id: Uuid, auth: &AuthContext) -> Result<Node, NodeError> {
        let row = sqlx::query("SELECT * FROM nodes WHERE id = $1")
            .bind(node_id.to_string())
            .fetch_optional(&self.db)
            .await
            .map_err(|e| NodeError::DatabaseError(e.to_string()))?
            .ok_or(NodeError::NotFound(node_id))?;

        let owner_id: String = row.get("owner_user_id");
        if !auth.has_scope(&Scope::Admin) && owner_id != auth.user_id {
            return Err(NodeError::Unauthorized);
        }

        self.map_row_to_node(&row)
    }

    async fn get_node_by_device_id(
        &self,
        device_id: &str,
        auth: &AuthContext,
    ) -> Result<Node, NodeError> {
        let row = sqlx::query("SELECT * FROM nodes WHERE device_id = $1")
            .bind(device_id)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| NodeError::DatabaseError(e.to_string()))?
            .ok_or(NodeError::NotFound(Uuid::nil()))?; // TODO: Improve error

        let owner_id: String = row.get("owner_user_id");
        if !auth.has_scope(&Scope::Admin) && owner_id != auth.user_id {
            return Err(NodeError::Unauthorized);
        }

        self.map_row_to_node(&row)
    }

    fn map_row_to_node(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Node, NodeError> {
        let id_str: String = row.get("id");
        let cap_str: String = row.get("capabilities");
        let cmds_str: String = row.get("declared_commands");
        let platform_str: String = row.get("platform");
        let status_str: String = row.get("status");
        let registered_at_str: String = row.get("registered_at");
        let last_seen_str: String = row.get("last_seen");
        let device_id: String = row.get("device_id");

        Ok(Node {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            device_id: device_id.clone(),
            name: row.get("name"),
            platform: serde_json::from_str(&format!("\"{}\"", platform_str))
                .unwrap_or(Platform::Other),
            capabilities: serde_json::from_str(&cap_str).unwrap_or_default(),
            declared_commands: serde_json::from_str(&cmds_str).unwrap_or_default(),
            public_key: row.get("public_key"),
            owner_user_id: row.get("owner_user_id"),
            status: match status_str.as_str() {
                "online" => NodeStatus::Online,
                "offline" => NodeStatus::Offline,
                _ => NodeStatus::Pending,
            },
            registered_at: DateTime::parse_from_rfc3339(&registered_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_seen: DateTime::parse_from_rfc3339(&last_seen_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            connection_id: self
                .sessions
                .get(&device_id)
                .and_then(|s| s.connection_id.clone()),
        })
    }

    /// Check if a command is allowed to be executed on a node.
    pub async fn check_command(
        &self,
        node_id: Uuid,
        command: &str,
        auth: &AuthContext,
    ) -> Result<(), NodeError> {
        let node = self.get_node(node_id, auth).await?;

        if node.status != NodeStatus::Online {
            return Err(NodeError::Unauthorized);
        }

        // Check declared commands
        self.policy
            .is_allowed(command, &node.declared_commands)
            .map_err(|e| NodeError::PolicyDenied(e.to_string()))?;

        Ok(())
    }

    /// Remove a node from the registry.
    pub async fn remove(&self, node_id: Uuid, auth: &AuthContext) -> Result<(), NodeError> {
        let node = self.get_node(node_id, auth).await?;

        sqlx::query("DELETE FROM nodes WHERE id = $1")
            .bind(node_id.to_string())
            .execute(&self.db)
            .await
            .map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        self.sessions.remove(&node.device_id);
        Ok(())
    }

    /// Approve a pending node, allowing it to execute commands.
    pub async fn approve(&self, node_id: Uuid, auth: &AuthContext) -> Result<(), NodeError> {
        let _node = self.get_node(node_id, auth).await?;

        sqlx::query("UPDATE nodes SET status = 'online' WHERE id = $1")
            .bind(node_id.to_string())
            .execute(&self.db)
            .await
            .map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Mark nodes as offline if they haven't sent a heartbeat recently.
    pub async fn check_liveness(&self) -> Result<(), NodeError> {
        let cutoff = Utc::now() - chrono::Duration::seconds(self.heartbeat_timeout_secs);

        sqlx::query(
            "UPDATE nodes SET status = 'offline' WHERE status = 'online' AND last_seen < $1",
        )
        .bind(cutoff.to_rfc3339())
        .execute(&self.db)
        .await
        .map_err(|e| NodeError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
