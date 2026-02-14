//! Diagram Component
//!
//! Renders diagrams using Kroki.io external service.
//! Supports multiple diagram types: mermaid, plantuml, graphviz, d2, etc.

use leptos::*;
use std::io::Write;

// Kroki.io base URL
const KROKI_BASE_URL: &str = "https://kroki.io";

/// Supported diagram types
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DiagramType {
    #[default]
    Mermaid,
    PlantUml,
    Graphviz,
    D2,
    Ditaa,
    Nomnoml,
    Erd,
    Svgbob,
    Vega,
    VegaLite,
}

impl DiagramType {
    /// Get the Kroki API path for this diagram type
    pub fn api_path(&self) -> &'static str {
        match self {
            DiagramType::Mermaid => "mermaid",
            DiagramType::PlantUml => "plantuml",
            DiagramType::Graphviz => "graphviz",
            DiagramType::D2 => "d2",
            DiagramType::Ditaa => "ditaa",
            DiagramType::Nomnoml => "nomnoml",
            DiagramType::Erd => "erd",
            DiagramType::Svgbob => "svgbob",
            DiagramType::Vega => "vega",
            DiagramType::VegaLite => "vegalite",
        }
    }

    /// Parse diagram type from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mermaid" => DiagramType::Mermaid,
            "plantuml" | "puml" => DiagramType::PlantUml,
            "graphviz" | "dot" => DiagramType::Graphviz,
            "d2" => DiagramType::D2,
            "ditaa" => DiagramType::Ditaa,
            "nomnoml" => DiagramType::Nomnoml,
            "erd" => DiagramType::Erd,
            "svgbob" => DiagramType::Svgbob,
            "vega" => DiagramType::Vega,
            "vegalite" | "vega-lite" => DiagramType::VegaLite,
            _ => DiagramType::Mermaid,
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            DiagramType::Mermaid => "Mermaid",
            DiagramType::PlantUml => "PlantUML",
            DiagramType::Graphviz => "Graphviz",
            DiagramType::D2 => "D2",
            DiagramType::Ditaa => "Ditaa",
            DiagramType::Nomnoml => "Nomnoml",
            DiagramType::Erd => "ERD",
            DiagramType::Svgbob => "Svgbob",
            DiagramType::Vega => "Vega",
            DiagramType::VegaLite => "Vega-Lite",
        }
    }
}

/// Diagram component that renders via Kroki.io
#[component]
pub fn Diagram(
    /// The diagram source code
    #[prop(into)]
    source: String,
    /// The diagram type (e.g., "mermaid", "plantuml")
    #[prop(into)]
    diagram_type: String,
    /// Optional title for the diagram
    #[prop(optional, into)]
    title: Option<String>,
    /// Whether to show a loading state
    #[prop(optional, default = true)]
    show_loading: bool,
) -> impl IntoView {
    let diagram_type_parsed = DiagramType::from_str(&diagram_type);
    let source_for_url = source.clone();
    let source_for_copy = source.clone();

    // Generate Kroki URL
    let kroki_url = create_kroki_url(diagram_type_parsed, &source_for_url);

    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    let display_title = title.unwrap_or_else(|| format!("{} Diagram", diagram_type_parsed.display_name()));
    let kroki_url_for_link = kroki_url.clone();

    view! {
        <div class="diagram-container bg-gray-900 rounded-lg overflow-hidden">
            // Header with diagram type
            <div class="flex items-center justify-between px-4 py-2 bg-gray-800 border-b border-gray-700">
                <div class="flex items-center space-x-2">
                    <DiagramIcon diagram_type=diagram_type_parsed />
                    <span class="text-sm font-medium text-gray-300">
                        {display_title}
                    </span>
                </div>
                <div class="flex items-center space-x-2">
                    // Copy source button
                    <CopySourceButton source=source_for_copy />
                    // Open in new tab button
                    <a
                        href=kroki_url_for_link
                        target="_blank"
                        rel="noopener noreferrer"
                        class="p-1 text-gray-400 hover:text-gray-200 rounded transition-colors"
                        title="Open in new tab"
                    >
                        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6"/>
                            <polyline points="15 3 21 3 21 9"/>
                            <line x1="10" y1="14" x2="21" y2="3"/>
                        </svg>
                    </a>
                </div>
            </div>

            // Diagram content
            <div class="p-4 flex items-center justify-center min-h-[200px]">
                // Loading state
                <Show when=move || show_loading && loading.get()>
                    <div class="flex items-center space-x-2 text-gray-400">
                        <div class="w-5 h-5 border-2 border-gray-600 border-t-blue-500 rounded-full animate-spin" />
                        <span class="text-sm">"Loading diagram..."</span>
                    </div>
                </Show>

                // Error state
                <Show when=move || error.get().is_some()>
                    <div class="text-center text-red-400">
                        <p class="font-medium">"Failed to load diagram"</p>
                        <p class="text-sm mt-1">{move || error.get().unwrap_or_default()}</p>
                    </div>
                </Show>

                // Diagram image
                <img
                    src=kroki_url
                    alt="Diagram"
                    class="max-w-full"
                    class:hidden=move || loading.get()
                    loading="lazy"
                    on:load=move |_| set_loading.set(false)
                    on:error=move |_| {
                        set_loading.set(false);
                        set_error.set(Some("Failed to render diagram".to_string()));
                    }
                />
            </div>
        </div>
    }
}

/// Diagram icon based on type
#[component]
fn DiagramIcon(diagram_type: DiagramType) -> impl IntoView {
    let icon_class = "w-4 h-4 text-gray-400";

    // Simple SVG icons for different diagram types
    match diagram_type {
        DiagramType::Mermaid => view! {
            <svg class=icon_class viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
            </svg>
        }.into_view(),
        DiagramType::Graphviz => view! {
            <svg class=icon_class viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3"/>
                <circle cx="4" cy="4" r="2"/>
                <circle cx="20" cy="4" r="2"/>
                <circle cx="4" cy="20" r="2"/>
                <circle cx="20" cy="20" r="2"/>
                <line x1="12" y1="9" x2="4" y2="6"/>
                <line x1="12" y1="9" x2="20" y2="6"/>
                <line x1="12" y1="15" x2="4" y2="18"/>
                <line x1="12" y1="15" x2="20" y2="18"/>
            </svg>
        }.into_view(),
        _ => view! {
            <svg class=icon_class viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="3" width="18" height="18" rx="2"/>
                <path d="M9 9h6M9 12h6M9 15h4"/>
            </svg>
        }.into_view(),
    }
}

/// Copy source button
#[component]
fn CopySourceButton(#[prop(into)] source: String) -> impl IntoView {
    let (copied, set_copied) = create_signal(false);
    let source_clone = source.clone();

    let on_copy = move |_| {
        let text = source_clone.clone();
        spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(
                    clipboard.write_text(&text)
                ).await;
                set_copied.set(true);
                gloo_timers::future::TimeoutFuture::new(2000).await;
                set_copied.set(false);
            }
        });
    };

    view! {
        <button
            class="p-1 text-gray-400 hover:text-gray-200 rounded transition-colors"
            title="Copy source"
            on:click=on_copy
        >
            <Show
                when=move || copied.get()
                fallback=move || view! {
                    <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="9" y="9" width="13" height="13" rx="2"/>
                        <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
                    </svg>
                }
            >
                <svg class="w-4 h-4 text-green-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="20 6 9 17 4 12"/>
                </svg>
            </Show>
        </button>
    }
}

/// Create a Kroki URL for diagram rendering
fn create_kroki_url(diagram_type: DiagramType, source: &str) -> String {
    use base64::Engine;

    // Compress source with zlib
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = encoder.write_all(source.as_bytes());
    let compressed = encoder.finish().unwrap_or_default();

    // Encode with URL-safe base64
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed);

    format!(
        "{}/{}/svg/{}",
        KROKI_BASE_URL,
        diagram_type.api_path(),
        encoded
    )
}

/// Editable diagram component with source editor
#[component]
pub fn EditableDiagram(
    /// Initial diagram source
    #[prop(into)]
    initial_source: String,
    /// Initial diagram type
    #[prop(into)]
    initial_type: String,
) -> impl IntoView {
    let (source, set_source) = create_signal(initial_source);
    let (diagram_type, set_diagram_type) = create_signal(initial_type);
    let (show_source, set_show_source) = create_signal(false);

    // Compute current values for the diagram
    let current_source = move || source.get();
    let current_type = move || diagram_type.get();

    view! {
        <div class="editable-diagram space-y-4">
            // Toolbar
            <div class="flex items-center justify-between">
                <div class="flex items-center space-x-2">
                    <select
                        class="px-3 py-1 bg-gray-800 border border-gray-700 rounded text-sm focus:outline-none focus:border-blue-500"
                        on:change=move |ev| {
                            let value = event_target_value(&ev);
                            set_diagram_type.set(value);
                        }
                    >
                        <option value="mermaid" selected=move || diagram_type.get() == "mermaid">"Mermaid"</option>
                        <option value="plantuml" selected=move || diagram_type.get() == "plantuml">"PlantUML"</option>
                        <option value="graphviz" selected=move || diagram_type.get() == "graphviz">"Graphviz"</option>
                        <option value="d2" selected=move || diagram_type.get() == "d2">"D2"</option>
                    </select>
                </div>
                <button
                    class="px-3 py-1 text-sm bg-gray-700 rounded hover:bg-gray-600 transition-colors"
                    on:click=move |_| set_show_source.update(|v| *v = !*v)
                >
                    {move || if show_source.get() { "Hide Source" } else { "Show Source" }}
                </button>
            </div>

            // Source editor (collapsible)
            <Show when=move || show_source.get()>
                <textarea
                    class="w-full h-48 px-4 py-3 bg-gray-900 border border-gray-700 rounded-lg font-mono text-sm focus:outline-none focus:border-blue-500 resize-y"
                    prop:value=move || source.get()
                    on:input=move |ev| {
                        let value = event_target_value(&ev);
                        set_source.set(value);
                    }
                    placeholder="Enter diagram source..."
                />
            </Show>

            // Diagram preview - use computed values directly
            {move || {
                let src = current_source();
                let dtype = current_type();
                view! {
                    <Diagram
                        source=src
                        diagram_type=dtype
                    />
                }
            }}
        </div>
    }
}
