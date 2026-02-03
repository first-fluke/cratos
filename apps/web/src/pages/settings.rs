//! Settings Page

use leptos::*;

/// Settings page
#[component]
pub fn Settings() -> impl IntoView {
    view! {
        <div class="space-y-8">
            // Header
            <h1 class="text-3xl font-bold">"Settings"</h1>

            // Settings sections
            <div class="space-y-6">
                <SettingsSection title="General">
                    <SettingItem
                        label="Theme"
                        description="Choose your preferred color scheme"
                    >
                        <select class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500">
                            <option value="dark">"Dark"</option>
                            <option value="light">"Light"</option>
                            <option value="system">"System"</option>
                        </select>
                    </SettingItem>
                    <SettingItem
                        label="Language"
                        description="Interface language"
                    >
                        <select class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500">
                            <option value="en">"English"</option>
                            <option value="ko">"한국어"</option>
                            <option value="ja">"日本語"</option>
                        </select>
                    </SettingItem>
                </SettingsSection>

                <SettingsSection title="LLM Configuration">
                    <SettingItem
                        label="Default Provider"
                        description="Primary LLM provider for requests"
                    >
                        <select class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500">
                            <option value="groq">"Groq (Free)"</option>
                            <option value="deepseek">"DeepSeek (Low Cost)"</option>
                            <option value="anthropic">"Anthropic (Premium)"</option>
                            <option value="openai">"OpenAI"</option>
                        </select>
                    </SettingItem>
                    <SettingItem
                        label="Auto Routing"
                        description="Automatically select the best model based on task complexity"
                    >
                        <ToggleSwitch checked=true />
                    </SettingItem>
                    <SettingItem
                        label="Streaming"
                        description="Enable streaming responses for real-time output"
                    >
                        <ToggleSwitch checked=true />
                    </SettingItem>
                </SettingsSection>

                <SettingsSection title="Channels">
                    <SettingItem
                        label="Telegram"
                        description="Enable Telegram bot integration"
                    >
                        <ToggleSwitch checked=true />
                    </SettingItem>
                    <SettingItem
                        label="Slack"
                        description="Enable Slack workspace integration"
                    >
                        <ToggleSwitch checked=false />
                    </SettingItem>
                    <SettingItem
                        label="Discord"
                        description="Enable Discord server integration"
                    >
                        <ToggleSwitch checked=false />
                    </SettingItem>
                </SettingsSection>

                <SettingsSection title="Security" id="security">
                    <SettingItem
                        label="Approval Mode"
                        description="When to require user approval for tool execution"
                    >
                        <select class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500">
                            <option value="risky_only">"Risky Operations Only"</option>
                            <option value="always">"Always"</option>
                            <option value="never">"Never"</option>
                        </select>
                    </SettingItem>
                    <SettingItem
                        label="Sandbox Mode"
                        description="Run dangerous tools in isolated containers"
                    >
                        <select class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500">
                            <option value="moderate">"Moderate"</option>
                            <option value="strict">"Strict"</option>
                            <option value="disabled">"Disabled"</option>
                        </select>
                    </SettingItem>
                </SettingsSection>

                <SettingsSection title="Skills" id="skills">
                    <div class="space-y-4">
                        <SkillCard
                            name="Web Search"
                            description="Search the web for information"
                            enabled=true
                        />
                        <SkillCard
                            name="Code Execution"
                            description="Execute code in sandboxed environment"
                            enabled=true
                        />
                        <SkillCard
                            name="File Operations"
                            description="Read, write, and manage files"
                            enabled=true
                        />
                        <SkillCard
                            name="Git Operations"
                            description="Git commands and repository management"
                            enabled=true
                        />
                        <SkillCard
                            name="Browser Automation"
                            description="Automate web browser interactions"
                            enabled=false
                        />
                    </div>
                </SettingsSection>
            </div>

            // Save button
            <div class="flex justify-end pt-4 border-t border-gray-700">
                <button class="px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
                    "Save Changes"
                </button>
            </div>
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
fn SettingItem(
    label: &'static str,
    description: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3 border-b border-gray-700 last:border-0">
            <div>
                <p class="font-medium">{label}</p>
                <p class="text-sm text-gray-400">{description}</p>
            </div>
            <div>
                {children()}
            </div>
        </div>
    }
}

/// Toggle switch component
#[component]
fn ToggleSwitch(checked: bool) -> impl IntoView {
    let (is_checked, set_is_checked) = create_signal(checked);

    view! {
        <button
            type="button"
            class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
            class:bg-blue-600=move || is_checked.get()
            class:bg-gray-600=move || !is_checked.get()
            on:click=move |_| set_is_checked.update(|c| *c = !*c)
        >
            <span
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                class:translate-x-5=move || is_checked.get()
                class:translate-x-0=move || !is_checked.get()
            />
        </button>
    }
}

/// Skill card component
#[component]
fn SkillCard(
    name: &'static str,
    description: &'static str,
    enabled: bool,
) -> impl IntoView {
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
