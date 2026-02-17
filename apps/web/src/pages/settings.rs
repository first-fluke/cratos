//! Settings Page
//!
//! Configuration UI with API integration via /api/v1/config.

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
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: T,
}

/// Server config from /api/v1/config - matches actual API response format
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ServerConfig {
    #[serde(default)]
    llm_provider: String,
    #[serde(default)]
    llm_model: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    persona: String,
    #[serde(default)]
    approval_mode: String,
    #[serde(default)]
    scheduler_enabled: bool,
    #[serde(default)]
    vector_search_enabled: bool,
    #[serde(default)]
    channels: ChannelsConfig,
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

/// Settings page
#[component]
pub fn Settings() -> impl IntoView {
    let (loading, set_loading) = create_signal(true);
    let (saving, set_saving) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);
    let (success, set_success) = create_signal(false);

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

    // Sync config signal when resource loads
    create_effect(move |_| {
        if let Some(Ok(data)) = config_resource.get() {
            set_config.set(data);
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
        // Store theme in localStorage for persistence
        let _ = LocalStorage::set("theme", &prefs.theme);
        let _ = LocalStorage::set(UI_PREFS_KEY, &prefs);
        set_ui_prefs.set(prefs.clone());

        // Update theme context
        if let Some(ctx) = theme_ctx {
            let is_dark = prefs.theme != "light";
            ctx.set_dark.set(is_dark);
        }
    };

    // Save handler
    let save_config = move |_| {
        let cfg = config.get();
        spawn_local(async move {
            set_saving.set(true);
            set_error.set(None);
            set_success.set(false);

            let client = ApiClient::new();
            // Wrap in ApiResponse if the server returns success: true inside the response
            match client.put::<ApiResponse<ServerConfig>, _>("/api/v1/config", &cfg).await {
                Ok(resp) => {
                    if resp.success {
                        set_success.set(true);
                        gloo_console::log!("Config saved successfully");
                        
                        // Refetch the resource to ensure UI is in sync with server state
                        config_resource.refetch();
                        
                        // Auto-hide success message after 3 seconds
                        set_timeout(move || set_success.set(false), std::time::Duration::from_secs(3));
                    } else {
                        set_error.set(Some("Server failed to save configuration".to_string()));
                    }
                }
                Err(e) => {
                    gloo_console::error!("Failed to save config:", e.clone());
                    set_error.set(Some(e));
                }
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="space-y-8">
            // Header
            <h1 class="text-3xl font-bold text-theme-primary">"Settings"</h1>

            // Loading indicator
            <Show when=move || loading.get()>
                <div class="text-center text-theme-muted py-8">"Loading configuration..."</div>
            </Show>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-theme-error/10 border border-theme-error rounded-lg p-4 text-theme-error">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Success message
            <Show when=move || success.get()>
                <div class="bg-theme-success/10 border border-theme-success rounded-lg p-4 text-theme-success">
                    "Settings saved successfully!"
                </div>
            </Show>

            // Settings sections
            <Show when=move || !loading.get()>
                <div class="space-y-6">
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
                                prop:value=move || config.get().language.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.language = value.clone());
                                    // Also update local prefs for immediate UI feedback if needed, 
                                    // but server config is the source of truth for persistence.
                                    let mut prefs = ui_prefs.get();
                                    prefs.language = value; // Keep local sync just in case
                                    save_ui_prefs(prefs);
                                }
                            >
                                <option value="en">"English"</option>
                                <option value="ko">"한국어"</option>
                                <option value="ja">"日本語"</option>
                            </select>
                        </SettingItem>
                    </SettingsSection>

                    <SettingsSection title="LLM Configuration">
                        <SettingItem label="Default Provider" description="Primary LLM provider for requests">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().llm_provider.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.llm_provider = value);
                                }
                            >
                                <option value="">"Auto"</option>
                                <option value="google_pro">"Google AI Pro (Gemini)"</option>
                                <option value="gemini">"Google AI (Free)"</option>
                                <option value="openai">"OpenAI"</option>
                                <option value="anthropic">"Anthropic"</option>
                                <option value="groq">"Groq (Free)"</option>
                                <option value="deepseek">"DeepSeek (Low Cost)"</option>
                                <option value="ollama">"Ollama (Local)"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Model" description="Specific model to use (leave empty for provider default)">
                            <input
                                type="text"
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info w-48 placeholder-theme-muted"
                                placeholder="e.g., gemini-2.0-flash"
                                prop:value=move || config.get().llm_model.clone()
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.llm_model = value);
                                }
                            />
                        </SettingItem>
                        <SettingItem label="Vector Search" description="Enable semantic search for context">
                            <ToggleSwitch
                                checked=move || config.get().vector_search_enabled
                                on_change=move |v| set_config.update(|c| c.vector_search_enabled = v)
                            />
                        </SettingItem>
                    </SettingsSection>

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

                    <SettingsSection title="Security" id="security">
                        <SettingItem label="Approval Mode" description="When to require user approval for tool execution">
                            <select
                                class="px-3 py-2 bg-theme-card text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                                prop:value=move || config.get().approval_mode.clone()
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.approval_mode = value);
                                }
                            >
                                <option value="">"Default"</option>
                                <option value="risky_only">"Risky Operations Only"</option>
                                <option value="always">"Always"</option>
                                <option value="never">"Never"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Scheduler" description="Enable proactive scheduled tasks">
                            <ToggleSwitch
                                checked=move || config.get().scheduler_enabled
                                on_change=move |v| set_config.update(|c| c.scheduler_enabled = v)
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
            class:bg-theme-elevated=move || !checked() // was bg-gray-600
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