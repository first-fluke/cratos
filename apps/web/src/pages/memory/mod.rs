pub mod components;

use leptos::*;
use serde::Deserialize;

use crate::api::ApiClient;
use components::*;

/// Generic API response wrapper
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T: Default> {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: T,
}

/// Memory page showing knowledge graph
#[component]
pub fn Memory() -> impl IntoView {
    let (stats, set_stats) = create_signal(GraphStats::default());
    let (graph_data, set_graph_data) = create_signal(GraphData::default());
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    // Fetch data on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();

            // Fetch graph stats
            match client.get::<ApiResponse<GraphStats>>("/api/v1/graph/stats").await {
                Ok(resp) => {
                    set_stats.set(resp.data);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch graph stats:", e.clone());
                    set_error.set(Some(e));
                }
            }

            // Fetch graph data
            match client.get::<ApiResponse<GraphData>>("/api/v1/graph?limit=100").await {
                Ok(resp) => {
                    set_graph_data.set(resp.data);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch graph data:", e.clone());
                    set_error.set(Some(e));
                }
            }

            set_loading.set(false);
        });
    });

    view! {
        <div class="h-full flex flex-col space-y-4 animate-in fade-in duration-700">
            // Header with Glassmorphism
            <GraphHeader loading=loading.into() />

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-theme-error/10 border border-theme-error/30 rounded-lg p-4 text-theme-error shrink-0">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Graph visualization Container
            <div class="flex-1 relative rounded-2xl overflow-hidden border border-theme-border-default shadow-2xl bg-[#0a0a0f] dark:bg-[#050508]">
                 <Show
                    when=move || !loading.get() && !graph_data.get().nodes.is_empty()
                    fallback=move || view! {
                        <div class="absolute inset-0 flex items-center justify-center text-theme-muted bg-theme-base/10 backdrop-blur-sm z-50">
                            <Show
                                when=move || loading.get()
                                fallback=move || view! {
                                    <div class="text-center">
                                        <p class="text-2xl font-light">"Empty Void"</p>
                                        <p class="text-sm mt-2 opacity-70">"No memories formed yet."</p>
                                    </div>
                                }
                            >
                                <div class="flex flex-col items-center">
                                    <div class="animate-spin rounded-full h-16 w-16 border-t-2 border-b-2 border-theme-info mb-6 shadow-[0_0_15px_var(--color-info)]"></div>
                                    <div class="text-theme-info font-mono text-sm tracking-widest">"INITIALIZING NEURAL LINK"</div>
                                </div>
                            </Show>
                        </div>
                    }
                >
                    <ForceGraph
                        nodes=Signal::derive(move || graph_data.get().nodes.clone())
                        edges=Signal::derive(move || {
                            let mut edges = graph_data.get().edges.clone();
                            // Filter out "cooccurrence" labels to reduce visual spam
                            for edge in &mut edges {
                                if edge.kind == "cooccurrence" {
                                    edge.kind = String::new();
                                }
                            }
                            edges
                        })
                    />
                </Show>

                // Floating Stats Overlay (Top Right)
                <GraphStatsOverlay 
                    stats=stats.into() 
                    edge_count=Signal::derive(move || graph_data.get().edges.len()) 
                />
            </div>
        </div>
    }
}
