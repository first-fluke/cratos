//! History Page
//!
//! Displays execution history from /api/v1/executions.

use leptos::*;
use serde::Deserialize;
use uuid::Uuid;
use wasm_bindgen::JsCast;

use crate::api::ApiClient;

/// Generic API response wrapper matching backend format
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T: Default> {
    #[serde(default)]

    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: T,
    #[serde(default)]
    #[allow(dead_code)]
    error: Option<String>,
}

/// Execution from API (matching backend ExecutionSummary)
#[derive(Debug, Clone, Deserialize)]
pub struct Execution {
    pub id: Uuid,
    /// Backend field: input_text
    #[serde(alias = "input_text")]
    pub input_preview: Option<String>,
    pub status: String,
    /// Backend field: channel_type
    #[serde(alias = "channel_type")]
    pub channel: String,
    /// Backend field: created_at
    #[serde(alias = "created_at")]
    pub started_at: Option<String>,
    /// Backend field: completed_at (for duration calculation)
    #[serde(default)]
    pub completed_at: Option<String>,
}

impl Execution {
    /// Calculate duration in milliseconds from timestamps
    pub fn duration_ms(&self) -> Option<u64> {
        let created = self.started_at.as_ref()?;
        let completed = self.completed_at.as_ref()?;
        let created_dt = chrono::DateTime::parse_from_rfc3339(created).ok()?;
        let completed_dt = chrono::DateTime::parse_from_rfc3339(completed).ok()?;
        let duration = completed_dt.signed_duration_since(created_dt);
        Some(duration.num_milliseconds().max(0) as u64)
    }
}

/// History page showing past executions
#[component]
pub fn History() -> impl IntoView {
    let (executions, set_executions) = create_signal::<Vec<Execution>>(Vec::new());
    let (total, set_total) = create_signal(0usize);
    let (page, set_page) = create_signal(1usize);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);
    let (filter_status, set_filter_status) = create_signal::<Option<String>>(None);

    let per_page = 10;

    // Fetch executions
    let fetch_executions = move || {
        let current_page = page.get();
        let status_filter = filter_status.get();

        spawn_local(async move {
            set_loading.set(true);
            let client = ApiClient::new();

            // Backend uses `limit` parameter, not page/per_page
            let limit = per_page;
            let mut url = format!("/api/v1/executions?limit={}", limit);
            if let Some(status) = status_filter {
                if !status.is_empty() {
                    url.push_str(&format!("&status={}", status));
                }
            }

            match client.get::<ApiResponse<Vec<Execution>>>(&url).await {
                Ok(resp) => {
                    let executions = resp.data;
                    set_total.set(executions.len());
                    set_executions.set(executions);
                    set_error.set(None);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch executions:", e.clone());
                    set_error.set(Some(e));
                }
            }
            set_loading.set(false);
        });
    };

    // Initial fetch
    create_effect(move |_| {
        fetch_executions();
    });

    // Refetch when page or filter changes
    create_effect(move |prev: Option<(usize, Option<String>)>| {
        let current = (page.get(), filter_status.get());
        if prev.is_some() && prev != Some(current.clone()) {
            fetch_executions();
        }
        current
    });

    let total_pages = move || total.get().div_ceil(per_page);

    view! {
        <div class="space-y-6">
            // Header
            <div class="flex items-center justify-between">
                <h1 class="text-3xl font-bold">"Execution History"</h1>
                <div class="flex items-center space-x-4">
                    <select
                        class="px-3 py-2 bg-theme-input-bg border border-theme-border-default rounded-lg focus:outline-none focus:border-theme-primary text-theme-text-primary"
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            set_filter_status.set(if value.is_empty() { None } else { Some(value) });
                            set_page.set(1);
                        }
                    >
                        <option value="">"All Status"</option>
                        <option value="completed">"Completed"</option>
                        <option value="failed">"Failed"</option>
                        <option value="running">"Running"</option>
                    </select>
                    <button
                        class="px-4 py-2 bg-theme-info text-white rounded-lg hover:opacity-90 transition-colors"
                        on:click=move |_| {
                            let data = executions.get();
                            if data.is_empty() {
                                gloo_console::warn!("No executions to export");
                                return;
                            }

                            // Build CSV content
                            let header = "ID,Input,Status,Channel,Started At,Duration (ms)";
                            let rows: Vec<String> = data
                                .iter()
                                .map(|e| {
                                    format!(
                                        "{},{},{},{},{},{}",
                                        e.id,
                                        e.input_preview.as_deref().unwrap_or("").replace(',', ";"),
                                        e.status,
                                        e.channel,
                                        e.started_at.as_deref().unwrap_or(""),
                                        e.duration_ms().map(|d| d.to_string()).unwrap_or_default()
                                    )
                                })
                                .collect();
                            let csv_content = format!("{}\n{}", header, rows.join("\n"));

                            // Create Blob and download
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    let array = js_sys::Array::new();
                                    array.push(&wasm_bindgen::JsValue::from_str(&csv_content));

                                    let mut opts = web_sys::BlobPropertyBag::new();
                                    opts.set_type("text/csv;charset=utf-8");

                                    if let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&array, &opts) {
                                        if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                            if let Ok(a) = document.create_element("a") {
                                                let _ = a.set_attribute("href", &url);
                                                let _ = a.set_attribute("download", "executions.csv");
                                                if let Some(anchor) = a.dyn_ref::<web_sys::HtmlAnchorElement>() {
                                                    anchor.click();
                                                }
                                                let _ = web_sys::Url::revoke_object_url(&url);
                                                gloo_console::log!("Exported", data.len(), "executions");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    >
                        "Export"
                    </button>
                </div>
            </div>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-red-900/50 border border-red-700 rounded-lg p-4 text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Loading indicator
            <Show when=move || loading.get()>
                <div class="text-center text-theme-muted py-8">
                    "Loading..."
                </div>
            </Show>

            // Executions table
            <Show when=move || !loading.get()>
                <div class="bg-theme-card rounded-lg overflow-hidden border border-theme-border-default shadow-sm">
                    <table class="w-full">
                        <thead class="bg-theme-elevated">
                            <tr>
                                <th class="px-6 py-3 text-left text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Input"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Status"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Channel"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Time"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Duration"
                                </th>
                                <th class="px-6 py-3 text-right text-xs font-medium text-theme-muted uppercase tracking-wider">
                                    "Actions"
                                </th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-theme-border-default">
                            <Show
                                when=move || !executions.get().is_empty()
                                fallback=|| view! {
                                    <tr>
                                        <td colspan="6" class="px-6 py-8 text-center text-theme-muted">
                                            "No executions found"
                                        </td>
                                    </tr>
                                }
                            >
                                <For
                                    each=move || executions.get()
                                    key=|exec| exec.id
                                    let:execution
                                >
                                    <ExecutionRow execution=execution.clone() />
                                </For>
                            </Show>
                        </tbody>
                    </table>
                </div>
            </Show>

            // Pagination
            <div class="flex items-center justify-between pt-4">
                <p class="text-sm text-theme-muted">
                    {move || {
                        let start = (page.get() - 1) * per_page + 1;
                        let end = std::cmp::min(page.get() * per_page, total.get());
                        format!("Showing {}-{} of {} executions", start, end, total.get())
                    }}
                </p>
                <div class="flex space-x-2">
                    <button
                        class="px-3 py-1 bg-theme-button-secondary text-theme-text-secondary rounded hover:bg-theme-button-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                        disabled=move || page.get() <= 1
                        on:click=move |_| set_page.update(|p| *p = p.saturating_sub(1).max(1))
                    >
                        "Previous"
                    </button>
                    <span class="px-3 py-1 text-theme-muted">
                        {move || format!("{} / {}", page.get(), total_pages())}
                    </span>
                    <button
                        class="px-3 py-1 bg-theme-button-secondary text-theme-text-secondary rounded hover:bg-theme-button-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                        disabled=move || page.get() >= total_pages()
                        on:click=move |_| set_page.update(|p| *p += 1)
                    >
                        "Next"
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Execution row component
#[component]
fn ExecutionRow(execution: Execution) -> impl IntoView {
    let status_class = match execution.status.as_str() {
        "completed" | "success" => "bg-theme-success/10 text-theme-success border border-theme-success/30",
        "failed" | "error" => "bg-theme-error/10 text-theme-error border border-theme-error/30",
        "running" | "in_progress" => "bg-theme-warning/10 text-theme-warning border border-theme-warning/30",
        _ => "bg-theme-secondary/10 text-theme-secondary border border-theme-secondary/30",
    };

    let time_display = execution
        .started_at
        .clone()
        .unwrap_or_else(|| "--".to_string());

    let duration_display = execution
        .duration_ms()
        .map(|d| format!("{}ms", d))
        .unwrap_or_else(|| "-".to_string());

    let exec_id = execution.id;

    view! {
        <tr class="hover:bg-theme-elevated transition-colors bg-theme-card">
            <td class="px-6 py-4">
                <div class="max-w-xs truncate text-sm text-theme-text-primary">
                    {execution.input_preview.clone().unwrap_or_else(|| execution.id.to_string())}
                </div>
            </td>
            <td class="px-6 py-4">
                <span class={format!("px-2 py-1 text-xs font-medium rounded {}", status_class)}>
                    {execution.status.clone()}
                </span>
            </td>
            <td class="px-6 py-4 text-sm text-theme-muted">
                {execution.channel.clone()}
            </td>
            <td class="px-6 py-4 text-sm text-theme-muted">
                {time_display}
            </td>
            <td class="px-6 py-4 text-sm text-theme-muted">
                {duration_display}
            </td>
            <td class="px-6 py-4 text-right">
                <a
                    href={format!("/history/{}", exec_id)}
                    class="text-theme-primary hover:text-theme-primary/80 text-sm font-medium mr-4 transition-colors"
                >
                    "View"
                </a>
                <button
                    class="text-theme-muted hover:text-theme-text-primary text-sm transition-colors"
                    on:click=move |_| {
                        // Trigger replay via API
                        spawn_local(async move {
                            let client = ApiClient::new();
                            let url = format!("/api/v1/executions/{}/rerun", exec_id);
                            match client.post::<serde_json::Value, _>(&url, &()).await {
                                Ok(_) => gloo_console::log!("Replay started"),
                                Err(e) => gloo_console::error!("Replay failed:", e),
                            }
                        });
                    }
                >
                    "Replay"
                </button>
            </td>
        </tr>
    }
}
