//! TUI application state management

use chrono::Local;
use cratos_core::Orchestrator;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A single chat message displayed in the TUI.
pub struct ChatMessage {
    pub role: Role,
    pub sender: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Local>,
}

/// Who sent the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Main application state.
pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub persona: String,
    pub provider_name: String,
    pub scroll_offset: usize,
    pub is_loading: bool,
    pub should_quit: bool,
    pub loading_tick: usize,
    /// Cached quota status line for TUI display (updated each tick).
    pub quota_line: String,
    /// Whether the sidebar is visible (toggled with F1).
    pub show_sidebar: bool,
    orchestrator: Arc<Orchestrator>,
    session_id: String,
    /// Sender side lives in App so `submit_message` can clone it into spawned tasks.
    response_tx: mpsc::UnboundedSender<ChatMessage>,
    /// Receiver side polled each frame by the event loop.
    pub response_rx: mpsc::UnboundedReceiver<ChatMessage>,
}

impl App {
    pub fn new(
        orchestrator: Arc<Orchestrator>,
        provider_name: String,
        persona: Option<String>,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let persona = persona.unwrap_or_else(|| "cratos".to_string());
        let session_id = format!("tui-{}", uuid::Uuid::new_v4());

        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            persona: persona.clone(),
            provider_name,
            scroll_offset: 0,
            is_loading: false,
            should_quit: false,
            loading_tick: 0,
            quota_line: String::new(),
            show_sidebar: false,
            orchestrator,
            session_id,
            response_tx: tx,
            response_rx: rx,
        };

        app.push_system(format!(
            "Welcome to Cratos TUI. Persona: {}. Type /help for commands.",
            persona,
        ));
        app
    }

    // ── helpers ──────────────────────────────────────────────────────────

    fn push_system(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::System,
            sender: "system".into(),
            content,
            timestamp: Local::now(),
        });
    }

    fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::User,
            sender: "You".into(),
            content,
            timestamp: Local::now(),
        });
    }

    /// Scroll to the bottom of the chat history.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Toggle sidebar visibility (F1).
    pub fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
    }

    // ── input handling ──────────────────────────────────────────────────

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            // Move left by one character, respecting multi-byte chars.
            let before = &self.input[..self.cursor_pos];
            if let Some(ch) = before.chars().next_back() {
                self.cursor_pos -= ch.len_utf8();
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let after = &self.input[self.cursor_pos..];
            if let Some(ch) = after.chars().next() {
                self.cursor_pos += ch.len_utf8();
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_pos > 0 {
            let before = &self.input[..self.cursor_pos];
            if let Some(ch) = before.chars().next_back() {
                let remove_pos = self.cursor_pos - ch.len_utf8();
                self.input.remove(remove_pos);
                self.cursor_pos = remove_pos;
            }
        }
    }

    /// Delete character after cursor (Delete key).
    pub fn delete_char_after_cursor(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
        }
    }

    /// Move cursor one word to the left (Alt+Left / Option+Left).
    pub fn move_cursor_word_left(&mut self) {
        let before = &self.input[..self.cursor_pos];
        // Skip trailing whitespace, then skip word chars
        let trimmed = before.trim_end();
        if trimmed.is_empty() {
            self.cursor_pos = 0;
            return;
        }
        // Find last whitespace boundary in trimmed portion
        if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
            // pos is byte index of the whitespace; move to char after it
            self.cursor_pos = pos + trimmed[pos..].chars().next().unwrap().len_utf8();
        } else {
            self.cursor_pos = 0;
        }
    }

    /// Move cursor one word to the right (Alt+Right / Option+Right).
    pub fn move_cursor_word_right(&mut self) {
        let after = &self.input[self.cursor_pos..];
        // Skip leading non-whitespace, then skip whitespace
        let mut chars = after.char_indices();
        // Skip current word
        let mut offset = 0;
        for (i, c) in chars.by_ref() {
            if c.is_whitespace() {
                offset = i;
                break;
            }
            offset = i + c.len_utf8();
        }
        // Skip whitespace
        for (i, c) in self.input[self.cursor_pos + offset..].char_indices() {
            if !c.is_whitespace() {
                self.cursor_pos += offset + i;
                return;
            }
        }
        self.cursor_pos = self.input.len();
    }

    /// Delete word before cursor (Ctrl+W / Alt+Backspace).
    pub fn delete_word_before_cursor(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let before = &self.input[..self.cursor_pos];
        // Skip trailing whitespace, then skip word chars
        let trimmed = before.trim_end();
        let new_pos = if trimmed.is_empty() {
            0
        } else if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
            pos + trimmed[pos..].chars().next().unwrap().len_utf8()
        } else {
            0
        };
        self.input.drain(new_pos..self.cursor_pos);
        self.cursor_pos = new_pos;
    }

    /// Clear from cursor to beginning of line (Ctrl+U).
    pub fn clear_to_start(&mut self) {
        self.input.drain(..self.cursor_pos);
        self.cursor_pos = 0;
    }

    /// Clear from cursor to end of line (Ctrl+K).
    pub fn clear_to_end(&mut self) {
        self.input.truncate(self.cursor_pos);
    }

    /// Move cursor to beginning of line (Ctrl+A / Home).
    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end of line (Ctrl+E / End).
    pub fn move_cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    // ── submit ──────────────────────────────────────────────────────────

    /// Process typed input: either a `/command` or a chat message.
    pub fn submit(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.input.clear();
        self.cursor_pos = 0;

        if text.starts_with('/') {
            self.handle_command(&text);
        } else {
            self.submit_message(text);
        }
    }

    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        match parts[0] {
            "/quit" | "/exit" => {
                self.should_quit = true;
            }
            "/clear" => {
                self.messages.clear();
                self.scroll_offset = 0;
                self.push_system("Chat cleared.".into());
            }
            "/help" => {
                self.push_system(
                    "Commands:\n\
                     \x20 /persona <name>  Switch persona\n\
                     \x20 /clear           Clear chat\n\
                     \x20 /help            Show this help\n\
                     \x20 /quit            Exit TUI\n\
                     \n\
                     Keys:\n\
                     \x20 Ctrl+U / Ctrl+K  Clear before/after cursor\n\
                     \x20 Ctrl+W           Delete word backward\n\
                     \x20 Ctrl+A / Ctrl+E  Home / End\n\
                     \x20 Alt+\u{2190}/\u{2192}         Word left/right\n\
                     \x20 \u{2191}/\u{2193}              Scroll chat\n\
                     \x20 PageUp/PageDn    Scroll fast\n\
                     \x20 F1               Toggle sidebar\n\
                     \x20 Ctrl+L           Clear screen\n\
                     \x20 Ctrl+C / Esc     Quit"
                        .into(),
                );
            }
            "/persona" => {
                if let Some(name) = parts.get(1) {
                    self.persona = name.trim().to_string();
                    self.push_system(format!("Persona switched to: {}", self.persona));
                } else {
                    self.push_system(format!("Current persona: {}", self.persona));
                }
            }
            _ => {
                self.push_system(format!("Unknown command: {}", parts[0]));
            }
        }
    }

    fn submit_message(&mut self, text: String) {
        self.push_user(text.clone());
        self.is_loading = true;
        self.loading_tick = 0;
        self.scroll_to_bottom();

        let orchestrator = self.orchestrator.clone();
        let session_id = self.session_id.clone();
        let persona = self.persona.clone();
        let tx = self.response_tx.clone();

        tokio::spawn(async move {
            let input = cratos_core::OrchestratorInput::new("tui", &session_id, "tui-user", &text);

            let msg = match orchestrator.process(input).await {
                Ok(result) => ChatMessage {
                    role: Role::Assistant,
                    sender: persona,
                    content: result.response,
                    timestamp: Local::now(),
                },
                Err(e) => ChatMessage {
                    role: Role::System,
                    sender: "error".into(),
                    content: format!("Error: {e}"),
                    timestamp: Local::now(),
                },
            };

            let _ = tx.send(msg);
        });
    }

    /// Called every tick to drain incoming LLM responses.
    pub fn poll_responses(&mut self) {
        while let Ok(msg) = self.response_rx.try_recv() {
            self.is_loading = false;
            self.messages.push(msg);
            self.scroll_to_bottom();
        }
    }

    /// Refresh the cached quota status line from the global tracker.
    pub fn refresh_quota(&mut self) {
        // Use try_read to avoid blocking — skip update if lock is held
        let tracker = cratos_llm::global_quota_tracker();
        let states = tracker.try_get_all_states();
        if states.is_empty() {
            self.quota_line = String::new();
            return;
        }

        // Show the most recently updated provider
        let latest = states
            .iter()
            .max_by_key(|s| s.updated_at);

        if let Some(state) = latest {
            let req = match (state.requests_remaining, state.requests_limit) {
                (Some(rem), Some(lim)) => format!("{}/{}", rem, cratos_llm::format_compact_number(lim)),
                _ => "-".to_string(),
            };
            let tok = match (state.tokens_remaining, state.tokens_limit) {
                (Some(rem), Some(lim)) => format!(
                    "{}/{}",
                    cratos_llm::format_compact_number(rem),
                    cratos_llm::format_compact_number(lim)
                ),
                _ => "-".to_string(),
            };
            let reset = state
                .reset_at
                .map(|at| {
                    let dur = at - chrono::Utc::now();
                    if dur.num_seconds() <= 0 {
                        "now".to_string()
                    } else {
                        cratos_llm::format_duration(&dur)
                    }
                })
                .unwrap_or_else(|| "-".to_string());

            let auth_suffix = cratos_llm::cli_auth::get_auth_source(&state.provider)
                .filter(|s| *s != cratos_llm::cli_auth::AuthSource::ApiKey)
                .map(|s| format!(" ({})", s))
                .unwrap_or_default();

            self.quota_line = format!(
                "{}{} {} req {} tok ~{}",
                state.provider, auth_suffix, req, tok, reset
            );
        }
    }

    /// Advance the loading spinner animation counter.
    pub fn tick(&mut self) {
        if self.is_loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }
        // Refresh quota every 4 ticks (~1 second at 250ms tick rate)
        if self.loading_tick % 4 == 0 {
            self.refresh_quota();
        }
    }
}
