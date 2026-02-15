//! Card Components

use leptos::*;
use leptos_router::A;

/// Generic card component with glassmorphism support
#[component]
pub fn Card(
    title: &'static str, 
    #[prop(optional)] class: &'static str,
    children: Children
) -> impl IntoView {
    view! {
        <div class=format!("glass-premium rounded-2xl p-6 border border-white/10 shadow-xl {}", class)>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-xl font-bold tracking-tight text-theme-primary">{title}</h2>
                <div class="w-8 h-1 bg-gradient-premium rounded-full opacity-50"></div>
            </div>
            <div class="relative z-10">
                {children()}
            </div>
        </div>
    }
}

/// Stat card for displaying metrics with glassmorphism and glowing effects
#[component]
pub fn StatCard<F>(
    title: &'static str,
    value: F,
    icon: &'static str,
    color: &'static str,
    #[prop(optional)] href: Option<&'static str>,
    #[prop(optional, into)] trend: Option<String>,
) -> impl IntoView
where
    F: Fn() -> String + 'static,
{
    let (bg_gradient, icon_bg, text_color, glow_class) = match color {
        "blue" | "info" => (
            "from-blue-500/10 to-transparent", 
            "bg-blue-500/20 text-blue-400", 
            "text-blue-400",
            "group-hover:shadow-blue-500/20"
        ),
        "green" | "success" => (
            "from-green-500/10 to-transparent", 
            "bg-green-500/20 text-green-400", 
            "text-green-400",
            "group-hover:shadow-green-500/20"
        ),
        "purple" => (
            "from-purple-500/10 to-transparent", 
            "bg-purple-500/20 text-purple-400", 
            "text-purple-400",
            "group-hover:shadow-purple-500/20"
        ),
        "yellow" | "warning" => (
            "from-yellow-500/10 to-transparent", 
            "bg-yellow-500/20 text-yellow-400", 
            "text-yellow-400",
            "group-hover:shadow-yellow-500/20"
        ),
        "red" | "error" => (
            "from-red-500/10 to-transparent", 
            "bg-red-500/20 text-red-400", 
            "text-red-400",
            "group-hover:shadow-red-500/20"
        ),
        _ => (
            "from-gray-500/10 to-transparent", 
            "bg-gray-500/20 text-gray-400", 
            "text-theme-primary",
            "group-hover:shadow-gray-500/20"
        ),
    };

    let icon_svg = match icon {
        "chart" => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
            </svg>
        }.into_view(),
        "clock" | "cpu" => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
        }.into_view(),
        "users" => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
            </svg>
        }.into_view(),
        "star" | "database" => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.382-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
            </svg>
        }.into_view(),
        "chat" => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
            </svg>
        }.into_view(),
        _ => view! {
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
        }.into_view(),
    };

    let base_class = format!(
        "group glass-premium rounded-2xl p-6 transition-all duration-500 hover:scale-[1.02] hover:shadow-2xl overflow-hidden {}",
        glow_class
    );

    let content = view! {
        <div class=format!("absolute inset-0 bg-gradient-to-br {} opacity-50 group-hover:opacity-100 transition-opacity", bg_gradient)></div>
        <div class="relative z-10 flex flex-col h-full">
            <div class="flex items-center justify-between mb-4">
                <div class=format!("p-3 rounded-xl {} shadow-lg", icon_bg)>
                    {icon_svg}
                </div>
                {move || trend.clone().map(|t| view! {
                    <div class="px-2.5 py-1 rounded-full bg-white/5 border border-white/10 text-[10px] font-black tracking-wider uppercase text-white/70">
                        {t}
                    </div>
                })}
            </div>
            <p class="text-sm text-theme-secondary font-medium tracking-wide uppercase">{title}</p>
            <p class={format!("text-4xl font-black mt-2 tracking-tight {}", text_color)}>
                {value}
            </p>
        </div>
    };

    match href {
        Some(link) => view! {
            <A href=link class=base_class>
                {content}
            </A>
        }.into_view(),
        None => view! {
            <div class=base_class>
                {content}
            </div>
        }.into_view(),
    }
}
