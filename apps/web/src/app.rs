//! Main Application Component

use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::pages::{Chat, Dashboard, History, Settings};

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="Cratos Dashboard" />
        <Meta name="description" content="Cratos AI Assistant Dashboard" />
        <Meta name="viewport" content="width=device-width, initial-scale=1" />

        <Router>
            <main class="min-h-screen bg-gray-900 text-gray-100">
                <NavBar />
                <div class="container mx-auto px-4 py-8">
                    <Routes>
                        <Route path="/" view=Dashboard />
                        <Route path="/chat" view=Chat />
                        <Route path="/chat/:session_id" view=Chat />
                        <Route path="/history" view=History />
                        <Route path="/settings" view=Settings />
                        <Route path="/*any" view=NotFound />
                    </Routes>
                </div>
            </main>
        </Router>
    }
}

/// Navigation bar component
#[component]
fn NavBar() -> impl IntoView {
    view! {
        <nav class="bg-gray-800 border-b border-gray-700">
            <div class="container mx-auto px-4">
                <div class="flex items-center justify-between h-16">
                    <div class="flex items-center space-x-8">
                        <A href="/" class="text-xl font-bold text-blue-400">
                            "Cratos"
                        </A>
                        <div class="flex space-x-4">
                            <NavLink href="/" text="Dashboard" />
                            <NavLink href="/chat" text="Chat" />
                            <NavLink href="/history" text="History" />
                            <NavLink href="/settings" text="Settings" />
                        </div>
                    </div>
                    <div class="flex items-center space-x-4">
                        <StatusIndicator />
                    </div>
                </div>
            </div>
        </nav>
    }
}

/// Navigation link component
#[component]
fn NavLink(href: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <A
            href=href
            class="px-3 py-2 rounded-md text-sm font-medium text-gray-300 hover:text-white hover:bg-gray-700 transition-colors"
        >
            {text}
        </A>
    }
}

/// Status indicator showing connection state
#[component]
fn StatusIndicator() -> impl IntoView {
    let (connected, _set_connected) = create_signal(true);

    view! {
        <div class="flex items-center space-x-2">
            <div
                class="w-2 h-2 rounded-full"
                class:bg-green-500=connected
                class:bg-red-500=move || !connected.get()
            />
            <span class="text-sm text-gray-400">
                {move || if connected.get() { "Connected" } else { "Disconnected" }}
            </span>
        </div>
    }
}

/// 404 Not Found page
#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="text-center py-20">
            <h1 class="text-6xl font-bold text-gray-600">"404"</h1>
            <p class="text-xl text-gray-400 mt-4">"Page not found"</p>
            <A href="/" class="inline-block mt-8 px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700">
                "Go Home"
            </A>
        </div>
    }
}
