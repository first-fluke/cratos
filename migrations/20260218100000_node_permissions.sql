-- Create nodes table
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY, -- UUID as string
    device_id TEXT NOT NULL UNIQUE, -- Physical Hardware ID
    name TEXT NOT NULL,
    platform TEXT NOT NULL,
    capabilities TEXT NOT NULL, -- JSON string of capabilities
    declared_commands TEXT NOT NULL, -- JSON string of commands
    public_key TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    status TEXT NOT NULL,
    registered_at TEXT NOT NULL, -- ISO8601 string or timestamp
    last_seen TEXT NOT NULL,
    is_blocked BOOLEAN NOT NULL DEFAULT 0
);

-- Create node_approvals table
CREATE TABLE IF NOT EXISTS node_approvals (
    device_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    approved_at TEXT NOT NULL,
    expires_at TEXT, -- NULL for no expiration
    PRIMARY KEY (device_id, capability),
    FOREIGN KEY (device_id) REFERENCES nodes(device_id) ON DELETE CASCADE
);
