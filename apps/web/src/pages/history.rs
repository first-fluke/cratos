//! History Page

use chrono::{DateTime, Utc};
use leptos::*;
use uuid::Uuid;

/// History page showing past executions
#[component]
pub fn History() -> impl IntoView {
    let (executions, _set_executions) = create_signal(sample_executions());
    let (_filter, set_filter) = create_signal(FilterOptions::default());

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
                            set_filter.update(|f| f.status = Some(value));
                        }
                    >
                        <option value="">"All Status"</option>
                        <option value="completed">"Completed"</option>
                        <option value="failed">"Failed"</option>
                        <option value="running">"Running"</option>
                    </select>
                    <button class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
                        "Export"
                    </button>
                </div>
            </div>

            // Executions table
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
                        <For
                            each=move || executions.get()
                            key=|exec| exec.id
                            let:execution
                        >
                            <ExecutionRow execution=execution.clone() />
                        </For>
                    </tbody>
                </table>
            </div>

            // Pagination
            <div class="flex items-center justify-between">
                <p class="text-sm text-gray-400">
                    "Showing 1-10 of 100 executions"
                </p>
                <div class="flex space-x-2">
                    <button class="px-3 py-1 bg-gray-700 rounded hover:bg-gray-600 disabled:opacity-50" disabled>
                        "Previous"
                    </button>
                    <button class="px-3 py-1 bg-gray-700 rounded hover:bg-gray-600">
                        "Next"
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Execution row component
#[component]
fn ExecutionRow(execution: ExecutionSummary) -> impl IntoView {
    let status_class = match execution.status.as_str() {
        "completed" => "bg-green-900 text-green-300",
        "failed" => "bg-red-900 text-red-300",
        "running" => "bg-yellow-900 text-yellow-300",
        _ => "bg-gray-700 text-gray-300",
    };

    view! {
        <tr class="hover:bg-gray-750 transition-colors">
            <td class="px-6 py-4">
                <div class="max-w-xs truncate text-sm">
                    {execution.input_preview.clone()}
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
                {format_time(&execution.started_at)}
            </td>
            <td class="px-6 py-4 text-sm text-gray-400">
                {execution.duration_ms.map(|d| format!("{}ms", d)).unwrap_or_else(|| "-".to_string())}
            </td>
            <td class="px-6 py-4 text-right">
                <button class="text-blue-400 hover:text-blue-300 text-sm">
                    "View"
                </button>
                <button class="ml-4 text-gray-400 hover:text-gray-300 text-sm">
                    "Replay"
                </button>
            </td>
        </tr>
    }
}

/// Filter options
#[derive(Clone, Default)]
#[allow(dead_code)]
struct FilterOptions {
    status: Option<String>,
    channel: Option<String>,
    date_from: Option<DateTime<Utc>>,
    date_to: Option<DateTime<Utc>>,
}

/// Execution summary for display
#[derive(Clone)]
struct ExecutionSummary {
    id: Uuid,
    input_preview: String,
    status: String,
    channel: String,
    started_at: DateTime<Utc>,
    duration_ms: Option<u64>,
}

/// Format timestamp for display
fn format_time(dt: &DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// Generate sample executions for demo
fn sample_executions() -> Vec<ExecutionSummary> {
    vec![
        ExecutionSummary {
            id: Uuid::new_v4(),
            input_preview: "Search for Rust async patterns".to_string(),
            status: "completed".to_string(),
            channel: "telegram".to_string(),
            started_at: Utc::now(),
            duration_ms: Some(1234),
        },
        ExecutionSummary {
            id: Uuid::new_v4(),
            input_preview: "Generate unit tests for api.rs".to_string(),
            status: "completed".to_string(),
            channel: "slack".to_string(),
            started_at: Utc::now(),
            duration_ms: Some(5678),
        },
        ExecutionSummary {
            id: Uuid::new_v4(),
            input_preview: "Refactor database module".to_string(),
            status: "failed".to_string(),
            channel: "api".to_string(),
            started_at: Utc::now(),
            duration_ms: Some(2000),
        },
        ExecutionSummary {
            id: Uuid::new_v4(),
            input_preview: "Create PR for feature branch".to_string(),
            status: "running".to_string(),
            channel: "telegram".to_string(),
            started_at: Utc::now(),
            duration_ms: None,
        },
    ]
}
