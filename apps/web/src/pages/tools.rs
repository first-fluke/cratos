//! Tools Page - 도구 관리

use leptos::*;
use serde::Deserialize;

use crate::api::ApiClient;

/// Tool information from API
#[derive(Debug, Clone, Deserialize)]
struct ToolInfo {
    name: String,
    description: String,
    category: String,
    requires_approval: bool,
    #[serde(default)]
    parameters: serde_json::Value,
}

/// API response wrapper
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T: Default> {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: T,
}

/// Tools page component
#[component]
pub fn Tools() -> impl IntoView {
    let (tools, set_tools) = create_signal::<Vec<ToolInfo>>(Vec::new());
    let (loading, set_loading) = create_signal(true);
    let (filter, set_filter) = create_signal(String::new());
    let (category_filter, set_category_filter) = create_signal::<Option<String>>(None);

    // Fetch tools on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();
            match client.get::<ApiResponse<Vec<ToolInfo>>>("/api/v1/tools").await {
                Ok(resp) => set_tools.set(resp.data),
                Err(e) => gloo_console::error!("Failed to load tools:", e),
            }
            set_loading.set(false);
        });
    });

    // Get unique categories
    let categories = move || {
        let mut cats: Vec<String> = tools
            .get()
            .iter()
            .map(|t| t.category.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    };

    // Filtered tools
    let filtered_tools = move || {
        let query = filter.get().to_lowercase();
        let cat = category_filter.get();
        tools
            .get()
            .into_iter()
            .filter(|t| {
                let matches_text = query.is_empty()
                    || t.name.to_lowercase().contains(&query)
                    || t.description.to_lowercase().contains(&query);
                let matches_cat = cat.as_ref().is_none_or(|c| &t.category == c);
                matches_text && matches_cat
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="space-y-6">
            // Header
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-3xl font-bold">"Tools"</h1>
                    <p class="text-theme-muted mt-1">
                        "Manage available tools and their permissions"
                    </p>
                </div>
                <div class="text-sm text-theme-muted">
                    {move || format!("{} tools", tools.get().len())}
                </div>
            </div>

            // Filters
            <div class="flex flex-col sm:flex-row gap-4">
                <input
                    type="text"
                    placeholder="Search tools..."
                    class="flex-1 px-4 py-2 bg-theme-input text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                    on:input=move |ev| set_filter.set(event_target_value(&ev))
                />
                <select
                    class="px-4 py-2 bg-theme-input text-theme-primary border border-theme-default rounded-lg focus:outline-none focus:border-theme-info"
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        set_category_filter.set(if value.is_empty() { None } else { Some(value) });
                    }
                >
                    <option value="">"All Categories"</option>
                    <For
                        each=categories
                        key=|cat| cat.clone()
                        let:cat
                    >
                        <option value=cat.clone()>{cat}</option>
                    </For>
                </select>
            </div>

            // Tools grid
            <Show
                when=move || !loading.get()
                fallback=|| view! {
                    <div class="text-center text-theme-muted py-8">
                        "Loading tools..."
                    </div>
                }
            >
                <Show
                    when=move || !filtered_tools().is_empty()
                    fallback=|| view! {
                        <div class="text-center text-theme-muted py-8">
                            "No tools found matching your criteria"
                        </div>
                    }
                >
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        <For
                            each=filtered_tools
                            key=|tool| tool.name.clone()
                            let:tool
                        >
                            <ToolCard tool=tool />
                        </For>
                    </div>
                </Show>
            </Show>
        </div>
    }
}

/// Tool card component
#[component]
fn ToolCard(tool: ToolInfo) -> impl IntoView {
    let (enabled, set_enabled) = create_signal(true);
    let (expanded, set_expanded) = create_signal(false);
    let name = tool.name.clone();

    let toggle_tool = move |_| {
        let new_state = !enabled.get();
        set_enabled.set(new_state);

        // API call to persist (future implementation)
        let tool_name = name.clone();
        spawn_local(async move {
            let client = ApiClient::new();
            let endpoint = if new_state {
                format!("/api/v1/tools/{}/enable", tool_name)
            } else {
                format!("/api/v1/tools/{}/disable", tool_name)
            };
            // Best-effort, ignore errors for now (API not implemented)
            let _ = client.put::<serde_json::Value, ()>(&endpoint, &()).await;
        });
    };

    let category_badge = match tool.category.as_str() {
        "System" => "bg-theme-info/10 text-theme-info border-theme-info/30",
        "Network" => "bg-theme-success/10 text-theme-success border-theme-success/30",
        "FileSystem" => "bg-theme-warning/10 text-theme-warning border-theme-warning/30",
        "Code" => "bg-purple-500/10 text-purple-500 border-purple-500/30", // Custom semantic? or keep hardcoded for distinct categories? keeping consistent style
        "Search" => "bg-cyan-500/10 text-cyan-500 border-cyan-500/30",
        "Communication" => "bg-pink-500/10 text-pink-500 border-pink-500/30",
        _ => "bg-theme-secondary/10 text-theme-secondary border-theme-secondary/30",
    };

    // Check if tool has parameters to show
    let has_params = !tool.parameters.is_null()
        && tool.parameters.as_object().is_some_and(|o| !o.is_empty());

    view! {
        <div class="bg-theme-card rounded-lg p-4 border border-theme-border-default hover:border-theme-primary/50 transition-colors shadow-sm">
            <div class="flex items-start justify-between">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                        <h3 class="font-semibold truncate text-theme-text-primary">{&tool.name}</h3>
                        <Show when=move || tool.requires_approval>
                            <span class="px-1.5 py-0.5 text-xs bg-theme-error/10 text-theme-error border border-theme-error/30 rounded">
                                "approval"
                            </span>
                        </Show>
                    </div>
                    <p class="text-sm text-theme-muted mt-1 line-clamp-2">{&tool.description}</p>
                    <div class="flex items-center gap-2 mt-2">
                        <span class=format!("px-2 py-0.5 text-xs rounded border {}", category_badge)>
                            {&tool.category}
                        </span>
                        <Show when=move || has_params>
                            <button
                                class="text-xs text-theme-muted hover:text-theme-primary transition-colors"
                                on:click=move |_| set_expanded.update(|e| *e = !*e)
                            >
                                {move || if expanded.get() { "Hide params" } else { "Show params" }}
                            </button>
                        </Show>
                    </div>
                </div>
                <button
                    type="button"
                    class={move || format!("ml-4 relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-theme-info focus:ring-offset-2 {}", 
                        if enabled.get() { "bg-theme-success" } else { "bg-theme-elevated" })}
                    on:click=toggle_tool
                >
                    <span
                        class={move || format!("pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out {}", 
                            if enabled.get() { "translate-x-5" } else { "translate-x-0" })}
                    />
                </button>
            </div>

            // Expanded parameters view
            <Show when=move || expanded.get() && has_params>
                <div class="mt-3 pt-3 border-t border-theme-border-default">
                    <pre class="text-xs text-theme-muted overflow-x-auto bg-theme-base p-2 rounded border border-theme-border-default">
                        {serde_json::to_string_pretty(&tool.parameters).unwrap_or_default()}
                    </pre>
                </div>
            </Show>
        </div>
    }
}
