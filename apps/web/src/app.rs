//! Main Application Component

use gloo_storage::Storage;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::pages::{Chat, Dashboard, History, Memory, Personas, Settings, Tools};

/// Theme provider context
#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub is_dark: ReadSignal<bool>,
    pub set_dark: WriteSignal<bool>,
}

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    // Initialize theme from localStorage
    let stored_theme = gloo_storage::LocalStorage::get::<String>("theme").unwrap_or_else(|_| "dark".to_string());
    let initial_dark = stored_theme != "light";

    let (is_dark, set_dark) = create_signal(initial_dark);

    // Provide theme context
    provide_context(ThemeContext { is_dark, set_dark });

    // Apply theme class to html element
    create_effect(move |_| {
        let dark = is_dark.get();
        gloo_console::log!("Theme changed: dark =", dark);
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(html) = document.document_element() {
                    let class_list = html.class_list();
                    if dark {
                        let _ = class_list.add_1("dark");
                        gloo_console::log!("Added 'dark' class to html");
                    } else {
                        let _ = class_list.remove_1("dark");
                        gloo_console::log!("Removed 'dark' class from html");
                    }
                    gloo_console::log!("Current classes:", class_list.value());
                }
            }
        }
    });

    view! {
        <Title text="Cratos Dashboard" />
        <Meta name="description" content="Cratos AI Assistant Dashboard" />
        <Meta name="viewport" content="width=device-width, initial-scale=1" />

        <Router>
            <main class="h-screen flex flex-col bg-theme-base text-theme-primary overflow-hidden">
                <NavBar />
                <div class="flex-1 container mx-auto px-4 py-8 overflow-y-auto flex flex-col">
                    <Routes>
                        <Route path="/" view=Dashboard />
                        <Route path="/dashboard" view=Dashboard />
                        <Route path="/chat" view=Chat />
                        <Route path="/chat/:session_id" view=Chat />
                        <Route path="/history" view=History />
                        <Route path="/personas" view=Personas />
                        <Route path="/memory" view=Memory />
                        <Route path="/tools" view=Tools />
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
        <nav class="bg-theme-card border-b border-theme-default shadow-sm">
            <div class="container mx-auto px-4">
                <div class="flex items-center justify-between h-16">
                    <div class="flex items-center space-x-8">
                        <A href="/" class="text-xl font-bold text-theme-primary">
                            "Cratos"
                        </A>
                        <div class="flex space-x-4">
                            <NavLink href="/" text="Dashboard" />
                            <NavLink href="/chat" text="Chat" />
                            <NavLink href="/history" text="History" />
                            <NavLink href="/personas" text="Personas" />
                            <NavLink href="/memory" text="Memory" />
                            <NavLink href="/tools" text="Tools" />
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
            class="px-3 py-2 rounded-md text-sm font-medium text-theme-secondary hover:text-theme-primary hover:bg-theme-elevated transition-colors"
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
                class:bg-theme-success=connected
                class:bg-theme-error=move || !connected.get()
            />
            <span class="text-sm text-theme-muted">
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
            <h1 class="text-6xl font-bold text-theme-muted">"404"</h1>
            <p class="text-xl text-theme-secondary mt-4">"Page not found"</p>
            <A href="/" class="inline-block mt-8 px-6 py-3 bg-theme-info text-white rounded-lg hover:opacity-90">
                "Go Home"
            </A>
        </div>
    }
}
