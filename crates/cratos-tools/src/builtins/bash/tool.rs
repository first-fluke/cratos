//! BashTool — PTY-based shell execution with 5-layer security
//!
//! Unlike the `exec` tool which uses `Command::new()` (no shell, no pipes),
//! this tool spawns a real bash shell via PTY, supporting:
//! - Pipe chains: `ps aux | grep node | head -20`
//! - Redirections: `echo data > /tmp/out.txt`
//! - Command chaining: `cd /project && make clean && make`
//! - Background sessions with poll/send_keys/kill

use super::config::BashConfig;
use super::constants::*;
use super::rate_limit::RateLimiter;
use super::sanitize::{sanitize_output, strip_ansi_escapes};
use super::security::{is_informational_exit, SecurityAnalyzer};
use super::session::{PtySession, SessionStatus};
use crate::error::{Error, Result};
use crate::registry::{RiskLevel, Tool, ToolCategory, ToolDefinition, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// PTY-based bash tool with 5-layer security.
pub struct BashTool {
    definition: ToolDefinition,
    config: BashConfig,
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl BashTool {
    /// Create a new bash tool with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(BashConfig::default())
    }

    /// Create a new bash tool with custom config.
    #[must_use]
    pub fn with_config(config: BashConfig) -> Self {
        let definition = ToolDefinition::new(
            "bash",
            "Execute shell commands via PTY with full bash support. \
             Supports pipes (|), redirections (>), command chaining (&&), \
             and background sessions. Use action=\"run\" (default) to execute, \
             action=\"poll\" to check background session output, \
             action=\"send_keys\" to send input, action=\"kill\" to terminate, \
             action=\"list\" to list sessions."
        )
        .with_category(ToolCategory::Exec)
        .with_risk_level(RiskLevel::High)
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute (supports pipes, redirections, chaining)"
                },
                "action": {
                    "type": "string",
                    "enum": ["run", "poll", "send_keys", "kill", "list"],
                    "description": "Action to perform (default: run)"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": format!("Timeout in seconds (default {}, max {})", config.default_timeout_secs, config.max_timeout_secs)
                },
                "session_id": {
                    "type": "string",
                    "description": "Background session ID. If provided with run, command runs in background."
                },
                "keys": {
                    "type": "string",
                    "description": "Input to send (for send_keys action). Use \\n for Enter, \\x03 for Ctrl+C."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory (optional)"
                }
            },
            "required": ["command"]
        }));

        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(config.max_commands_per_minute)));
        let sessions: Arc<Mutex<HashMap<String, PtySession>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn background cleanup task (only if tokio runtime is available)
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let sessions_ref = sessions.clone();
            handle.spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(
                        SESSION_CLEANUP_INTERVAL_SECS,
                    ))
                    .await;
                    let mut map = sessions_ref.lock().await;

                    // V3: Drain and monitor background session output
                    for (id, session) in map.iter_mut() {
                        if matches!(session.status, SessionStatus::Running) {
                            let mut read_buf = [0u8; 4096];
                            let read_timeout = std::time::Duration::from_millis(50);
                            loop {
                                match tokio::time::timeout(
                                    read_timeout,
                                    session.pty.read(&mut read_buf),
                                )
                                .await
                                {
                                    Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                                    Ok(Ok(n)) => {
                                        session.output_buffer.extend_from_slice(&read_buf[..n]);
                                    }
                                }
                            }
                            // Check for secret patterns in output
                            let raw = String::from_utf8_lossy(&session.output_buffer);
                            for pattern in SECRET_PATTERNS {
                                if raw.contains(pattern) {
                                    warn!(
                                        session_id = %id,
                                        pattern = %pattern,
                                        "Secret detected in background session output"
                                    );
                                    break;
                                }
                            }
                        }
                    }

                    let now = Instant::now();
                    let idle_timeout = std::time::Duration::from_secs(SESSION_IDLE_TIMEOUT_SECS);
                    let stale_ids: Vec<String> = map
                        .iter()
                        .filter(|(_, s)| now.duration_since(s.last_activity) > idle_timeout)
                        .map(|(id, _)| id.clone())
                        .collect();
                    for id in stale_ids {
                        if let Some(mut session) = map.remove(&id) {
                            let _ = session.child.kill().await;
                            info!(session_id = %id, "Cleaned up idle bash session");
                        }
                    }
                }
            });
        }

        Self {
            definition,
            config,
            sessions,
            rate_limiter,
        }
    }

    fn analyzer(&self) -> SecurityAnalyzer<'_> {
        SecurityAnalyzer::new(&self.config)
    }

    // ── Actions ──────────────────────────────────────────────────────────

    async fn action_run(
        &self,
        command: &str,
        session_id: Option<&str>,
        timeout_secs: u64,
        cwd: Option<&str>,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let analyzer = self.analyzer();

        // Layer 2: Pipeline analysis
        analyzer.analyze_pipeline(command)?;

        // Layer 4: Rate limit
        {
            let mut rl = self.rate_limiter.lock().await;
            if !rl.check() {
                return Err(Error::PermissionDenied(
                    "Rate limit exceeded: too many commands per minute".to_string(),
                ));
            }
        }

        // Resolve working directory
        let working_dir = if let Some(dir) = cwd {
            Some(analyzer.validate_cwd(dir)?)
        } else {
            self.config.default_cwd.clone()
        };

        // Build environment
        let env_vars = analyzer.build_env_whitelist();

        // Determine shell path
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        // Allocate PTY pair
        let (pty, pts) = pty_process::open()
            .map_err(|e| Error::Execution(format!("Failed to open PTY: {}", e)))?;

        // Build PTY command (builder pattern — each method consumes self)
        let mut pty_cmd = pty_process::Command::new(&shell)
            .args(["-c", command])
            .env_clear();

        // Apply environment whitelist
        for (key, value) in &env_vars {
            pty_cmd = pty_cmd.env(key, value);
        }
        // Ensure TERM is set for PTY
        pty_cmd = pty_cmd.env("TERM", "xterm-256color");

        if let Some(dir) = &working_dir {
            pty_cmd = pty_cmd.current_dir(dir);
        }

        // Spawn the child process attached to the PTY
        let child = pty_cmd
            .spawn(pts)
            .map_err(|e| Error::Execution(format!("Failed to spawn bash: {}", e)))?;

        // If session_id is given, store as background session
        if let Some(sid) = session_id {
            let mut sessions = self.sessions.lock().await;

            // Layer 4: Session limit
            if sessions.len() >= self.config.max_sessions {
                // Kill oldest session
                let oldest_id = sessions
                    .iter()
                    .min_by_key(|(_, s)| s.created_at)
                    .map(|(id, _)| id.clone());
                if let Some(old_id) = oldest_id {
                    if let Some(mut old) = sessions.remove(&old_id) {
                        let _ = old.child.kill().await;
                        warn!(session_id = %old_id, "Evicted oldest session to make room");
                    }
                }
            }

            sessions.insert(
                sid.to_string(),
                PtySession {
                    id: sid.to_string(),
                    command: command.to_string(),
                    child,
                    pty,
                    output_buffer: Vec::new(),
                    read_offset: 0,
                    created_at: Instant::now(),
                    last_activity: Instant::now(),
                    status: SessionStatus::Running,
                },
            );

            let duration = start.elapsed().as_millis() as u64;
            return Ok(ToolResult::success(
                serde_json::json!({
                    "session_id": sid,
                    "status": "started",
                    "command": command,
                    "message": "Background session started. Use action=\"poll\" to check output."
                }),
                duration,
            ));
        }

        // Foreground execution: read output until exit or timeout
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let mut output_buf = Vec::new();
        let mut read_buf = [0u8; 4096];
        let mut pty_rw = pty;
        let mut child = child;
        let max_output = self.config.max_output_bytes;

        let result = tokio::time::timeout(timeout, async {
            loop {
                tokio::select! {
                    n = pty_rw.read(&mut read_buf) => {
                        match n {
                            Ok(0) => break,
                            Ok(n) => {
                                output_buf.extend_from_slice(&read_buf[..n]);
                                // Layer 4: Output cap
                                if output_buf.len() > max_output {
                                    output_buf.truncate(max_output);
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    status = child.wait() => {
                        // Child exited; drain remaining output
                        loop {
                            match pty_rw.read(&mut read_buf).await {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    output_buf.extend_from_slice(&read_buf[..n]);
                                    if output_buf.len() > max_output {
                                        output_buf.truncate(max_output);
                                        break;
                                    }
                                }
                            }
                        }
                        return status;
                    }
                }
            }
            child.wait().await
        })
        .await;

        let duration = start.elapsed().as_millis() as u64;
        let raw_output = String::from_utf8_lossy(&output_buf).to_string();

        // Strip ANSI escape sequences for clean output
        let cleaned_output = strip_ansi_escapes(&raw_output);

        // Layer 5: Sanitize output
        let sanitized = sanitize_output(&cleaned_output);

        let truncated = output_buf.len() >= max_output;

        match result {
            Ok(Ok(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                let success = status.success() || is_informational_exit(command, exit_code);

                let mut output_json = serde_json::json!({
                    "stdout": sanitized,
                    "exit_code": exit_code,
                    "command": command
                });
                if truncated {
                    output_json["truncated"] = serde_json::json!(true);
                    output_json["truncated_message"] =
                        serde_json::json!(format!("Output truncated at {} bytes", max_output));
                }

                if success {
                    Ok(ToolResult::success(output_json, duration))
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: output_json,
                        error: Some(format!("Command exited with code {}", exit_code)),
                        duration_ms: duration,
                    })
                }
            }
            Ok(Err(e)) => Err(Error::Execution(format!(
                "Failed to wait for process: {}",
                e
            ))),
            Err(_) => {
                // Timeout — kill child
                let _ = child.kill().await;
                Ok(ToolResult {
                    success: false,
                    output: serde_json::json!({
                        "stdout": sanitized,
                        "command": command,
                        "timeout": true
                    }),
                    error: Some(format!("Command timed out after {}s", timeout_secs)),
                    duration_ms: duration,
                })
            }
        }
    }

    async fn action_poll(&self, session_id: &str) -> Result<ToolResult> {
        let start = Instant::now();
        let mut sessions = self.sessions.lock().await;

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::InvalidInput(format!("Session '{}' not found", session_id)))?;

        session.last_activity = Instant::now();

        // Try to read new output with a short timeout
        let mut read_buf = [0u8; 4096];
        let read_timeout = std::time::Duration::from_millis(100);
        loop {
            match tokio::time::timeout(read_timeout, session.pty.read(&mut read_buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    session.output_buffer.extend_from_slice(&read_buf[..n]);
                }
                Ok(Err(_)) => break,
                Err(_) => break, // Timeout — no more data ready
            }
        }

        // Check if child exited
        match session.child.try_wait() {
            Ok(Some(status)) => {
                session.status = SessionStatus::Exited(status.code().unwrap_or(-1));
            }
            Ok(None) => {} // Still running
            Err(_) => {
                session.status = SessionStatus::Exited(-1);
            }
        }

        // Return new output since last poll
        let new_output = if session.read_offset < session.output_buffer.len() {
            let data = &session.output_buffer[session.read_offset..];
            session.read_offset = session.output_buffer.len();
            let raw = String::from_utf8_lossy(data).to_string();
            sanitize_output(&strip_ansi_escapes(&raw))
        } else {
            String::new()
        };

        let (status_str, exit_code) = match &session.status {
            SessionStatus::Running => ("running", None),
            SessionStatus::Exited(code) => ("exited", Some(*code)),
        };

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            serde_json::json!({
                "session_id": session_id,
                "status": status_str,
                "exit_code": exit_code,
                "new_output": new_output,
                "total_output_bytes": session.output_buffer.len(),
                "elapsed_secs": session.created_at.elapsed().as_secs()
            }),
            duration,
        ))
    }

    async fn action_send_keys(&self, session_id: &str, keys: &str) -> Result<ToolResult> {
        let start = Instant::now();
        let mut sessions = self.sessions.lock().await;

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::InvalidInput(format!("Session '{}' not found", session_id)))?;

        if matches!(session.status, SessionStatus::Exited(_)) {
            return Err(Error::InvalidInput(format!(
                "Session '{}' has already exited",
                session_id
            )));
        }

        session.last_activity = Instant::now();

        // V2: Validate send_keys input before writing to PTY
        self.analyzer().validate_send_keys(keys)?;

        // Process escape sequences
        let processed = keys
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\x03", "\x03") // Ctrl+C
            .replace("\\x04", "\x04") // Ctrl+D
            .replace("\\x1a", "\x1a"); // Ctrl+Z

        session
            .pty
            .write_all(processed.as_bytes())
            .await
            .map_err(|e| Error::Execution(format!("Failed to send keys: {}", e)))?;

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            serde_json::json!({
                "session_id": session_id,
                "sent": keys,
                "bytes": processed.len()
            }),
            duration,
        ))
    }

    async fn action_kill(&self, session_id: &str) -> Result<ToolResult> {
        let start = Instant::now();
        let mut sessions = self.sessions.lock().await;

        let mut session = sessions
            .remove(session_id)
            .ok_or_else(|| Error::InvalidInput(format!("Session '{}' not found", session_id)))?;

        // Kill the child process
        let _ = session.child.kill().await;

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            serde_json::json!({
                "session_id": session_id,
                "status": "killed",
                "ran_for_secs": session.created_at.elapsed().as_secs()
            }),
            duration,
        ))
    }

    async fn action_list(&self) -> Result<ToolResult> {
        let start = Instant::now();
        let sessions = self.sessions.lock().await;

        let list: Vec<serde_json::Value> = sessions
            .values()
            .map(|s| {
                let (status, exit_code) = match &s.status {
                    SessionStatus::Running => ("running".to_string(), None),
                    SessionStatus::Exited(c) => ("exited".to_string(), Some(*c)),
                };
                serde_json::json!({
                    "session_id": s.id,
                    "command": s.command,
                    "status": status,
                    "exit_code": exit_code,
                    "elapsed_secs": s.created_at.elapsed().as_secs(),
                    "output_bytes": s.output_buffer.len()
                })
            })
            .collect();

        let duration = start.elapsed().as_millis() as u64;
        Ok(ToolResult::success(
            serde_json::json!({
                "sessions": list,
                "count": list.len()
            }),
            duration,
        ))
    }

    // Test helpers
    #[cfg(test)]
    pub(crate) fn analyze_pipeline(&self, command: &str) -> Result<()> {
        self.analyzer().analyze_pipeline(command)
    }

    #[cfg(test)]
    pub(crate) fn validate_cwd(&self, cwd: &str) -> Result<std::path::PathBuf> {
        self.analyzer().validate_cwd(cwd)
    }

    #[cfg(test)]
    pub(crate) fn validate_send_keys(&self, keys: &str) -> Result<()> {
        self.analyzer().validate_send_keys(keys)
    }

    #[cfg(test)]
    pub(crate) fn build_env_whitelist(&self) -> Vec<(String, String)> {
        self.analyzer().build_env_whitelist()
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("run");

        match action {
            "list" => return self.action_list().await,
            "poll" => {
                let sid = input
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("'session_id' required for poll action".to_string())
                    })?;
                return self.action_poll(sid).await;
            }
            "send_keys" => {
                let sid = input
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput(
                            "'session_id' required for send_keys action".to_string(),
                        )
                    })?;
                let keys = input.get("keys").and_then(|v| v.as_str()).ok_or_else(|| {
                    Error::InvalidInput("'keys' required for send_keys action".to_string())
                })?;
                return self.action_send_keys(sid, keys).await;
            }
            "kill" => {
                let sid = input
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::InvalidInput("'session_id' required for kill action".to_string())
                    })?;
                return self.action_kill(sid).await;
            }
            "run" => { /* fall through */ }
            _ => {
                return Err(Error::InvalidInput(format!(
                    "Unknown action '{}'. Valid: run, poll, send_keys, kill, list",
                    action
                )));
            }
        }

        // "run" action
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::InvalidInput("Missing 'command' parameter".to_string()))?;

        let session_id = input.get("session_id").and_then(|v| v.as_str());
        let cwd = input.get("cwd").and_then(|v| v.as_str());

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.config.default_timeout_secs)
            .min(self.config.max_timeout_secs);

        self.action_run(command, session_id, timeout_secs, cwd)
            .await
    }
}
