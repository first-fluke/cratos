//! Dashboard Page
//!
//! Real-time dashboard with API-fetched data.

use leptos::*;
use serde::Deserialize;

use crate::api::ApiClient;
use crate::components::{Card, Chart, ChartData, ChartType, DataSeries, StatCard};

/// Health response from /health/detailed
#[derive(Debug, Clone, Deserialize, Default)]
struct HealthResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    services: Vec<ServiceStatus>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServiceStatus {
    name: String,
    status: String,
    #[serde(default)]
    uptime: Option<String>,
}

/// Execution list response from /api/v1/executions
#[derive(Debug, Clone, Deserialize, Default)]
struct ExecutionsResponse {
    #[serde(default)]
    executions: Vec<ExecutionItem>,
    #[serde(default)]
    total: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ExecutionItem {
    id: String,
    input_preview: Option<String>,
    status: String,
    channel: String,
    #[serde(default)]
    duration_ms: Option<u64>,
}

/// Main dashboard page
#[component]
pub fn Dashboard() -> impl IntoView {
    // API-fetched data signals
    let (executions_today, set_executions_today) = create_signal(0usize);
    let (avg_response_time, set_avg_response_time) = create_signal("--".to_string());
    let (active_sessions, set_active_sessions) = create_signal(0usize);
    let (skills_count, set_skills_count) = create_signal(0usize);
    let (recent_executions, set_recent_executions) = create_signal::<Vec<ExecutionItem>>(Vec::new());
    let (services, set_services) = create_signal::<Vec<ServiceStatus>>(Vec::new());
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    // Fetch data on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();

            // Fetch executions
            match client.get::<ExecutionsResponse>("/api/v1/executions?limit=10").await {
                Ok(resp) => {
                    set_executions_today.set(resp.total);

                    // Calculate average response time
                    let durations: Vec<u64> = resp
                        .executions
                        .iter()
                        .filter_map(|e| e.duration_ms)
                        .collect();
                    if !durations.is_empty() {
                        let avg = durations.iter().sum::<u64>() / durations.len() as u64;
                        set_avg_response_time.set(format!("{}ms", avg));
                    }

                    set_recent_executions.set(resp.executions);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch executions:", e.clone());
                    set_error.set(Some(e));
                }
            }

            // Fetch health status
            match client.get::<HealthResponse>("/health/detailed").await {
                Ok(resp) => {
                    set_services.set(resp.services);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch health:", e);
                }
            }

            // Fetch tools count (as proxy for skills)
            #[derive(Deserialize)]
            struct ToolsResponse {
                #[serde(default)]
                tools: Vec<serde_json::Value>,
            }
            if let Ok(resp) = client.get::<ToolsResponse>("/api/v1/tools").await {
                set_skills_count.set(resp.tools.len());
            }

            // Sessions count from quota endpoint
            #[derive(Deserialize)]
            struct QuotaResponse {
                #[serde(default)]
                active_sessions: Option<usize>,
            }
            if let Ok(resp) = client.get::<QuotaResponse>("/api/v1/quota").await {
                set_active_sessions.set(resp.active_sessions.unwrap_or(0));
            }

            set_loading.set(false);
        });
    });

    view! {
        <div class="space-y-8">
            // Header
            <div class="flex items-center justify-between">
                <h1 class="text-3xl font-bold">"Dashboard"</h1>
                <div class="text-sm text-gray-400">
                    {move || if loading.get() { "Loading..." } else { "Live" }}
                </div>
            </div>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-red-900/50 border border-red-700 rounded-lg p-4 text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Stats grid
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard
                    title="Executions Today"
                    value=move || executions_today.get().to_string()
                    icon="chart"
                    color="blue"
                />
                <StatCard
                    title="Avg Response Time"
                    value=move || avg_response_time.get()
                    icon="clock"
                    color="green"
                />
                <StatCard
                    title="Active Sessions"
                    value=move || active_sessions.get().to_string()
                    icon="users"
                    color="purple"
                />
                <StatCard
                    title="Tools"
                    value=move || skills_count.get().to_string()
                    icon="star"
                    color="yellow"
                />
            </div>

            // Main content grid
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                // Recent activity
                <Card title="Recent Activity">
                    <RecentActivity executions=recent_executions />
                </Card>

                // Quick actions
                <Card title="Quick Actions">
                    <QuickActions />
                </Card>
            </div>

            // System status
            <Card title="System Status">
                <SystemStatus services=services />
            </Card>

            // Usage charts
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <Card title="Executions Over Time">
                    <ExecutionsChart />
                </Card>
                <Card title="Model Usage">
                    <ModelUsageChart />
                </Card>
            </div>
        </div>
    }
}

/// Recent activity list from API data
#[component]
fn RecentActivity(executions: ReadSignal<Vec<ExecutionItem>>) -> impl IntoView {
    view! {
        <ul class="divide-y divide-gray-700">
            <Show
                when=move || !executions.get().is_empty()
                fallback=|| view! { <li class="py-3 text-gray-400">"No recent activity"</li> }
            >
                <For
                    each=move || executions.get()
                    key=|exec| exec.id.clone()
                    let:exec
                >
                    {
                        let status_color = match exec.status.as_str() {
                            "completed" | "success" => "text-green-400",
                            "failed" | "error" => "text-red-400",
                            _ => "text-yellow-400",
                        };
                        let duration = exec.duration_ms
                            .map(|d| format!("{}ms", d))
                            .unwrap_or_else(|| "running".to_string());

                        view! {
                            <li class="py-3 flex items-center justify-between">
                                <div>
                                    <p class="text-sm font-medium truncate max-w-xs">
                                        {exec.input_preview.clone().unwrap_or_else(|| exec.id.clone())}
                                    </p>
                                    <p class="text-xs text-gray-500">{exec.channel.clone()} " · " {duration}</p>
                                </div>
                                <span class={format!("text-xs font-medium {}", status_color)}>
                                    {exec.status.clone()}
                                </span>
                            </li>
                        }
                    }
                </For>
            </Show>
        </ul>
    }
}

/// Quick action buttons
#[component]
fn QuickActions() -> impl IntoView {
    view! {
        <div class="grid grid-cols-2 gap-4">
            <ActionButton
                title="New Chat"
                description="Start a new conversation"
                href="/chat"
            />
            <ActionButton
                title="View History"
                description="Browse past executions"
                href="/history"
            />
            <ActionButton
                title="Settings"
                description="Configure Cratos"
                href="/settings"
            />
            <ActionButton
                title="API Docs"
                description="View API documentation"
                href="/docs"
            />
        </div>
    }
}

/// Action button component
#[component]
fn ActionButton(title: &'static str, description: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <a
            href=href
            class="block p-4 bg-gray-800 rounded-lg hover:bg-gray-750 transition-colors border border-gray-700 hover:border-gray-600"
        >
            <h3 class="font-medium text-white">{title}</h3>
            <p class="text-xs text-gray-400 mt-1">{description}</p>
        </a>
    }
}

/// Executions chart component (static for now - would need time-series API)
#[component]
fn ExecutionsChart() -> impl IntoView {
    let data = ChartData {
        title: String::new(),
        labels: vec![
            "Mon".to_string(),
            "Tue".to_string(),
            "Wed".to_string(),
            "Thu".to_string(),
            "Fri".to_string(),
            "Sat".to_string(),
            "Sun".to_string(),
        ],
        series: vec![DataSeries {
            name: "Executions".to_string(),
            values: vec![0.0; 7], // Empty until time-series API is available
            color: Some("#3B82F6".to_string()),
        }],
    };

    view! {
        <Chart
            data=data
            chart_type=ChartType::Area
            width=600
            height=300
            show_legend=false
        />
    }
}

/// Model usage chart component (static for now - would need usage API)
#[component]
fn ModelUsageChart() -> impl IntoView {
    let data = ChartData {
        title: String::new(),
        labels: vec![
            "Groq".to_string(),
            "DeepSeek".to_string(),
            "Claude".to_string(),
            "GPT".to_string(),
        ],
        series: vec![DataSeries {
            name: "Requests".to_string(),
            values: vec![0.0; 4], // Empty until usage API is available
            color: None,
        }],
    };

    view! {
        <Chart
            data=data
            chart_type=ChartType::Bar
            width=600
            height=300
            show_legend=false
        />
    }
}

/// System status component from API data
#[component]
fn SystemStatus(services: ReadSignal<Vec<ServiceStatus>>) -> impl IntoView {
    view! {
        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <Show
                when=move || !services.get().is_empty()
                fallback=|| view! {
                    <div class="col-span-4 text-center text-gray-400 py-4">
                        "Loading system status..."
                    </div>
                }
            >
                <For
                    each=move || services.get()
                    key=|svc| svc.name.clone()
                    let:service
                >
                    {
                        let status_color = match service.status.as_str() {
                            "healthy" | "ok" => "bg-green-500",
                            "degraded" | "warning" => "bg-yellow-500",
                            _ => "bg-red-500",
                        };
                        let uptime = service.uptime.clone().unwrap_or_else(|| "--".to_string());

                        view! {
                            <div class="p-4 bg-gray-800 rounded-lg">
                                <div class="flex items-center space-x-2">
                                    <div class={format!("w-2 h-2 rounded-full {}", status_color)} />
                                    <span class="font-medium text-sm">{service.name.clone()}</span>
                                </div>
                                <p class="text-xs text-gray-400 mt-2">"Uptime: " {uptime}</p>
                            </div>
                        }
                    }
                </For>
            </Show>
        </div>
    }
}
