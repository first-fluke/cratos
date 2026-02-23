//! Settings Page
//!
//! Configuration UI with API integration via /api/v1/config.
//! Categorized layout matching the server's AppConfigView structure.

use gloo_storage::{LocalStorage, Storage};
use leptos::*;
use serde::{Deserialize, Serialize};

use crate::api::ApiClient;
use crate::app::ThemeContext;

/// Local UI preferences (stored in localStorage, not server)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct UiPreferences {
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_language")]
    language: String,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

const UI_PREFS_KEY: &str = "cratos_ui_preferences";

/// API response wrapper
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T> {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: T,
}

// ============================================================================
// Server config types — matches AppConfigView from the API
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ServerConfig {
    #[serde(default)]
    general: GeneralConfig,
    #[serde(default)]
    llm: LlmConfig,
    #[serde(default)]
    channels: ChannelsConfig,
    #[serde(default)]
    security: SecurityConfig,
    #[serde(default)]
    tools: ToolsConfig,
    #[serde(default)]
    advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct GeneralConfig {
    #[serde(default)]
    language: String,
    #[serde(default)]
    persona: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct LlmConfig {
    #[serde(default)]
    default_provider: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    model_routing: Option<ModelRoutingConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ModelRoutingConfig {
    #[serde(default)]
    simple: Option<RouteEntry>,
    #[serde(default)]
    general: Option<RouteEntry>,
    #[serde(default)]
    complex: Option<RouteEntry>,
    #[serde(default)]
    fallback: Option<RouteEntry>,
    #[serde(default)]
    auto_downgrade: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct RouteEntry {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    model: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ChannelsConfig {
    #[serde(default)]
    telegram_enabled: bool,
    #[serde(default)]
    slack_enabled: bool,
    #[serde(default)]
    discord_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct SecurityConfig {
    #[serde(default)]
    approval_mode: String,
    #[serde(default)]
    sandbox_policy: String,
    #[serde(default)]
    exec_mode: String,
    #[serde(default)]
    injection_protection: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ToolsConfig {
    #[serde(default)]
    scheduler_enabled: bool,
    #[serde(default)]
    scheduler_check_interval_secs: u64,
    #[serde(default)]
    vector_search_enabled: bool,
    #[serde(default)]
    browser_enabled: bool,
    #[serde(default)]
    mcp_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct AdvancedConfig {
    #[serde(default)]
    server_port: u16,
    #[serde(default)]
    replay_retention_days: u32,
    #[serde(default)]
    redis_url: String,
}

// ============================================================================
// Update request types — all fields optional
// ============================================================================

#[derive(Debug, Serialize, Default)]
struct ConfigUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    general: Option<GeneralUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    llm: Option<LlmUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<ChannelsUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security: Option<SecurityUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<ToolsUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    advanced: Option<AdvancedUpdate>,
}

#[derive(Debug, Serialize)]
struct GeneralUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    persona: Option<String>,
}

#[derive(Debug, Serialize)]
struct LlmUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChannelsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    telegram_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slack_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discord_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SecurityUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    injection_protection: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ToolsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vector_search_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct AdvancedUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    server_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay_retention_days: Option<u32>,
}

/// Update response from server
#[derive(Debug, Clone, Deserialize, Default)]
struct ConfigUpdateResponse {
    #[serde(default)]
    config: ServerConfig,
    #[serde(default)]
    requires_restart: Vec<String>,
}

/// Settings page
#[component]
pub fn Settings() -> impl IntoView {
    let (loading, set_loading) = create_signal(true);
    let (saving, set_saving) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);
    let (success, set_success) = create_signal(false);
    let (restart_warning, set_restart_warning) = create_signal::<Vec<String>>(Vec::new());

    // Config resource
    let config_resource = create_resource(
        || (),
        |_| async move {
            let client = ApiClient::new();
            match client.get::<ApiResponse<ServerConfig>>("/api/v1/config").await {
                Ok(resp) => Ok(resp.data),
                Err(e) => Err(e),
            }
        },
    );

    // Derived signal for config
    let (config, set_config) = create_signal(ServerConfig::default());

    // Track original config to compute delta for save
    let (original_config, set_original_config) = create_signal(ServerConfig::default());

    // Sync config signal when resource loads
    create_effect(move |_| {
        if let Some(Ok(data)) = config_resource.get() {
            set_config.set(data.clone());
            set_original_config.set(data);
            set_loading.set(false);
        } else if let Some(Err(e)) = config_resource.get() {
            set_error.set(Some(e));
            set_loading.set(false);
        }
    });

    // UI preferences (localStorage)
    let initial_prefs: UiPreferences = LocalStorage::get(UI_PREFS_KEY).unwrap_or_default();
    let (ui_prefs, set_ui_prefs) = create_signal(initial_prefs);

    // Get theme context from app
    let theme_ctx = use_context::<ThemeContext>();

    // Save UI preferences to localStorage and update theme context
    let save_ui_prefs = move |prefs: UiPreferences| {
        let _ = LocalStorage::set("theme", &prefs.theme);
        let _ = LocalStorage::set(UI_PREFS_KEY, &prefs);
        set_ui_prefs.set(prefs.clone());

        if let Some(ctx) = theme_ctx {
            let is_dark = prefs.theme != "light";
            ctx.set_dark.set(is_dark);
        }
    };

    // Build update request from current vs original config
    let build_update = move || -> ConfigUpdateRequest {
        let cfg = config.get();
        let orig = original_config.get();
        let mut req = ConfigUpdateRequest::default();

        // General
        if cfg.general.language != orig.general.language
            || cfg.general.persona != orig.general.persona
        {
            req.general = Some(GeneralUpdate {
                language: if cfg.general.language != orig.general.language {
                    Some(cfg.general.language.clone())
                } else {
                    None
                },
                persona: if cfg.general.persona != orig.general.persona {
                    Some(cfg.general.persona.clone())
                } else {
                    None
                },
            });
        }

        // LLM
        if cfg.llm.default_provider != orig.llm.default_provider
            || cfg.llm.model != orig.llm.model
        {
            req.llm = Some(LlmUpdate {
                default_provider: if cfg.llm.default_provider != orig.llm.default_provider {
                    Some(cfg.llm.default_provider.clone())
                } else {
                    None
                },
                model: if cfg.llm.model != orig.llm.model {
                    Some(cfg.llm.model.clone())
                } else {
                    None
                },
            });
        }

        // Channels
        if cfg.channels.telegram_enabled != orig.channels.telegram_enabled
            || cfg.channels.slack_enabled != orig.channels.slack_enabled
            || cfg.channels.discord_enabled != orig.channels.discord_enabled
        {
            req.channels = Some(ChannelsUpdate {
                telegram_enabled: if cfg.channels.telegram_enabled
                    != orig.channels.telegram_enabled
                {
                    Some(cfg.channels.telegram_enabled)
                } else {
                    None
                },
                slack_enabled: if cfg.channels.slack_enabled != orig.channels.slack_enabled {
                    Some(cfg.channels.slack_enabled)
                } else {
                    None
                },
                discord_enabled: if cfg.channels.discord_enabled != orig.channels.discord_enabled {
                    Some(cfg.channels.discord_enabled)
                } else {
                    None
                },
            });
        }

        // Security
        if cfg.security.approval_mode != orig.security.approval_mode
            || cfg.security.exec_mode != orig.security.exec_mode
            || cfg.security.injection_protection != orig.security.injection_protection
        {
            req.security = Some(SecurityUpdate {
                approval_mode: if cfg.security.approval_mode != orig.security.approval_mode {
                    Some(cfg.security.approval_mode.clone())
                } else {
                    None
                },
                exec_mode: if cfg.security.exec_mode != orig.security.exec_mode {
                    Some(cfg.security.exec_mode.clone())
                } else {
                    None
                },
                injection_protection: if cfg.security.injection_protection
                    != orig.security.injection_protection
                {
                    Some(cfg.security.injection_protection)
                } else {
                    None
                },
            });
        }

        // Tools
        if cfg.tools.scheduler_enabled != orig.tools.scheduler_enabled
            || cfg.tools.vector_search_enabled != orig.tools.vector_search_enabled
        {
            req.tools = Some(ToolsUpdate {
                scheduler_enabled: if cfg.tools.scheduler_enabled != orig.tools.scheduler_enabled {
                    Some(cfg.tools.scheduler_enabled)
                } else {
                    None
                },
                vector_search_enabled: if cfg.tools.vector_search_enabled
                    != orig.tools.vector_search_enabled
                {
                    Some(cfg.tools.vector_search_enabled)
                } else {
                    None
                },
            });
        }

        // Advanced
        if cfg.advanced.server_port != orig.advanced.server_port
            || cfg.advanced.replay_retention_days != orig.advanced.replay_retention_days
        {
            req.advanced = Some(AdvancedUpdate {
                server_port: if cfg.advanced.server_port != orig.advanced.server_port {
                    Some(cfg.advanced.server_port)
                } else {
                    None
                },
                replay_retention_days: if cfg.advanced.replay_retention_days
                    != orig.advanced.replay_retention_days
                {
                    Some(cfg.advanced.replay_retention_days)
                } else {
                    None
                },
            });
        }

        req
    };

    // Save handler
    let save_config = move |_| {
        let update_req = build_update();
        spawn_local(async move {
            set_saving.set(true);
            set_error.set(None);
            set_success.set(false);
            set_restart_warning.set(Vec::new());

            let client = ApiClient::new();
            match client
                .put::<ApiResponse<ConfigUpdateResponse>, _>("/api/v1/config", &update_req)
                .await
            {
                Ok(resp) => {
                    if resp.success {
                        let data = resp.data;
                        if !data.requires_restart.is_empty() {
                            set_restart_warning.set(data.requires_restart);
                        }
                        set_config.set(data.config.clone());
                        set_original_config.set(data.config);
                        set_success.set(true);
                        set_timeout(
                            move || set_success.set(false),
                            std::time::Duration::from_secs(3),
                        );
                    } else {
                        set_error.set(Some("Server failed to save configuration".to_string()));
                    }
                }
                Err(e) => {
                    set_error.set(Some(e));
                }
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="space-y-8">
            <h1 class="text-3xl font-bold text-theme-primary">"Settings"</h1>

            <Show when=move || loading.get()>
                <div class="text-center text-theme-muted py-8">"Loading configuration..."</div>
            </Show>

            <Show when=move || error.get().is_some()>
                <div class="bg-theme-error/10 border border-theme-error rounded-lg p-4 text-theme-error">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || success.get()>
                <div class="bg-theme-success/10 border border-theme-success rounded-lg p-4 text-theme-success">
                    "Settings saved successfully!"
                </div>
            </Show>

            <Show when=move || !restart_warning.get().is_empty()>
                <div class="bg-yellow-500/10 border border-yellow-500 rounded-lg p-4 text-yellow-500">
                    <p class="font-medium">"Some changes require a server restart:"</p>
                    <ul class="list-disc list-inside mt-1">
                        {move || restart_warning.get().iter().map(|f| view! { <li>{f.clone()}</li> }).collect::<Vec<_>>()}
                    </ul>
                </div>
            </Show>

            <Show when=move || !loading.get()>
                <div class="space-y-6">
                    // ── General ──
                    <SettingsSection title="General">
                        <SettingItem label="Theme" description="Choose your preferred color scheme">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || ui_prefs.get().theme.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    let mut prefs = ui_prefs.get();
                                    prefs.theme = value;
                                    save_ui_prefs(prefs);
                                }
                            >
                                <option value="dark">"Dark"</option>
                                <option value="light">"Light"</option>
                                <option value="system">"System"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Language" description="Interface language">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().general.language.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.general.language = value.clone());
                                    let mut prefs = ui_prefs.get();
                                    prefs.language = value;
                                    save_ui_prefs(prefs);
                                }
                            >
                                <option value="en">"English"</option>
                                <option value="ko">"한국어"</option>
                                <option value="ja">"日本語"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Persona" description="Active AI persona">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().general.persona.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.general.persona = value);
                                }
                            >
                                <option value="cratos">"Cratos"</option>
                                <option value="athena">"Athena (PM)"</option>
                                <option value="sindri">"Sindri (Dev)"</option>
                                <option value="heimdall">"Heimdall (QA)"</option>
                                <option value="mimir">"Mimir (Research)"</option>
                            </select>
                        </SettingItem>
                    </SettingsSection>

                    // ── LLM Configuration ──
                    <SettingsSection title="LLM Configuration">
                        <SettingItem label="Default Provider" description="Primary LLM provider for requests">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().llm.default_provider.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.llm.default_provider = value);
                                }
                            >
                                <option value="">"Auto"</option>
                                <option value="google_pro">"Google AI Pro (Gemini)"</option>
                                <option value="gemini">"Google AI (Free)"</option>
                                <option value="openai">"OpenAI"</option>
                                <option value="anthropic">"Anthropic"</option>
                                <option value="groq">"Groq (Free)"</option>
                                <option value="deepseek">"DeepSeek (Low Cost)"</option>
                                <option value="glm">"GLM (ZhipuAI)"</option>
                                <option value="openrouter">"OpenRouter"</option>
                                <option value="ollama">"Ollama (Local)"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Model" description="Specific model to use (leave empty for provider default)">
                            <input
                                type="text"
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info w-48 placeholder-theme-muted"
                                placeholder="e.g., gemini-2.0-flash"
                                prop:value=move || config.get().llm.model.clone()
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.llm.model = value);
                                }
                            />
                        </SettingItem>
                    </SettingsSection>

                    // ── Channels ──
                    <SettingsSection title="Channels">
                        <SettingItem label="Telegram" description="Enable Telegram bot integration">
                            <ToggleSwitch
                                checked=move || config.get().channels.telegram_enabled
                                on_change=move |v| set_config.update(|c| c.channels.telegram_enabled = v)
                            />
                        </SettingItem>
                        <SettingItem label="Slack" description="Enable Slack workspace integration">
                            <ToggleSwitch
                                checked=move || config.get().channels.slack_enabled
                                on_change=move |v| set_config.update(|c| c.channels.slack_enabled = v)
                            />
                        </SettingItem>
                        <SettingItem label="Discord" description="Enable Discord server integration">
                            <ToggleSwitch
                                checked=move || config.get().channels.discord_enabled
                                on_change=move |v| set_config.update(|c| c.channels.discord_enabled = v)
                            />
                        </SettingItem>
                    </SettingsSection>

                    // ── Security ──
                    <SettingsSection title="Security">
                        <SettingItem label="Approval Mode" description="When to require user approval for tool execution">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().security.approval_mode.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.security.approval_mode = value);
                                }
                            >
                                <option value="risky_only">"Risky Operations Only"</option>
                                <option value="always">"Always"</option>
                                <option value="never">"Never"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Exec Mode" description="Command execution security level">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().security.exec_mode.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.security.exec_mode = value);
                                }
                            >
                                <option value="permissive">"Permissive"</option>
                                <option value="strict">"Strict"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Injection Protection" description="Enable command injection protection">
                            <ToggleSwitch
                                checked=move || config.get().security.injection_protection
                                on_change=move |v| set_config.update(|c| c.security.injection_protection = v)
                            />
                        </SettingItem>
                    </SettingsSection>

                    // ── Tools ──
                    <SettingsSection title="Tools">
                        <SettingItem label="Scheduler" description="Enable proactive scheduled tasks">
                            <ToggleSwitch
                                checked=move || config.get().tools.scheduler_enabled
                                on_change=move |v| set_config.update(|c| c.tools.scheduler_enabled = v)
                            />
                        </SettingItem>
                        <SettingItem label="Vector Search" description="Enable semantic search for context">
                            <ToggleSwitch
                                checked=move || config.get().tools.vector_search_enabled
                                on_change=move |v| set_config.update(|c| c.tools.vector_search_enabled = v)
                            />
                        </SettingItem>
                    </SettingsSection>

                    // ── Integrations ──
                    <SettingsSection title="Integrations" id="integrations">
                        <SettingItem label="Google AI Pro" description="Connect your Google Cloud account for higher quotas">
                            <a
                                href="/api/auth/google/login?pro=true"
                                rel="external"
                                class="px-4 py-2 bg-theme-elevated hover:bg-theme-button-hover text-theme-primary rounded-lg border border-theme-border-default transition-colors flex items-center gap-2"
                            >
                                <svg class="w-5 h-5" viewBox="0 0 24 24">
                                    <path fill="currentColor" d="M12.545,10.239v3.821h5.445c-0.712,2.315-2.647,3.972-5.445,3.972c-3.332,0-6.033-2.701-6.033-6.032s2.701-6.032,6.033-6.032c1.498,0,2.866,0.549,3.921,1.453l2.814-2.814C17.503,2.988,15.139,2,12.545,2C7.021,2,2.543,6.477,2.543,12s4.478,10,10.002,10c8.396,0,10.249-7.85,9.426-11.748L12.545,10.239z"/>
                                </svg>
                                "Connect"
                            </a>
                        </SettingItem>
                    </SettingsSection>

                    // ── Advanced ──
                    <SettingsSection title="Advanced">
                        <SettingItem label="Server Port" description="HTTP server port (requires restart)">
                            <input
                                type="number"
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info w-24"
                                prop:value=move || config.get().advanced.server_port.to_string()
                                on:input=move |ev| {
                                    if let Ok(port) = event_target_value(&ev).parse::<u16>() {
                                        set_config.update(|c| c.advanced.server_port = port);
                                    }
                                }
                            />
                        </SettingItem>
                        <SettingItem label="Replay Retention" description="Days to keep replay history">
                            <input
                                type="number"
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info w-24"
                                prop:value=move || config.get().advanced.replay_retention_days.to_string()
                                on:input=move |ev| {
                                    if let Ok(days) = event_target_value(&ev).parse::<u32>() {
                                        set_config.update(|c| c.advanced.replay_retention_days = days);
                                    }
                                }
                            />
                        </SettingItem>
                    </SettingsSection>
                </div>

                // Save button
                <div class="flex justify-end pt-4 border-t border-theme-default">
                    <button
                        class="px-6 py-2 bg-theme-info text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled=move || saving.get()
                        on:click=save_config
                    >
                        {move || if saving.get() { "Saving..." } else { "Save Changes" }}
                    </button>
                </div>
            </Show>
        </div>
    }
}

/// Settings section component
#[component]
fn SettingsSection(
    title: &'static str,
    #[prop(optional)] id: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <div id=id class="bg-theme-card rounded-lg p-6 border border-theme-default">
            <h2 class="text-xl font-semibold mb-4 text-theme-primary">{title}</h2>
            <div class="space-y-4">
                {children()}
            </div>
        </div>
    }
}

/// Individual setting item
#[component]
fn SettingItem(label: &'static str, description: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3 border-b border-theme-default last:border-0">
            <div>
                <p class="font-medium text-theme-primary">{label}</p>
                <p class="text-sm text-theme-secondary">{description}</p>
            </div>
            <div>{children()}</div>
        </div>
    }
}

/// Toggle switch component with callback
#[component]
fn ToggleSwitch<F>(checked: F, on_change: impl Fn(bool) + Copy + 'static) -> impl IntoView
where
    F: Fn() -> bool + Copy + 'static,
{
    view! {
        <button
            type="button"
            role="switch"
            aria-checked=move || if checked() { "true" } else { "false" }
            class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-theme-info focus:ring-offset-2"
            class:bg-theme-success=checked
            class:bg-theme-elevated=move || !checked()
            on:click=move |_| on_change(!checked())
        >
            <span
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                class:translate-x-5=checked
                class:translate-x-0=move || !checked()
            />
        </button>
    }
}
