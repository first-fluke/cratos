//! PTY session management

use std::time::Instant;

/// Status of a PTY session.
#[derive(Debug, Clone)]
pub(crate) enum SessionStatus {
    Running,
    Exited(i32),
}

/// A background PTY session.
pub(crate) struct PtySession {
    pub id: String,
    pub command: String,
    pub child: tokio::process::Child,
    pub pty: pty_process::Pty,
    pub output_buffer: Vec<u8>,
    pub read_offset: usize,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub status: SessionStatus,
}
