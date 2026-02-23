//! TUI application state management

use chrono::Local;
use cratos_core::Orchestrator;
use ratatui::style::Style;
use ratatui::widgets::{ListState, ScrollbarState};
use std::sync::Arc;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use super::command::CommandRegistry;
use super::settings::SettingsState;

/// Application mode (Vim-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Insert,
    #[allow(dead_code)]
    Command, // e.g. when typing ":" (future use)
}

/// UI Focus area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Chat,
    Input,
    Sidebar,
    Settings,
}

/// A single chat message displayed in the TUI.
#[derive(Debug, Clone)]
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
    pub provider: String,
    pub remaining_pct: Option<f64>,
    pub summary: String,
    pub reset_display: String,
    pub tier_label: Option<String>,
}

/// UI-specific state.
pub struct TuiState {
    pub scroll_offset: usize,
    pub scrollbar_state: ScrollbarState,
    pub show_sidebar: bool,
    pub mouse_captured: bool,
    pub is_loading: bool,
    pub loading_tick: usize,
    pub tick_count: usize,
    pub history_index: Option<usize>,
    pub suggestions: Vec<&'static str>,
    pub mode: AppMode,
    pub focus: Focus,
    // For selecting messages in Normal mode (future use)
    pub list_state: ListState,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            scrollbar_state: ScrollbarState::default(),
            show_sidebar: false,
            mouse_captured: true,
            is_loading: false,
            loading_tick: 0,
            tick_count: 0,
            history_index: None,
            suggestions: Vec::new(),
            mode: AppMode::Insert, // Start in Insert mode for convenience
            focus: Focus::Input,
            list_state: ListState::default(),
        }
    }
}

/// Internal event for the TUI event loop
#[derive(Debug)]
pub enum AppEvent {
    Chat(ChatMessage),
    ExecutionStarted(uuid::Uuid),
    ExecutionEnded(uuid::Uuid),
}

/// Main application state.
pub struct App {
    pub messages: Vec<ChatMessage>,
    pub textarea: TextArea<'static>,
    pub persona: String,
    pub provider_name: String,
    pub should_quit: bool,
    pub ui_state: TuiState,

    pub provider_quotas: Vec<ProviderQuotaDisplay>,
    pub quota_status_line: String,
    pub cost_line: String,

    input_history: Vec<String>,
    orchestrator: Arc<Orchestrator>,
    session_id: String,
    commands: Arc<CommandRegistry>,

    current_execution_id: Option<uuid::Uuid>,

    response_tx: mpsc::UnboundedSender<AppEvent>,
    pub response_rx: mpsc::UnboundedReceiver<AppEvent>,

    pub settings_state: Option<SettingsState>,
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
            should_quit: false,
            ui_state: TuiState::default(),
            provider_quotas: Vec::new(),
            quota_status_line: "awaiting first API call".to_string(),
            cost_line: String::new(),
            input_history: Vec::new(),
            orchestrator,
            session_id,
            commands: Arc::new(CommandRegistry::new()),
            current_execution_id: None,
            response_tx: tx,
            response_rx: rx,
            settings_state: None,
        };

        app.push_system(format!(
            "Welcome to Cratos TUI. Persona: {}. Type /help for commands.",
            persona,
        ));
        app
    }

    // ── helpers ──────────────────────────────────────────────────────────

    pub fn push_system(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::System,
            sender: "system".into(),
            content,
            timestamp: Local::now(),
        });
        self.scroll_to_bottom();
    }

    fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::User,
            sender: "You".into(),
            content,
            timestamp: Local::now(),
        });
        self.scroll_to_bottom();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.ui_state.scroll_offset = 0;
        // Also reset list state selection if we were browsing
        self.ui_state.list_state.select(None);
    }

    pub fn open_settings(&mut self) {
        self.settings_state = Some(SettingsState::load());
        self.ui_state.focus = Focus::Settings;
    }

    pub fn close_settings(&mut self) {
        self.settings_state = None;
        self.ui_state.focus = Focus::Input;
        self.ui_state.mode = AppMode::Insert;
    }

    pub fn toggle_sidebar(&mut self) {
        self.ui_state.show_sidebar = !self.ui_state.show_sidebar;
        // If sidebar hidden while focused, move focus back to input
        if !self.ui_state.show_sidebar && self.ui_state.focus == Focus::Sidebar {
            self.ui_state.focus = Focus::Input;
        }
    }

    pub fn set_mode(&mut self, mode: AppMode) {
        self.ui_state.mode = mode;
        match mode {
            AppMode::Insert => {
                self.ui_state.focus = Focus::Input;
                // Cursor style: block/bar depending on terminal, usually default is fine
                self.textarea.set_cursor_style(Style::default());
            }
            AppMode::Normal => {
                self.ui_state.focus = Focus::Chat;
                // Hide cursor in textarea when in normal mode
                // (Though strictly textarea handles its own rendering, we just won't update it)
            }
            AppMode::Command => {
                // Not fully implemented yet
            }
        }
    }

    // ── input handling ──────────────────────────────────────────────────

    pub fn is_input_empty(&self) -> bool {
        self.textarea.lines().len() == 1 && self.textarea.lines()[0].is_empty()
    }

    pub fn has_history(&self) -> bool {
        !self.input_history.is_empty()
    }

    pub fn history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let idx = match self.ui_state.history_index {
            None => self.input_history.len() - 1,
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.ui_state.history_index = Some(idx);
        let text = self.input_history[idx].clone();
        self.textarea = TextArea::new(vec![text]);
        self.textarea.move_cursor(tui_textarea::CursorMove::End);
    }

    pub fn history_down(&mut self) {
        match self.ui_state.history_index {
            None => {}
            Some(i) if i >= self.input_history.len() - 1 => {
                self.ui_state.history_index = None;
                self.textarea = new_textarea();
            }
            Some(i) => {
                self.ui_state.history_index = Some(i + 1);
                let text = self.input_history[i + 1].clone();
                self.textarea = TextArea::new(vec![text]);
                self.textarea.move_cursor(tui_textarea::CursorMove::End);
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.ui_state.scroll_offset = self.ui_state.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.ui_state.scroll_offset = self.ui_state.scroll_offset.saturating_sub(1);
    }

    pub fn update_suggestions(&mut self) {
        let text = self.textarea.lines()[0].as_str();
        if text.starts_with('/') && !text.contains(' ') {
            self.ui_state.suggestions = self.commands.get_suggestions(text);
        } else {
            self.ui_state.suggestions.clear();
        }
    }

    // ── submit ──────────────────────────────────────────────────────────

    pub fn submit(&mut self) {
        let text = self.textarea.lines().join("\n").trim().to_string();
        if text.is_empty() {
            return;
        }

        self.input_history.push(text.clone());
        if self.input_history.len() > MAX_HISTORY {
            self.input_history.remove(0);
        }
        self.ui_state.history_index = None;
        self.textarea = new_textarea();
        self.ui_state.suggestions.clear();

        if text.starts_with('/') {
            let commands = self.commands.clone();
            if let Err(e) = commands.handle(self, &text) {
                self.push_system(format!("Command error: {}", e));
            }
        } else {
            self.submit_message(text);
        }
    }

    fn submit_message(&mut self, text: String) {
        self.push_user(text.clone());

        // Check if we should steer an existing execution
        if let Some(exec_id) = self.current_execution_id {
            if let Some(handle) = self.orchestrator.get_steer_handle(exec_id) {
                // Inject steering message
                let tx = self.response_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle.inject_message(text).await {
                        let _ = tx.send(AppEvent::Chat(ChatMessage {
                            role: Role::System,
                            sender: "system".into(),
                            content: format!("Failed to inject steering message: {}", e),
                            timestamp: Local::now(),
                        }));
                    }
                });
                return;
            }
        }

        self.ui_state.is_loading = true;
        self.ui_state.loading_tick = 0;
        // Don't auto-scroll here, let the push_user handle it or user control

        let orchestrator = self.orchestrator.clone();
        let session_id = self.session_id.clone();
        let persona = self.persona.clone();
        let tx = self.response_tx.clone();

        let event_tx = tx.clone();
        let event_bus = orchestrator.event_bus().cloned();

        tokio::spawn(async move {
            let event_handle = if let Some(bus) = event_bus {
                let mut rx = bus.subscribe();
                let etx = event_tx.clone();
                Some(tokio::spawn(async move {
                    while let Ok(event) = rx.recv().await {
                        match event {
                            cratos_core::event_bus::OrchestratorEvent::ExecutionStarted {
                                execution_id,
                                ..
                            } => {
                                let _ = etx.send(AppEvent::ExecutionStarted(execution_id));
                            }
                            cratos_core::event_bus::OrchestratorEvent::ToolStarted {
                                tool_name,
                                ..
                            } => {
                                let _ = etx.send(AppEvent::Chat(ChatMessage {
                                    role: Role::System,
                                    sender: "system".into(),
                                    content: format!("[{}] executing...", tool_name),
                                    timestamp: Local::now(),
                                }));
                            }
                            cratos_core::event_bus::OrchestratorEvent::ToolCompleted {
                                tool_name,
                                success,
                                duration_ms,
                                ..
                            } => {
                                let status = if success { "OK" } else { "FAILED" };
                                let _ = etx.send(AppEvent::Chat(ChatMessage {
                                    role: Role::System,
                                    sender: "system".into(),
                                    content: format!(
                                        "[{}] {} ({}ms)",
                                        tool_name, status, duration_ms
                                    ),
                                    timestamp: Local::now(),
                                }));
                            }
                            cratos_core::event_bus::OrchestratorEvent::ExecutionCompleted {
                                execution_id,
                                ..
                            }
                            | cratos_core::event_bus::OrchestratorEvent::ExecutionFailed {
                                execution_id,
                                ..
                            } => {
                                let _ = etx.send(AppEvent::ExecutionEnded(execution_id));
                                break;
                            }
                            cratos_core::event_bus::OrchestratorEvent::ExecutionCancelled {
                                execution_id,
                            } => {
                                let _ = etx.send(AppEvent::ExecutionEnded(execution_id));
                                break;
                            }
                            _ => {}
                        }
                    }
                }))
            } else {
                None
            };

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

            let _ = tx.send(AppEvent::Chat(msg));

            if let Some(handle) = event_handle {
                handle.abort();
            }
        });
    }

    pub fn poll_responses(&mut self) {
        while let Ok(event) = self.response_rx.try_recv() {
            match event {
                AppEvent::Chat(msg) => {
                    self.ui_state.is_loading = false;
                    self.messages.push(msg);
                    self.scroll_to_bottom();
                }
                AppEvent::ExecutionStarted(id) => {
                    self.current_execution_id = Some(id);
                    self.ui_state.is_loading = true; // Ensure loading state
                }
                AppEvent::ExecutionEnded(id) => {
                    if self.current_execution_id == Some(id) {
                        self.current_execution_id = None;
                        self.ui_state.is_loading = false;
                    }
                }
            }
        }
    }

    pub fn has_active_execution(&self) -> bool {
        self.current_execution_id.is_some()
    }

    pub fn abort_current_execution(&mut self) {
        if let Some(exec_id) = self.current_execution_id {
            if let Some(handle) = self.orchestrator.get_steer_handle(exec_id) {
                let tx = self.response_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle
                        .abort(Some("Aborted by user via TUI".to_string()))
                        .await
                    {
                        let _ = tx.send(AppEvent::Chat(ChatMessage {
                            role: Role::System,
                            sender: "system".into(),
                            content: format!("Failed to abort: {}", e),
                            timestamp: Local::now(),
                        }));
                    }
                });
                self.push_system(format!("Sending abort signal to execution {}...", exec_id));
            } else {
                self.push_system("No steering handle found for active execution.".to_string());
            }
        }
    }

    pub fn refresh_quota(&mut self) {
        let tracker = cratos_llm::global_quota_tracker();
        let states = tracker.try_get_all_states();

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

    pub fn refresh_cost(&mut self) {
        let tracker = cratos_llm::global_tracker();
        if let Some(stats) = tracker.try_get_stats() {
            if stats.total_requests == 0 {
                self.cost_line = String::new();
            } else {
                let is_free = cratos_llm::cli_auth::get_all_auth_sources()
                    .values()
                    .any(|s| *s != cratos_llm::cli_auth::AuthSource::ApiKey);
                if is_free {
                    self.cost_line = format!("{} req (free tier)", stats.total_requests);
                } else {
                    self.cost_line =
                        format!("~${:.4} ({} req)", stats.total_cost, stats.total_requests,);
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.ui_state.tick_count = self.ui_state.tick_count.wrapping_add(1);
        if self.ui_state.is_loading {
            self.ui_state.loading_tick = self.ui_state.loading_tick.wrapping_add(1);
        }
        if self.ui_state.tick_count % 4 == 0 {
            self.refresh_quota();
            self.refresh_cost();
        }
    }

    pub fn abort_execution(&mut self) {
        if let Some(exec_id) = self.current_execution_id {
            if let Some(handle) = self.orchestrator.get_steer_handle(exec_id) {
                let tx = self.response_tx.clone();
                // We spawn this because handle.abort is async
                tokio::spawn(async move {
                    if let Err(e) = handle
                        .abort(Some("User requested abort via TUI".to_string()))
                        .await
                    {
                        let _ = tx.send(AppEvent::Chat(ChatMessage {
                            role: Role::System,
                            sender: "system".into(),
                            content: format!("Failed to abort: {}", e),
                            timestamp: Local::now(),
                        }));
                    }
                });
                self.push_system("Sending abort signal...".into());
            } else {
                self.push_system("No active steering handle found for current execution.".into());
            }
        } else {
            self.push_system("No active execution to abort.".into());
        }
    }
}

fn new_textarea() -> TextArea<'static> {
    let mut ta = TextArea::default();
    ta.set_cursor_line_style(Style::default());
    ta.set_placeholder_text("Type a message... (Enter to send, Esc for Normal Mode)");
    ta.set_max_histories(50);
    ta
}
