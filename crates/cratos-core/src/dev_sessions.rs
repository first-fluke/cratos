//! AI Development Session Monitor
//!
//! Detects and monitors active AI coding sessions (Claude Code, Gemini CLI, Codex, etc.)
//! by scanning known process patterns and config directories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Supported AI development tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevTool {
    /// Claude Code (Anthropic CLI)
    ClaudeCode,
    /// Gemini CLI (Google)
    GeminiCli,
    /// Codex (OpenAI)
    Codex,
    /// Cursor IDE
    Cursor,
}

impl DevTool {
    /// Human-readable name
    pub fn display_name(&self) -> &str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::GeminiCli => "Gemini CLI",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
        }
    }
}

/// Status of a dev session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Actively processing
    Active,
    /// Running but idle
    Idle,
}

/// A detected AI development session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevSession {
    /// Which tool is running
    pub tool: DevTool,
    /// Working directory (if detectable)
    pub project_path: Option<String>,
    /// Session status
    pub status: SessionStatus,
    /// When the session was first detected
    pub detected_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Process ID (if from process detection)
    pub pid: Option<u32>,
}

/// Monitors active AI development sessions on the local machine.
pub struct DevSessionMonitor {
    sessions: Arc<RwLock<Vec<DevSession>>>,
    poll_interval: Duration,
}

impl DevSessionMonitor {
    /// Create a new monitor with the given poll interval.
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(Vec::new())),
            poll_interval,
        }
    }

    /// Get current sessions snapshot.
    pub async fn sessions(&self) -> Vec<DevSession> {
        self.sessions.read().await.clone()
    }

    /// Get sessions for a specific tool.
    pub async fn sessions_for_tool(&self, tool: DevTool) -> Vec<DevSession> {
        self.sessions
            .read()
            .await
            .iter()
            .filter(|s| s.tool == tool)
            .cloned()
            .collect()
    }

    /// Start the background polling loop.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let monitor = self;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitor.poll_interval);
            loop {
                interval.tick().await;
                monitor.poll_once().await;
            }
        })
    }

    /// Perform a single poll cycle.
    async fn poll_once(&self) {
        let mut detected = Vec::new();
        let now = Utc::now();

        // Detect Claude Code sessions
        if let Some(sessions) = detect_claude_code(now).await {
            detected.extend(sessions);
        }

        // Detect Gemini CLI sessions
        if let Some(sessions) = detect_by_process("gemini", DevTool::GeminiCli, now).await {
            detected.extend(sessions);
        }

        // Detect Codex sessions
        if let Some(sessions) = detect_by_process("codex", DevTool::Codex, now).await {
            detected.extend(sessions);
        }

        // Detect Cursor
        if let Some(sessions) = detect_by_process("cursor", DevTool::Cursor, now).await {
            detected.extend(sessions);
        }

        let mut sessions = self.sessions.write().await;
        *sessions = detected;
        debug!(count = sessions.len(), "Dev session poll complete");
    }
}

/// Detect Claude Code sessions by checking `~/.claude/projects/` directory mtime.
async fn detect_claude_code(now: DateTime<Utc>) -> Option<Vec<DevSession>> {
    let home = dirs::home_dir()?;
    let claude_dir = home.join(".claude").join("projects");
    if !claude_dir.exists() {
        return None;
    }

    // Also check for running process
    let pid = find_process_pid("claude").await;

    // Check recently modified project directories (within last 5 minutes)
    let mut sessions = Vec::new();
    let entries = match std::fs::read_dir(&claude_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let elapsed = modified.elapsed().unwrap_or(Duration::from_secs(86400));
        if elapsed < Duration::from_secs(300) {
            // Active within 5 minutes
            let project_name = entry.file_name().to_string_lossy().to_string();
            let status = if elapsed < Duration::from_secs(30) {
                SessionStatus::Active
            } else {
                SessionStatus::Idle
            };
            sessions.push(DevSession {
                tool: DevTool::ClaudeCode,
                project_path: Some(project_name),
                status,
                detected_at: now,
                last_activity: now - chrono::Duration::from_std(elapsed).unwrap_or_default(),
                pid,
            });
        }
    }

    if sessions.is_empty() && pid.is_some() {
        // Process running but no recent project activity
        sessions.push(DevSession {
            tool: DevTool::ClaudeCode,
            project_path: None,
            status: SessionStatus::Idle,
            detected_at: now,
            last_activity: now,
            pid,
        });
    }

    if sessions.is_empty() {
        None
    } else {
        Some(sessions)
    }
}

/// Detect sessions by looking for a running process name.
async fn detect_by_process(
    process_name: &str,
    tool: DevTool,
    now: DateTime<Utc>,
) -> Option<Vec<DevSession>> {
    let pid = find_process_pid(process_name).await?;
    Some(vec![DevSession {
        tool,
        project_path: None,
        status: SessionStatus::Active,
        detected_at: now,
        last_activity: now,
        pid: Some(pid),
    }])
}

/// Find the PID of a process by name using `pgrep`.
async fn find_process_pid(name: &str) -> Option<u32> {
    let output = tokio::process::Command::new("pgrep")
        .arg("-f")
        .arg(name)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok())
}

#[cfg(test)]
mod tests;

