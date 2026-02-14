//! History Page
//!
//! Displays execution history from /api/v1/executions.

use leptos::*;
use serde::Deserialize;
use uuid::Uuid;
use wasm_bindgen::JsCast;

use crate::api::ApiClient;

/// Execution from API
#[derive(Debug, Clone, Deserialize)]
pub struct Execution {
    pub id: Uuid,
    #[serde(default)]
    pub input_preview: Option<String>,
    pub status: String,
    pub channel: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// API response
#[derive(Debug, Clone, Deserialize, Default)]
struct ExecutionsResponse {
    #[serde(default)]
    executions: Vec<Execution>,
    #[serde(default)]
    total: usize,
    #[serde(default)]
    page: usize,
    #[serde(default)]
    per_page: usize,
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

            let mut url = format!(
                "/api/v1/executions?page={}&per_page={}",
                current_page, per_page
            );
            if let Some(status) = status_filter {
                if !status.is_empty() {
                    url.push_str(&format!("&status={}", status));
                }
            }

            match client.get::<ExecutionsResponse>(&url).await {
                Ok(resp) => {
                    set_executions.set(resp.executions);
                    set_total.set(resp.total);
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

    let total_pages = move || (total.get() + per_page - 1) / per_page;

    view! {
        <div class="space-y-6">
            // Header
            <div class="flex items-center justify-between">
                <h1 class="text-3xl font-bold">"Execution History"</h1>
                <div class="flex items-center space-x-4">
                    <select
                        class="px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:border-blue-500"
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
                        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
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
                                        e.duration_ms.map(|d| d.to_string()).unwrap_or_default()
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
                <div class="text-center text-gray-400 py-8">
                    "Loading..."
                </div>
            </Show>

            // Executions table
            <Show when=move || !loading.get()>
                <div class="bg-gray-800 rounded-lg overflow-hidden">
                    <table class="w-full">
                        <thead class="bg-gray-750">
                            <tr>
                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Input"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Status"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Channel"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Time"
                                </th>
                                <th class="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Duration"
                                </th>
                                <th class="px-6 py-3 text-right text-xs font-medium text-gray-400 uppercase tracking-wider">
                                    "Actions"
                                </th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-gray-700">
                            <Show
                                when=move || !executions.get().is_empty()
                                fallback=|| view! {
                                    <tr>
                                        <td colspan="6" class="px-6 py-8 text-center text-gray-400">
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
            <div class="flex items-center justify-between">
                <p class="text-sm text-gray-400">
                    {move || {
                        let start = (page.get() - 1) * per_page + 1;
                        let end = std::cmp::min(page.get() * per_page, total.get());
                        format!("Showing {}-{} of {} executions", start, end, total.get())
                    }}
                </p>
                <div class="flex space-x-2">
                    <button
                        class="px-3 py-1 bg-gray-700 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled=move || page.get() <= 1
                        on:click=move |_| set_page.update(|p| *p = p.saturating_sub(1).max(1))
                    >
                        "Previous"
                    </button>
                    <span class="px-3 py-1 text-gray-400">
                        {move || format!("{} / {}", page.get(), total_pages())}
                    </span>
                    <button
                        class="px-3 py-1 bg-gray-700 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed"
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
        "completed" | "success" => "bg-green-900 text-green-300",
        "failed" | "error" => "bg-red-900 text-red-300",
        "running" | "in_progress" => "bg-yellow-900 text-yellow-300",
        _ => "bg-gray-700 text-gray-300",
    };

    let time_display = execution
        .started_at
        .clone()
        .unwrap_or_else(|| "--".to_string());

    let duration_display = execution
        .duration_ms
        .map(|d| format!("{}ms", d))
        .unwrap_or_else(|| "-".to_string());

    let exec_id = execution.id;

    view! {
        <tr class="hover:bg-gray-750 transition-colors">
            <td class="px-6 py-4">
                <div class="max-w-xs truncate text-sm">
                    {execution.input_preview.clone().unwrap_or_else(|| execution.id.to_string())}
                </div>
            </td>
            <td class="px-6 py-4">
                <span class={format!("px-2 py-1 text-xs font-medium rounded {}", status_class)}>
                    {execution.status.clone()}
                </span>
            </td>
            <td class="px-6 py-4 text-sm text-gray-400">
                {execution.channel.clone()}
            </td>
            <td class="px-6 py-4 text-sm text-gray-400">
                {time_display}
            </td>
            <td class="px-6 py-4 text-sm text-gray-400">
                {duration_display}
            </td>
            <td class="px-6 py-4 text-right">
                <a
                    href={format!("/history/{}", exec_id)}
                    class="text-blue-400 hover:text-blue-300 text-sm"
                >
                    "View"
                </a>
                <button
                    class="ml-4 text-gray-400 hover:text-gray-300 text-sm"
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
