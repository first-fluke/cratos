-- Cratos Initial Schema
-- This migration creates the core tables for the Cratos AI assistant

-- Enable UUID extension if not already enabled
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================================
-- executions: Main execution records
-- ============================================================================
-- Each user request creates an execution record that tracks the full lifecycle
CREATE TABLE executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Channel identification
    channel_type VARCHAR(32) NOT NULL,     -- telegram, slack, api
    channel_id VARCHAR(255) NOT NULL,      -- chat/channel identifier
    user_id VARCHAR(255) NOT NULL,         -- user who initiated
    thread_id VARCHAR(255),                -- thread/reply context

    -- Status tracking
    status VARCHAR(32) NOT NULL DEFAULT 'pending',  -- pending, running, completed, failed, cancelled
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Content
    input_text TEXT NOT NULL,              -- original user message
    output_text TEXT,                      -- final response

    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for querying executions by channel
CREATE INDEX idx_executions_channel ON executions(channel_type, channel_id);

-- Index for querying executions by user
CREATE INDEX idx_executions_user ON executions(user_id);

-- Index for querying executions by status
CREATE INDEX idx_executions_status ON executions(status);

-- Index for time-based queries
CREATE INDEX idx_executions_created_at ON executions(created_at DESC);

-- ============================================================================
-- events: Event log for replay functionality
-- ============================================================================
-- Every significant action is logged as an event for replay/audit
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,

    -- Event ordering
    sequence_num INTEGER NOT NULL,         -- order within execution

    -- Event data
    event_type VARCHAR(64) NOT NULL,       -- UserInput, LlmRequest, ToolCall, etc.
    payload JSONB NOT NULL,                -- event-specific data

    -- Timing
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_ms INTEGER,                   -- how long this event took

    -- Metadata
    parent_event_id UUID REFERENCES events(id),  -- for nested events
    metadata JSONB NOT NULL DEFAULT '{}'
);

-- Index for querying events by execution
CREATE INDEX idx_events_execution ON events(execution_id, sequence_num);

-- Index for querying events by type
CREATE INDEX idx_events_type ON events(event_type);

-- Index for time-based queries
CREATE INDEX idx_events_timestamp ON events(timestamp DESC);

-- Ensure sequence numbers are unique within an execution
CREATE UNIQUE INDEX idx_events_execution_sequence ON events(execution_id, sequence_num);

-- ============================================================================
-- sessions: Conversation context storage
-- ============================================================================
-- Maintains conversation history and context for multi-turn interactions
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Session identification (channel_type:channel_id:user_id)
    session_key VARCHAR(512) NOT NULL UNIQUE,

    -- Context data
    context JSONB NOT NULL DEFAULT '[]',   -- array of messages

    -- Session state
    last_activity TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,                -- optional expiration

    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for session lookup
CREATE INDEX idx_sessions_key ON sessions(session_key);

-- Index for cleanup of expired sessions
CREATE INDEX idx_sessions_expires ON sessions(expires_at) WHERE expires_at IS NOT NULL;

-- ============================================================================
-- tool_executions: Detailed tool execution log
-- ============================================================================
-- Tracks individual tool calls for debugging and analysis
CREATE TABLE tool_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    event_id UUID REFERENCES events(id) ON DELETE SET NULL,

    -- Tool identification
    tool_name VARCHAR(128) NOT NULL,
    tool_version VARCHAR(32),

    -- Execution data
    input JSONB NOT NULL,
    output JSONB,
    error TEXT,

    -- Status
    status VARCHAR(32) NOT NULL DEFAULT 'pending',  -- pending, running, completed, failed, timeout
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    duration_ms INTEGER,

    -- Risk assessment
    risk_level VARCHAR(16) NOT NULL DEFAULT 'low',  -- low, medium, high
    required_approval BOOLEAN NOT NULL DEFAULT FALSE,
    approved_by VARCHAR(255),
    approved_at TIMESTAMPTZ
);

-- Index for querying tool executions
CREATE INDEX idx_tool_executions_execution ON tool_executions(execution_id);
CREATE INDEX idx_tool_executions_tool ON tool_executions(tool_name);
CREATE INDEX idx_tool_executions_status ON tool_executions(status);

-- ============================================================================
-- llm_requests: LLM API call log
-- ============================================================================
-- Tracks all LLM API calls for cost analysis and debugging
CREATE TABLE llm_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    event_id UUID REFERENCES events(id) ON DELETE SET NULL,

    -- Provider info
    provider VARCHAR(64) NOT NULL,         -- openai, anthropic, gemini, ollama
    model VARCHAR(128) NOT NULL,

    -- Request data
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    total_tokens INTEGER,

    -- Cost tracking (in USD cents)
    cost_cents INTEGER,

    -- Timing
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    latency_ms INTEGER,

    -- Status
    status VARCHAR(32) NOT NULL DEFAULT 'pending',  -- pending, completed, failed, timeout
    error TEXT,

    -- Response caching
    cache_hit BOOLEAN NOT NULL DEFAULT FALSE,
    cache_key VARCHAR(64)
);

-- Index for querying LLM requests
CREATE INDEX idx_llm_requests_execution ON llm_requests(execution_id);
CREATE INDEX idx_llm_requests_provider ON llm_requests(provider, model);
CREATE INDEX idx_llm_requests_timestamp ON llm_requests(started_at DESC);

-- ============================================================================
-- Trigger for updated_at
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_executions_updated_at
    BEFORE UPDATE ON executions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_sessions_updated_at
    BEFORE UPDATE ON sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
