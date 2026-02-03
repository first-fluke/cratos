//! Dashboard Page

use leptos::*;

use crate::components::{Card, Chart, ChartData, ChartType, DataSeries, StatCard};

/// Main dashboard page
#[component]
pub fn Dashboard() -> impl IntoView {
    // Sample data - would be fetched from API in production
    let (executions_today, _) = create_signal(42);
    let (avg_response_time, _) = create_signal("1.2s".to_string());
    let (active_sessions, _) = create_signal(3);
    let (skills_count, _) = create_signal(15);

    view! {
        <div class="space-y-8">
            // Header
            <div class="flex items-center justify-between">
                <h1 class="text-3xl font-bold">"Dashboard"</h1>
                <div class="text-sm text-gray-400">
                    "Last updated: just now"
                </div>
            </div>

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
                    title="Skills"
                    value=move || skills_count.get().to_string()
                    icon="star"
                    color="yellow"
                />
            </div>

            // Main content grid
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                // Recent activity
                <Card title="Recent Activity">
                    <RecentActivity />
                </Card>

                // Quick actions
                <Card title="Quick Actions">
                    <QuickActions />
                </Card>
            </div>

            // System status
            <Card title="System Status">
                <SystemStatus />
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

/// Recent activity list
#[component]
fn RecentActivity() -> impl IntoView {
    let activities = vec![
        ("Search for Rust async patterns", "2 min ago", "completed"),
        ("Generate unit tests for api.rs", "15 min ago", "completed"),
        ("Refactor database module", "1 hour ago", "failed"),
        ("Create PR for feature branch", "2 hours ago", "completed"),
    ];

    view! {
        <ul class="divide-y divide-gray-700">
            {activities.into_iter().map(|(action, time, status)| {
                let status_color = match status {
                    "completed" => "text-green-400",
                    "failed" => "text-red-400",
                    _ => "text-yellow-400",
                };

                view! {
                    <li class="py-3 flex items-center justify-between">
                        <div>
                            <p class="text-sm font-medium">{action}</p>
                            <p class="text-xs text-gray-500">{time}</p>
                        </div>
                        <span class={format!("text-xs font-medium {}", status_color)}>
                            {status}
                        </span>
                    </li>
                }
            }).collect_view()}
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
                icon="chat"
            />
            <ActionButton
                title="View History"
                description="Browse past executions"
                href="/history"
                icon="history"
            />
            <ActionButton
                title="Manage Skills"
                description="Configure AI skills"
                href="/settings#skills"
                icon="star"
            />
            <ActionButton
                title="API Docs"
                description="View API documentation"
                href="/docs"
                icon="book"
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
) -> impl IntoView {
    let _ = icon; // Would be used for icon rendering

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

/// Executions chart component
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
            values: vec![25.0, 42.0, 38.0, 55.0, 48.0, 32.0, 45.0],
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

/// Model usage chart component
#[component]
fn ModelUsageChart() -> impl IntoView {
    let data = ChartData {
        title: String::new(),
        labels: vec![
            "Groq".to_string(),
            "DeepSeek".to_string(),
            "Claude".to_string(),
            "GPT-4".to_string(),
        ],
        series: vec![DataSeries {
            name: "Requests".to_string(),
            values: vec![45.0, 30.0, 15.0, 10.0],
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

/// System status component
#[component]
fn SystemStatus() -> impl IntoView {
    let services = vec![
        ("API Server", "healthy", "100%"),
        ("Redis", "healthy", "99.9%"),
        ("Telegram Bot", "healthy", "100%"),
        ("LLM Provider", "degraded", "95%"),
    ];

    view! {
        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            {services.into_iter().map(|(name, status, uptime)| {
                let status_color = match status {
                    "healthy" => "bg-green-500",
                    "degraded" => "bg-yellow-500",
                    _ => "bg-red-500",
                };

                view! {
                    <div class="p-4 bg-gray-800 rounded-lg">
                        <div class="flex items-center space-x-2">
                            <div class={format!("w-2 h-2 rounded-full {}", status_color)} />
                            <span class="font-medium text-sm">{name}</span>
                        </div>
                        <p class="text-xs text-gray-400 mt-2">"Uptime: " {uptime}</p>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}
