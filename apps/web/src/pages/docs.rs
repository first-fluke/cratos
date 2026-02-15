//! Documentation Page

use leptos::*;
use leptos_router::A;

#[component]
pub fn Docs() -> impl IntoView {
    view! {
        <div class="max-w-4xl mx-auto space-y-8 animate-in fade-in">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-3xl font-bold text-theme-primary">"Documentation"</h1>
                    <p class="text-theme-secondary mt-2">"Learn how to integrate and use the Cratos AI ecosystem."</p>
                </div>
                <A href="/" class="text-sm font-medium text-theme-info hover:underline">
                    "Back to Dashboard"
                </A>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                <a href="https://github.com/cratos/cratos" target="_blank" rel="noopener noreferrer">
                    <DocCard 
                        title="Getting Started" 
                        description="Quick start guide to get your first AI agent running."
                        icon="rocket"
                    />
                </a>
                <a href="/docs/" target="_blank">
                    <DocCard 
                        title="API Reference" 
                        description="Interactive Swagger UI for our REST and WebSocket APIs."
                        icon="code"
                    />
                </a>
                <A href="/personas">
                    <DocCard 
                        title="Persona Forge" 
                        description="How to design and specialize your own AI personalities."
                        icon="user-plus"
                    />
                </A>
            </div>

            <div class="bg-theme-card border border-theme-border-default rounded-2xl p-8 space-y-6">
                <section class="space-y-4">
                    <h2 class="text-xl font-bold text-theme-primary">"Core Concepts"</h2>
                    <div class="prose prose-invert max-w-none text-theme-secondary">
                        <p>
                            "Cratos is built on a distributed intelligence architecture. Every agent (Persona) 
                            operates within a specialized domain while sharing a global memory system."
                        </p>
                        <ul class="list-disc pl-5 space-y-2">
                            <li><strong>"Personas:"</strong> "Specialized AI instances with unique traits and skills."</li>
                            <li><strong>"Executions:"</strong> "Individual tasks or conversations processed by the system."</li>
                            <li><strong>"Memory:"</strong> "Vector-stored context that persists across sessions."</li>
                        </ul>
                    </div>
                </section>

                <div class="pt-6 border-t border-theme-border-default">
                    <h2 class="text-xl font-bold text-theme-primary mb-4">"Integration Hooks"</h2>
                    <div class="bg-theme-base rounded-lg p-4 font-mono text-sm text-theme-secondary overflow-x-auto">
                        <pre>
                            "// Example API Request\n"
                            "curl -X POST https://api.cratos.ai/v1/chat \\\n"
                            "  -H \"Content-Type: application/json\" \\\n"
                            "  -d '{\"message\": \"Analyze system logs\", \"persona\": \"Orion\"}'"
                        </pre>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn DocCard(title: &'static str, description: &'static str, icon: &'static str) -> impl IntoView {
    let icon_svg = match icon {
        "rocket" => view! { <path d="M15.5 8.5L19 5m-14 14l3.5-3.5M12 12l-4 4m8-8l-4-4m4 4H7.5c-1.9 0-3.5 1.6-3.5 3.5V17m11.5-8.5c1.9 0 3.5 1.6 3.5 3.5V17" /> }.into_view(),
        "code" => view! { <path d="M8 9l-3 3 3 3m8-6l3 3-3 3m-7-9l4 12" /> }.into_view(),
        "user-plus" => view! { <path d="M16 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2m8-10a4 4 0 100-8 4 4 0 000 8zm11 5h-6m3-3v6" /> }.into_view(),
        _ => view! { <path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /> }.into_view(),
    };

    view! {
        <div class="bg-theme-card border border-theme-border-default rounded-xl p-6 hover:border-theme-info transition-colors group cursor-pointer">
            <div class="w-10 h-10 rounded-lg bg-theme-info/10 text-theme-info flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
                <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                    {icon_svg}
                </svg>
            </div>
            <h3 class="text-lg font-bold text-theme-primary mb-2">{title}</h3>
            <p class="text-sm text-theme-secondary leading-relaxed">{description}</p>
        </div>
    }
}
