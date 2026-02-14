//! Settings Page
//!
//! Configuration UI with API integration via /api/v1/config.

use gloo_storage::{LocalStorage, Storage};
use leptos::*;
use serde::{Deserialize, Serialize};

use crate::api::ApiClient;

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

/// Server config from /api/v1/config
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ServerConfig {
    #[serde(default)]
    llm: LlmConfig,
    #[serde(default)]
    channels: ChannelsConfig,
    #[serde(default)]
    security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct LlmConfig {
    #[serde(default)]
    default_provider: String,
    #[serde(default)]
    auto_routing: bool,
    #[serde(default)]
    streaming: bool,
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
    sandbox_mode: String,
}

/// Settings page
#[component]
pub fn Settings() -> impl IntoView {
    let (config, set_config) = create_signal(ServerConfig::default());
    let (loading, set_loading) = create_signal(true);
    let (saving, set_saving) = create_signal(false);
    let (error, set_error) = create_signal::<Option<String>>(None);
    let (success, set_success) = create_signal(false);

    // UI preferences (localStorage)
    let initial_prefs: UiPreferences = LocalStorage::get(UI_PREFS_KEY).unwrap_or_default();
    let (ui_prefs, set_ui_prefs) = create_signal(initial_prefs);

    // Apply theme on mount and when changed
    create_effect(move |_| {
        let prefs = ui_prefs.get();
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(el) = document.document_element() {
                let _ = el.set_attribute("data-theme", &prefs.theme);
            }
        }
    });

    // Save UI preferences to localStorage
    let save_ui_prefs = move |prefs: UiPreferences| {
        let _ = LocalStorage::set(UI_PREFS_KEY, &prefs);
        set_ui_prefs.set(prefs);
    };

    // Fetch config on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();
            match client.get::<ServerConfig>("/api/v1/config").await {
                Ok(cfg) => {
                    set_config.set(cfg);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch config:", e.clone());
                    set_error.set(Some(e));
                }
            }
            set_loading.set(false);
        });
    });

    // Save handler
    let save_config = move |_| {
        let cfg = config.get();
        spawn_local(async move {
            set_saving.set(true);
            set_error.set(None);
            set_success.set(false);

            let client = ApiClient::new();
            match client.put::<ServerConfig, _>("/api/v1/config", &cfg).await {
                Ok(_) => {
                    set_success.set(true);
                    gloo_console::log!("Config saved");
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
            <h1 class="text-3xl font-bold">"Settings"</h1>

            // Loading indicator
            <Show when=move || loading.get()>
                <div class="text-center text-gray-400 py-8">"Loading configuration..."</div>
            </Show>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-red-900/50 border border-red-700 rounded-lg p-4 text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Success message
            <Show when=move || success.get()>
                <div class="bg-green-900/50 border border-green-700 rounded-lg p-4 text-green-200">
                    "Settings saved successfully!"
                </div>
            </Show>

            // Settings sections
            <Show when=move || !loading.get()>
                <div class="space-y-6">
                    <SettingsSection title="General">
                        <SettingItem label="Theme" description="Choose your preferred color scheme">
                            <select
                                class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    let mut prefs = ui_prefs.get();
                                    prefs.theme = value;
                                    save_ui_prefs(prefs);
                                }
                            >
                                <option value="dark" selected=move || ui_prefs.get().theme == "dark">"Dark"</option>
                                <option value="light" selected=move || ui_prefs.get().theme == "light">"Light"</option>
                                <option value="system" selected=move || ui_prefs.get().theme == "system">"System"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Language" description="Interface language">
                            <select
                                class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    let mut prefs = ui_prefs.get();
                                    prefs.language = value;
                                    save_ui_prefs(prefs);
                                }
                            >
                                <option value="en" selected=move || ui_prefs.get().language == "en">"English"</option>
                                <option value="ko" selected=move || ui_prefs.get().language == "ko">"한국어"</option>
                                <option value="ja" selected=move || ui_prefs.get().language == "ja">"日本語"</option>
                            </select>
                        </SettingItem>
                    </SettingsSection>

                    <SettingsSection title="LLM Configuration">
                        <SettingItem label="Default Provider" description="Primary LLM provider for requests">
                            <select
                                class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.llm.default_provider = value);
                                }
                            >
                                <option value="groq" selected=move || config.get().llm.default_provider == "groq">"Groq (Free)"</option>
                                <option value="deepseek" selected=move || config.get().llm.default_provider == "deepseek">"DeepSeek (Low Cost)"</option>
                                <option value="anthropic" selected=move || config.get().llm.default_provider == "anthropic">"Anthropic (Premium)"</option>
                                <option value="openai" selected=move || config.get().llm.default_provider == "openai">"OpenAI"</option>
                                <option value="gemini" selected=move || config.get().llm.default_provider == "gemini">"Gemini"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Auto Routing" description="Automatically select the best model based on task complexity">
                            <ToggleSwitch
                                checked=move || config.get().llm.auto_routing
                                on_change=move |v| set_config.update(|c| c.llm.auto_routing = v)
                            />
                        </SettingItem>
                        <SettingItem label="Streaming" description="Enable streaming responses for real-time output">
                            <ToggleSwitch
                                checked=move || config.get().llm.streaming
                                on_change=move |v| set_config.update(|c| c.llm.streaming = v)
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

                    <SettingsSection title="Security" id="security">
                        <SettingItem label="Approval Mode" description="When to require user approval for tool execution">
                            <select
                                class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.security.approval_mode = value);
                                }
                            >
                                <option value="risky_only" selected=move || config.get().security.approval_mode == "risky_only">"Risky Operations Only"</option>
                                <option value="always" selected=move || config.get().security.approval_mode == "always">"Always"</option>
                                <option value="never" selected=move || config.get().security.approval_mode == "never">"Never"</option>
                            </select>
                        </SettingItem>
                        <SettingItem label="Sandbox Mode" description="Run dangerous tools in isolated containers">
                            <select
                                class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_config.update(|c| c.security.sandbox_mode = value);
                                }
                            >
                                <option value="moderate" selected=move || config.get().security.sandbox_mode == "moderate">"Moderate"</option>
                                <option value="strict" selected=move || config.get().security.sandbox_mode == "strict">"Strict"</option>
                                <option value="disabled" selected=move || config.get().security.sandbox_mode == "disabled">"Disabled"</option>
                            </select>
                        </SettingItem>
                    </SettingsSection>

                    <SettingsSection title="Tools" id="tools">
                        <ToolsList />
                    </SettingsSection>
                </div>

                // Save button
                <div class="flex justify-end pt-4 border-t border-gray-700">
                    <button
                        class="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
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
        <div id=id class="bg-gray-800 rounded-lg p-6">
            <h2 class="text-xl font-semibold mb-4">{title}</h2>
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
        <div class="flex items-center justify-between py-3 border-b border-gray-700 last:border-0">
            <div>
                <p class="font-medium">{label}</p>
                <p class="text-sm text-gray-400">{description}</p>
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
            class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
            class:bg-blue-600=move || checked()
            class:bg-gray-600=move || !checked()
            on:click=move |_| on_change(!checked())
        >
            <span
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                class:translate-x-5=move || checked()
                class:translate-x-0=move || !checked()
            />
        </button>
    }
}

/// Tools list component - fetches from API
#[component]
fn ToolsList() -> impl IntoView {
    #[derive(Debug, Clone, Deserialize)]
    struct ToolInfo {
        name: String,
        description: String,
        #[serde(default)]
        enabled: bool,
    }

    #[derive(Debug, Clone, Deserialize, Default)]
    struct ToolsResponse {
        #[serde(default)]
        tools: Vec<ToolInfo>,
    }

    let (tools, set_tools) = create_signal::<Vec<ToolInfo>>(Vec::new());

    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();
            if let Ok(resp) = client.get::<ToolsResponse>("/api/v1/tools").await {
                set_tools.set(resp.tools);
            }
        });
    });

    view! {
        <div class="space-y-4">
            <Show
                when=move || !tools.get().is_empty()
                fallback=|| view! { <p class="text-gray-400">"Loading tools..."</p> }
            >
                <For
                    each=move || tools.get()
                    key=|tool| tool.name.clone()
                    let:tool
                >
                    <ToolCard
                        name=tool.name.clone()
                        description=tool.description.clone()
                        enabled=tool.enabled
                    />
                </For>
            </Show>
        </div>
    }
}

/// Tool card component
#[component]
fn ToolCard(name: String, description: String, enabled: bool) -> impl IntoView {
    let (is_enabled, set_is_enabled) = create_signal(enabled);

    view! {
        <div class="flex items-center justify-between p-4 bg-gray-750 rounded-lg">
            <div>
                <h3 class="font-medium">{name}</h3>
                <p class="text-sm text-gray-400">{description}</p>
            </div>
            <button
                type="button"
                class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
                class:bg-blue-600=move || is_enabled.get()
                class:bg-gray-600=move || !is_enabled.get()
                on:click=move |_| set_is_enabled.update(|c| *c = !*c)
            >
                <span
                    class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                    class:translate-x-5=move || is_enabled.get()
                    class:translate-x-0=move || !is_enabled.get()
                />
            </button>
        </div>
    }
}
