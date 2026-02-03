//! Card Components

use leptos::*;

/// Generic card component
#[component]
pub fn Card(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="bg-gray-800 rounded-lg p-6">
            <h2 class="text-lg font-semibold mb-4">{title}</h2>
            <div>
                {children()}
            </div>
        </div>
    }
}

/// Stat card for displaying metrics
#[component]
pub fn StatCard<F>(
    title: &'static str,
    value: F,
    icon: &'static str,
    color: &'static str,
) -> impl IntoView
where
    F: Fn() -> String + 'static,
{
    let _ = icon; // Would be used for icon rendering

    let bg_color = match color {
        "blue" => "bg-blue-900/50",
        "green" => "bg-green-900/50",
        "purple" => "bg-purple-900/50",
        "yellow" => "bg-yellow-900/50",
        "red" => "bg-red-900/50",
        _ => "bg-gray-800",
    };

    let text_color = match color {
        "blue" => "text-blue-400",
        "green" => "text-green-400",
        "purple" => "text-purple-400",
        "yellow" => "text-yellow-400",
        "red" => "text-red-400",
        _ => "text-gray-400",
    };

    view! {
        <div class={format!("rounded-lg p-6 {}", bg_color)}>
            <p class="text-sm text-gray-400 font-medium">{title}</p>
            <p class={format!("text-3xl font-bold mt-2 {}", text_color)}>
                {value}
            </p>
        </div>
    }
}
