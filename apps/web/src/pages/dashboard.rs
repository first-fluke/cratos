//! Dashboard Page
//!
//! Real-time dashboard with API-fetched data and premium glassmorphism aesthetic.

use leptos::*;
use leptos_router::A;
use serde::Deserialize;

use crate::api::ApiClient;

// Re-export chrono for timestamp parsing
use chrono;
use crate::components::{Card, Chart, ChartData, ChartType, DataSeries, StatCard};

/// Generic API response wrapper matching backend format
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T: Default> {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: T,
    #[serde(default)]
    error: Option<String>,
}

/// Health response from /health/detailed
#[derive(Debug, Clone, Deserialize, Default)]
struct HealthResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    checks: Option<HealthChecks>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct HealthChecks {
    database: ComponentHealth,
    redis: ComponentHealth,
    llm: ComponentHealth,
    scheduler: ComponentHealth,
    event_bus: ComponentHealth,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ComponentHealth {
    status: String,
    #[serde(default)]
    latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServiceStatus {
    name: String,
    status: String,
    #[serde(default)]
    uptime: Option<String>,
}

/// Execution item matching backend ExecutionSummary
#[derive(Debug, Clone, Deserialize)]
struct ExecutionItem {
    id: String,
    /// Backend field: input_text
    #[serde(alias = "input_text")]
    input_preview: Option<String>,
    status: String,
    /// Backend field: channel_type
    #[serde(alias = "channel_type")]
    channel: String,
    /// Timestamps for duration calculation
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

impl ExecutionItem {
    /// Calculate duration in milliseconds from timestamps
    fn duration_ms(&self) -> Option<u64> {
        let created = self.created_at.as_ref()?;
        let completed = self.completed_at.as_ref()?;
        let created_dt = chrono::DateTime::parse_from_rfc3339(created).ok()?;
        let completed_dt = chrono::DateTime::parse_from_rfc3339(completed).ok()?;
        let duration = completed_dt.signed_duration_since(created_dt);
        Some(duration.num_milliseconds().max(0) as u64)
    }
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
    let (stats, set_stats) = create_signal::<Option<ExecutionStats>>(None);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    // Fetch data on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();

            // Fetch executions
            match client.get::<ApiResponse<Vec<ExecutionItem>>>("/api/v1/executions?limit=10").await {
                Ok(resp) => {
                    let executions = resp.data;
                    set_executions_today.set(executions.len());

                    let durations: Vec<u64> = executions
                        .iter()
                        .filter_map(|e| e.duration_ms())
                        .collect();
                    if !durations.is_empty() {
                        let avg = durations.iter().sum::<u64>() / durations.len() as u64;
                        set_avg_response_time.set(format!("{}ms", avg));
                    }

                    set_recent_executions.set(executions);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch executions:", e.clone());
                    set_error.set(Some(e));
                }
            }

            // Fetch execution stats for chart
            match client.get::<ApiResponse<ExecutionStats>>("/api/v1/executions/stats").await {
                Ok(resp) => {
                    set_stats.set(Some(resp.data));
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch stats:", e);
                }
            }

            // Fetch health status
            match client.get::<HealthResponse>("/health/detailed").await {
                Ok(resp) => {
                    if let Some(checks) = resp.checks {
                        let mut new_services = Vec::new();
                        
                        let map_health = |name: &str, h: ComponentHealth| ServiceStatus {
                            name: name.to_string(),
                            status: h.status,
                            uptime: h.latency_ms.map(|l| format!("{}ms", l)).or(Some("OK".to_string())),
                        };

                        new_services.push(map_health("Database", checks.database));
                        new_services.push(map_health("Redis", checks.redis));
                        new_services.push(map_health("LLM", checks.llm));
                        new_services.push(map_health("Scheduler", checks.scheduler));
                        new_services.push(map_health("EventBus", checks.event_bus));

                        set_services.set(new_services);
                    }
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch health:", e);
                }
            }

            // Fetch tools count
            #[derive(Deserialize)]
            struct ToolsResponse {
                #[serde(default)]
                data: Vec<serde_json::Value>,
            }
            if let Ok(resp) = client.get::<ToolsResponse>("/api/v1/tools").await {
                set_skills_count.set(resp.data.len());
            }

            // Sessions count
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
        <div class="space-y-12 pb-12 animate-in fade-in duration-700">
            // Hero Header
            <div class="relative overflow-hidden rounded-[2rem] p-8 md:p-12 glass-premium border border-white/10 shadow-2xl">
                <div class="absolute -top-24 -right-24 w-96 h-96 bg-blue-500/10 rounded-full blur-[120px] animate-pulse"></div>
                <div class="absolute -bottom-24 -left-24 w-96 h-96 bg-purple-500/10 rounded-full blur-[120px] animate-pulse"></div>
                
                <div class="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-8">
                    <div>
                        <h1 class="text-4xl md:text-6xl font-black tracking-tighter text-theme-primary mb-3">
                            "Command Center"
                        </h1>
                        <p class="text-theme-secondary text-lg md:text-xl font-medium opacity-70 max-w-2xl">
                            "Real-time intelligence and system performance metrics for the Cratos ecosystem."
                        </p>
                    </div>
                    <div class="flex items-center space-x-4 bg-white/5 border border-white/10 rounded-2xl px-6 py-3 backdrop-blur-xl shadow-inner">
                        <div class="relative">
                            <div class="w-3 h-3 bg-green-400 rounded-full shadow-[0_0_15px_rgba(74,222,128,0.6)]"></div>
                            <div class="absolute inset-0 w-3 h-3 bg-green-400 rounded-full animate-ping opacity-40"></div>
                        </div>
                        <span class="text-sm font-black tracking-widest uppercase opacity-80">
                            {move || if loading.get() { "Syncing..." } else { "System Live" }}
                        </span>
                    </div>
                </div>
            </div>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="glass-premium border-red-500/30 rounded-2xl p-6 text-red-400 flex items-center space-x-4 animate-in slide-in-from-top-4">
                    <div class="p-3 bg-red-500/20 rounded-xl shadow-lg">
                        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                        </svg>
                    </div>
                    <p class="font-bold tracking-tight">{move || error.get().unwrap_or_default()}</p>
                </div>
            </Show>

            // Stats grid
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard
                    title="Executions Today"
                    value=move || executions_today.get().to_string()
                    icon="chart"
                    color="blue"
                    trend="+14%".to_string()
                />
                <StatCard
                    title="Avg Latency"
                    value=move || avg_response_time.get()
                    icon="clock"
                    color="purple"
                    trend="-24ms".to_string()
                />
                <StatCard
                    title="Active Sessions"
                    value=move || active_sessions.get().to_string()
                    icon="users"
                    color="green"
                    trend="+2".to_string()
                />
                                <StatCard
                    title="Available Tools"
                    value=move || skills_count.get().to_string()
                    icon="star"
                    color="yellow"
                    trend="Stable".to_string()
                    href="/tools"
                />
            </div>

            // Main Content Grid
            <div class="grid grid-cols-1 xl:grid-cols-3 gap-8 items-start">
                // Recent activity widget grid
                <div class="xl:col-span-2 space-y-6">
                    <div class="flex items-center justify-between px-4">
                        <h2 class="text-2xl font-black tracking-tight text-theme-primary uppercase">"Recent Activity"</h2>
                        <A href="/history" class="text-xs font-black text-blue-400 hover:text-blue-300 transition-colors tracking-widest uppercase bg-blue-500/10 px-4 py-2 rounded-full border border-blue-500/20">
                            "History →"
                        </A>
                    </div>
                    <RecentActivityGrid executions=recent_executions />
                </div>

                // Quick Launch
                <div class="space-y-6">
                    <h2 class="text-2xl font-black tracking-tight text-theme-primary px-4 uppercase">"Quick Launch"</h2>
                    <QuickActions />
                </div>
            </div>

            // Secondary Metrics
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
                <Card title="Traffic Analysis">
                    <ExecutionsChart stats=stats />
                </Card>
                <Card title="Infrastructure Health">
                    <SystemStatus services=services />
                </Card>
            </div>
        </div>
    }
}

/// Recent activity widget grid
#[component]
fn RecentActivityGrid(executions: ReadSignal<Vec<ExecutionItem>>) -> impl IntoView {
    let executions_preview = move || {
        executions.get().into_iter().take(6).collect::<Vec<ExecutionItem>>()
    };

    view! {
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Show
                when=move || !executions.get().is_empty()
                fallback=|| view! { 
                    <div class="col-span-2 glass-premium rounded-3xl p-16 text-center">
                        <div class="w-16 h-16 bg-white/5 rounded-full flex items-center justify-center mx-auto mb-4 border border-white/10">
                            <svg class="w-8 h-8 text-theme-muted" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                        </div>
                        <p class="text-theme-muted font-bold tracking-widest uppercase text-xs">"No activity detected in the last 24h"</p>
                    </div> 
                }
            >
                <For
                    each=executions_preview
                    key=|exec| exec.id.clone()
                    let:exec
                >
                    {
                        let (status_text, status_class) = match exec.status.as_str() {
                            "completed" | "success" => ("Success", "text-green-400 bg-green-500/10 border-green-500/20"),
                            "failed" | "error" => ("Failed", "text-red-400 bg-red-500/10 border-red-500/20"),
                            _ => ("Running", "text-yellow-400 bg-yellow-500/10 border-yellow-500/20"),
                        };
                        let duration = exec.duration_ms()
                            .map(|d| format!("{}ms", d))
                            .unwrap_or_else(|| "active".to_string());

                        view! {
                            <div class="group glass-premium rounded-3xl p-6 border border-white/5 hover:border-white/20 transition-all duration-500 hover:shadow-2xl">
                                <div class="flex justify-between items-start mb-6">
                                    <div class=format!("px-3 py-1 rounded-xl text-[10px] font-black uppercase tracking-widest shadow-inner {}", status_class)>
                                        {status_text}
                                    </div>
                                    <span class="text-[10px] font-black text-theme-muted uppercase tracking-widest opacity-50 group-hover:opacity-100 transition-opacity">
                                        {exec.channel.clone()}
                                    </span>
                                </div>
                                <h3 class="text-lg font-bold text-theme-primary truncate mb-1 tracking-tight">
                                    {exec.input_preview.clone().unwrap_or_else(|| exec.id.clone())}
                                </h3>
                                <div class="flex items-center justify-between mt-6 pt-6 border-t border-white/5">
                                    <span class="text-[10px] font-black text-theme-secondary uppercase tracking-widest opacity-40">"Processing Time"</span>
                                    <span class="text-xs font-mono font-black text-theme-primary bg-white/5 px-3 py-1 rounded-lg border border-white/5">{duration}</span>
                                </div>
                            </div>
                        }
                    }
                </For>
            </Show>
        </div>
    }
}

/// Quick action buttons
#[component]
fn QuickActions() -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 gap-4">
            <ActionButton
                title="New Chat Session"
                description="Initialize fresh AI context"
                href="/chat"
                icon="plus"
                color="blue"
            />
            <ActionButton
                title="Persona Forge"
                description="Craft AI personalities"
                href="/personas"
                icon="user"
                color="purple"
            />
            <ActionButton
                title="Core Configuration"
                description="Fine-tune system parameters"
                href="/settings"
                icon="cog"
                color="gray"
            />
            <ActionButton
                title="API Documentation"
                description="Explore integration hooks"
                href="/docs"
                icon="book"
                color="green"
            />
        </div>
    }
}

/// Action button component
#[component]
fn ActionButton(
    title: &'static str, 
    description: &'static str, 
    href: &'static str,
    icon: &'static str,
    color: &'static str,
) -> impl IntoView {
    let icon_svg = match icon {
        "plus" => view! { <path d="M12 4v16m8-8H4" /> }.into_view(),
        "user" => view! { <path d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" /> }.into_view(),
        "cog" => view! { 
            <>
                <path d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </>
        }.into_view(),
        "book" => view! { <path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /> }.into_view(),
        _ => view! { <path d="M13 10V3L4 14h7v7l9-11h-7z" /> }.into_view(),
    };

    let color_class = match color {
        "blue" => "text-blue-400 bg-blue-500/10 border-blue-500/20 shadow-blue-500/10",
        "purple" => "text-purple-400 bg-purple-500/10 border-purple-500/20 shadow-purple-500/10",
        "green" => "text-green-400 bg-green-500/10 border-green-500/20 shadow-green-500/10",
        _ => "text-gray-400 bg-gray-500/10 border-gray-500/20 shadow-gray-500/10",
    };

    view! {
        <A
            href=href
            class=format!("group flex items-center p-5 glass-premium rounded-3xl border transition-all duration-500 hover:scale-[1.03] hover:shadow-2xl {}", color_class)
        >
            <div class=format!("p-4 rounded-2xl mr-5 shadow-inner transition-transform group-hover:rotate-12 {}", color_class)>
                <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.5">
                    {icon_svg}
                </svg>
            </div>
            <div class="flex-1">
                <h3 class="text-lg font-black text-theme-primary leading-tight tracking-tight">{title}</h3>
                <p class="text-xs font-bold text-theme-secondary opacity-60 mt-1 uppercase tracking-wider">{description}</p>
            </div>
            <div class="opacity-0 group-hover:opacity-100 transition-all transform translate-x-4 group-hover:translate-x-0">
                <svg class="w-6 h-6 text-theme-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M9 5l7 7-7 7" />
                </svg>
            </div>
        </A>
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ExecutionStats {
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    series: Vec<f64>,
}

/// Executions chart component
#[component]
fn ExecutionsChart(stats: ReadSignal<Option<ExecutionStats>>) -> impl IntoView {
    view! {
        <div class="relative w-full">
            <div class="absolute inset-0 bg-gradient-to-b from-blue-500/5 to-transparent rounded-3xl pointer-events-none"></div>
            <Show
                when=move || stats.get().is_some()
                fallback=|| view! {
                    <div class="flex items-center justify-center h-[300px] text-theme-muted opacity-40">
                         <div class="w-8 h-8 border-4 border-current border-t-transparent rounded-full animate-spin mr-3"></div>
                         <span class="text-xs font-black uppercase tracking-widest">"Loading Analytics..."</span>
                    </div>
                }
            >
                {move || {
                    let s = stats.get().unwrap();
                    let data = ChartData {
                        title: String::new(),
                        labels: s.labels,
                        series: vec![DataSeries {
                            name: "Requests".to_string(),
                            values: s.series,
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
                }}
            </Show>
        </div>
    }
}

/// System status component
#[component]
fn SystemStatus(services: ReadSignal<Vec<ServiceStatus>>) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Show
                when=move || !services.get().is_empty()
                fallback=|| view! {
                    <div class="col-span-2 flex flex-col items-center justify-center py-16 text-theme-muted opacity-40">
                        <div class="w-12 h-12 border-4 border-current border-t-transparent rounded-full animate-spin mb-4"></div>
                        <p class="font-black tracking-[0.2em] uppercase text-[10px]">"Mapping Infrastructure..."</p>
                    </div>
                }
            >
                <For
                    each=move || services.get()
                    key=|svc| svc.name.clone()
                    let:service
                >
                    {
                        let (status_color, status_text) = match service.status.as_str() {
                            "healthy" | "ok" => ("text-green-400 bg-green-500/5 border-green-500/20", "Active"),
                            "degraded" | "warning" => ("text-yellow-400 bg-yellow-500/5 border-yellow-500/20", "Warning"),
                            _ => ("text-red-400 bg-red-500/5 border-red-500/20", "Down"),
                        };
                        let uptime = service.uptime.clone().unwrap_or_else(|| "calculating...".to_string());

                        view! {
                            <div class=format!("p-5 glass rounded-3xl border transition-all hover:bg-white/5 group {}", status_color)>
                                <div class="flex items-center justify-between mb-4">
                                    <div class="flex items-center space-x-3">
                                        <div class="relative">
                                            <div class="w-3 h-3 rounded-full bg-current shadow-[0_0_10px_rgba(0,0,0,0.5)]"></div>
                                            <div class="absolute inset-0 w-3 h-3 rounded-full bg-current animate-ping opacity-25"></div>
                                        </div>
                                        <span class="font-black text-theme-primary tracking-tighter group-hover:scale-105 transition-transform">{service.name.clone()}</span>
                                    </div>
                                    <span class="text-[9px] font-black uppercase tracking-[0.15em] opacity-60">{status_text}</span>
                                </div>
                                <div class="flex items-center justify-between text-[10px] font-black opacity-40 group-hover:opacity-70 transition-opacity">
                                    <span class="uppercase tracking-widest">"Uptime"</span>
                                    <span class="font-mono">{uptime}</span>
                                </div>
                            </div>
                        }
                    }
                </For>
            </Show>
        </div>
    }
}