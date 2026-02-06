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
                    "Commands:\n  /persona <name>  Switch persona\n  /clear            Clear chat\n  /help             Show this help\n  /quit             Exit TUI".into(),
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

    /// Advance the loading spinner animation counter.
    pub fn tick(&mut self) {
        if self.is_loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }
    }
}
