//! TUI application state management

use chrono::Local;
use cratos_core::Orchestrator;
use ratatui::style::Style;
use std::sync::Arc;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

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

/// Maximum number of input history entries retained.
const MAX_HISTORY: usize = 50;

/// Per-provider quota display data for the sidebar.
pub struct ProviderQuotaDisplay {
    /// Provider name (e.g. "gemini", "anthropic").
    pub provider: String,
    /// Remaining percentage (0.0–100.0), if known.
    pub remaining_pct: Option<f64>,
    /// Human-readable summary (e.g. "85% left" or "987/1.0K req").
    pub summary: String,
    /// Reset time display (e.g. "~2m 14s").
    pub reset_display: String,
    /// Tier label (e.g. "free"), if known.
    pub tier_label: Option<String>,
}

/// Main application state.
pub struct App {
    pub messages: Vec<ChatMessage>,
    pub textarea: TextArea<'static>,
    pub persona: String,
    pub provider_name: String,
    pub scroll_offset: usize,
    pub is_loading: bool,
    pub should_quit: bool,
    pub loading_tick: usize,
    /// Global tick counter (always increments, used for periodic refresh).
    tick_count: usize,
    /// Per-provider quota info for sidebar display.
    pub provider_quotas: Vec<ProviderQuotaDisplay>,
    /// One-line quota summary for the status bar.
    pub quota_status_line: String,
    /// Cached cost status line for TUI display (updated each tick).
    pub cost_line: String,
    /// Whether the sidebar is visible (toggled with F1).
    pub show_sidebar: bool,
    /// Previous input history for up/down navigation.
    input_history: Vec<String>,
    /// Current position in input history (None = new input).
    history_index: Option<usize>,
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
            textarea: new_textarea(),
            persona: persona.clone(),
            provider_name,
            scroll_offset: 0,
            is_loading: false,
            should_quit: false,
            loading_tick: 0,
            tick_count: 0,
            provider_quotas: Vec::new(),
            quota_status_line: "awaiting first API call".to_string(),
            cost_line: String::new(),
            show_sidebar: false,
            input_history: Vec::new(),
            history_index: None,
            orchestrator,
            session_id,
            response_tx: tx,
            response_rx: rx,
        };

        app.push_system(format!(
            "Welcome to Cratos TUI. Persona: {}. Type /help for commands, F1 for sidebar.",
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

    /// Returns true if the textarea is empty (single empty line).
    pub fn is_input_empty(&self) -> bool {
        self.textarea.lines().len() == 1 && self.textarea.lines()[0].is_empty()
    }

    /// Returns true if there are previous inputs in history.
    pub fn has_history(&self) -> bool {
        !self.input_history.is_empty()
    }

    /// Navigate to the previous entry in input history.
    pub fn history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            None => self.input_history.len() - 1,
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.history_index = Some(idx);
        let text = self.input_history[idx].clone();
        self.textarea = TextArea::new(vec![text]);
        self.textarea.set_cursor_line_style(Style::default());
    }

    /// Navigate to the next entry in input history, or clear if at the end.
    pub fn history_down(&mut self) {
        match self.history_index {
            None => {}
            Some(i) if i >= self.input_history.len() - 1 => {
                self.history_index = None;
                self.textarea = new_textarea();
            }
            Some(i) => {
                self.history_index = Some(i + 1);
                let text = self.input_history[i + 1].clone();
                self.textarea = TextArea::new(vec![text]);
                self.textarea.set_cursor_line_style(Style::default());
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
        let text = self.textarea.lines().join("\n").trim().to_string();
        if text.is_empty() {
            return;
        }

        // Store in history (cap at MAX_HISTORY)
        self.input_history.push(text.clone());
        if self.input_history.len() > MAX_HISTORY {
            self.input_history.remove(0);
        }
        self.history_index = None;

        // Reset textarea
        self.textarea = new_textarea();

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
                     \x20 \u{2191}/\u{2193} (empty input) Input history\n\
                     \x20 \u{2191}/\u{2193} (otherwise)   Scroll chat\n\
                     \x20 PageUp/PageDn    Scroll fast\n\
                     \x20 Mouse scroll     Scroll chat\n\
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

    /// Refresh multi-provider quota data from the global tracker.
    pub fn refresh_quota(&mut self) {
        let tracker = cratos_llm::global_quota_tracker();
        let states = tracker.try_get_all_states();

        // Bug 6: show placeholder when no data yet
        if states.is_empty() {
            self.provider_quotas.clear();
            self.quota_status_line = "awaiting first API call".to_string();
            return;
        }

        let mut displays = Vec::with_capacity(states.len());
        let mut status_parts: Vec<String> = Vec::new();

        for state in &states {
            let remaining_pct = state.remaining_pct();

            let summary = if let Some(pct) = remaining_pct {
                format!("{:.0}% left", pct)
            } else {
                match (state.requests_remaining, state.requests_limit) {
                    (Some(rem), Some(lim)) => {
                        format!("{}/{} req", rem, cratos_llm::format_compact_number(lim))
                    }
                    _ => "-".to_string(),
                }
            };

            let reset_display = state
                .reset_at
                .map(|at| {
                    let dur = at - chrono::Utc::now();
                    if dur.num_seconds() <= 0 {
                        "now".to_string()
                    } else {
                        format!("~{}", cratos_llm::format_duration(&dur))
                    }
                })
                .unwrap_or_default();

            // Build status bar part
            let tier_suffix = state
                .tier_label
                .as_deref()
                .map(|t| format!("[{}]", t))
                .unwrap_or_default();
            status_parts.push(format!("{}{} {}", state.provider, tier_suffix, summary));

            displays.push(ProviderQuotaDisplay {
                provider: state.provider.clone(),
                remaining_pct,
                summary,
                reset_display,
                tier_label: state.tier_label.clone(),
            });
        }

        self.provider_quotas = displays;
        self.quota_status_line = status_parts.join(" \u{00b7} ");
    }

    /// Refresh the cached cost status line from the global tracker.
    pub fn refresh_cost(&mut self) {
        let tracker = cratos_llm::global_tracker();
        if let Some(stats) = tracker.try_get_stats() {
            if stats.total_requests == 0 {
                self.cost_line = String::new();
            } else {
                // Check if using a free-tier auth source (OAuth, CLI tokens)
                let is_free = cratos_llm::cli_auth::get_all_auth_sources()
                    .values()
                    .any(|s| *s != cratos_llm::cli_auth::AuthSource::ApiKey);
                if is_free {
                    self.cost_line = format!("{} req (free tier)", stats.total_requests);
                } else {
                    self.cost_line = format!(
                        "~${:.4} ({} req)",
                        stats.total_cost, stats.total_requests,
                    );
                }
            }
        }
    }

    /// Advance the loading spinner animation counter.
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.is_loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }
        // Refresh quota and cost every 4 ticks (~800ms at 200ms tick rate)
        if self.tick_count % 4 == 0 {
            self.refresh_quota();
            self.refresh_cost();
        }
    }
}

/// Create a fresh TextArea with default styling.
fn new_textarea() -> TextArea<'static> {
    let mut ta = TextArea::default();
    ta.set_cursor_line_style(Style::default());
    ta.set_placeholder_text("Type a message... (Enter to send)");
    ta.set_max_histories(50);
    ta
}
