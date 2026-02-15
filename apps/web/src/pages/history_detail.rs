use leptos::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::ApiClient;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ExecutionDetail {
    pub id: Uuid,
    #[serde(default)]
    pub channel_type: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub input_text: String,
    #[serde(default)]
    pub output_text: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub events: Vec<EventSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct EventSummary {
    pub id: Uuid,
    #[serde(default)]
    pub sequence_num: i32,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub duration_ms: Option<i32>,
}

#[component]
pub fn HistoryDetail() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.get().get("id").cloned().unwrap_or_default();

    let (execution, set_execution) = create_signal::<Option<ExecutionDetail>>(None);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    create_effect(move |_| {
        let exec_id = id();
        if !exec_id.is_empty() {
            spawn_local(async move {
                let client = ApiClient::new();
                match client.get::<ApiResponse<ExecutionDetail>>(&format!("/api/v1/executions/{}", exec_id)).await {
                    Ok(resp) => {
                        if resp.success {
                            set_execution.set(resp.data);
                        } else {
                            set_error.set(resp.error.or(Some("Failed to load execution".to_string())));
                        }
                    }
                    Err(e) => set_error.set(Some(e)),
                }
                set_loading.set(false);
            });
        }
    });

    view! {
        <div class="space-y-6 animate-in fade-in">
            <div class="flex items-center gap-4">
                <A href="/history" class="p-2 hover:bg-theme-elevated rounded-lg transition-colors">
                    "← Back"
                </A>
                <h1 class="text-2xl font-bold">"Execution Details"</h1>
            </div>

            <Show when=move || loading.get()>
                <div class="py-12 text-center text-theme-muted">"Loading details..."</div>
            </Show>

            <Show when=move || error.get().is_some()>
                <div class="bg-red-900/20 border border-red-800 p-4 rounded text-red-300">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || execution.get().is_some()>
                {
                    let exec = execution.get().unwrap();
                    let events = exec.events.clone();
                    view! {
                        <div class="bg-theme-card border border-theme-border-default rounded-xl p-6 space-y-6">
                            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                                <div>
                                    <span class="text-theme-muted block">"ID"</span>
                                    <span class="font-mono text-theme-primary">{exec.id.to_string()}</span>
                                </div>
                                <div>
                                    <span class="text-theme-muted block">"Status"</span>
                                    <span class="font-bold uppercase text-theme-primary">{exec.status}</span>
                                </div>
                                <div>
                                    <span class="text-theme-muted block">"Channel"</span>
                                    <span>{exec.channel_type}</span>
                                </div>
                                <div>
                                    <span class="text-theme-muted block">"Created At"</span>
                                    <span>{exec.created_at}</span>
                                </div>
                            </div>

                            <div class="bg-theme-base rounded-lg p-4 font-mono text-sm overflow-x-auto">
                                <div class="text-theme-muted mb-2 text-xs uppercase">"Input Payload"</div>
                                <pre class="whitespace-pre-wrap">{exec.input_text.clone()}</pre>
                            </div>

                            {
                                let (output_text, _) = create_signal(exec.output_text.clone());
                                view! {
                                    <Show when=move || output_text.get().is_some()>
                                        <div class="bg-green-900/10 border border-green-800/30 rounded-lg p-4">
                                            <div class="text-green-400 mb-2 text-xs uppercase font-bold">"Result Output"</div>
                                            <pre class="font-mono text-sm whitespace-pre-wrap">{output_text.get().unwrap_or_default()}</pre>
                                        </div>
                                    </Show>
                                }
                            }

                            // Timeline/Events section
                            <div class="space-y-4">
                                <h3 class="text-lg font-bold">"Execution Timeline"</h3>
                                <div class="space-y-2">
                                    <For
                                        each=move || events.clone()
                                        key=|e| e.id
                                        let:event
                                    >
                                        <div class="flex items-center gap-4 p-3 bg-theme-base/50 rounded-lg border border-theme-border-default text-sm">
                                            <div class="w-24 text-theme-muted text-xs">{event.timestamp.split('T').collect::<Vec<_>>().get(1).unwrap_or(&"").to_string()}</div>
                                            <div class="flex-1 font-medium">{event.event_type}</div>
                                            <div class="text-theme-muted text-xs">
                                                {event.duration_ms.map(|d| format!("{}ms", d)).unwrap_or_default()}
                                            </div>
                                        </div>
                                    </For>
                                </div>
                            </div>
                        </div>
                    }
                }
            </Show>
        </div>
    }
}
