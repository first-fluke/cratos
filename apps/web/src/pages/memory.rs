//! Memory Page
//!
//! Visualizes the Graph RAG knowledge graph with premium interactivity.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use leptos::*;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::MouseEvent;

use crate::api::ApiClient;
// use crate::components::StatCard;

/// Generic API response wrapper
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse<T: Default> {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: T,
}

/// Graph node from API
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct GraphNode {
    id: String,
    label: String,
    kind: String,
    #[serde(default, alias = "mentionCount")]
    mention_count: u32,
}

/// Graph edge from API
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct GraphEdge {
    source: String,
    target: String,
    #[serde(default)]
    weight: u32,
}

/// Complete graph data
#[derive(Debug, Clone, Deserialize, Default)]
struct GraphData {
    #[serde(default)]
    nodes: Vec<GraphNode>,
    #[serde(default)]
    edges: Vec<GraphEdge>,
}

/// Graph statistics
#[derive(Debug, Clone, Deserialize, Default)]
struct GraphStats {
    #[serde(default, alias = "turnCount")]
    #[allow(dead_code)]
    turn_count: u32,
    #[serde(default, alias = "entityCount")]
    entity_count: u32,
}

/// Simulation node state
#[derive(Clone, Debug)]
struct NodeState {
    #[allow(dead_code)]
    id: String, // Keeping id for reference if needed, though HashMap key is id
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    is_dragged: bool,
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
            <div class="flex items-center justify-between shrink-0 bg-theme-card/30 backdrop-blur-md p-4 rounded-xl border border-theme-border-default/50">
                <div>
                    <h1 class="text-3xl font-extrabold text-transparent bg-clip-text bg-gradient-to-r from-theme-info to-purple-400">
                        "Neural Memory Graph"
                    </h1>
                    <p class="text-theme-secondary text-sm flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full bg-theme-success animate-pulse"></span>
                        "Live Connection via Serena Bridge"
                    </p>
                </div>
                <div class="text-xs font-mono text-theme-muted bg-theme-base/50 px-2 py-1 rounded border border-theme-border-default">
                    {move || if loading.get() { "SYNCING..." } else { "ONLINE" }}
                </div>
            </div>

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
                        edges=Signal::derive(move || graph_data.get().edges.clone())
                    />
                </Show>

                // Floating Stats Overlay (Top Right)
                 <div class="absolute top-4 right-4 flex flex-col gap-2 pointer-events-none">
                    <div class="bg-black/40 backdrop-blur-md border border-white/10 p-3 rounded-lg text-right pointer-events-auto hover:bg-black/60 transition-colors">
                        <div class="text-xs text-theme-muted uppercase tracking-wider">"Entities"</div>
                        <div class="text-xl font-bold text-white">{move || stats.get().entity_count}</div>
                    </div>
                    <div class="bg-black/40 backdrop-blur-md border border-white/10 p-3 rounded-lg text-right pointer-events-auto hover:bg-black/60 transition-colors">
                         <div class="text-xs text-theme-muted uppercase tracking-wider">"Connections"</div>
                        <div class="text-xl font-bold text-theme-info">{move || graph_data.get().edges.len()}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Force-directed graph visualization
#[component]
fn ForceGraph(
    nodes: Signal<Vec<GraphNode>>,
    edges: Signal<Vec<GraphEdge>>,
) -> impl IntoView {
    let container_ref = create_node_ref::<html::Div>();
    let (width, set_width) = create_signal(800.0);
    let (height, set_height) = create_signal(600.0);
    
    // Transform state (Pan/Zoom)
    let (transform, set_transform) = create_signal((0.0, 0.0, 1.0)); // x, y, scale
    let (is_panning, set_is_panning) = create_signal(false);
    let (last_mouse_pos, set_last_mouse_pos) = create_signal((0.0, 0.0));
    let (selected_node_id, set_selected_node_id) = create_signal::<Option<String>>(None);

    let selected_node = move || {
        let id = selected_node_id.get()?;
        nodes.get().into_iter().find(|n| n.id == id)
    };

    // Physics Simulation State
    let node_positions = Rc::new(RefCell::new(HashMap::<String, NodeState>::new()));
    let (render_nodes, set_render_nodes) = create_signal(Vec::<(GraphNode, f64, f64)>::new());
    let (dragged_node, set_dragged_node) = create_signal::<Option<String>>(None);

    // Initialize & Simulation Loop
    let node_positions_init = node_positions.clone();
    create_effect(move |_| {
         let ns = nodes.get();
        if ns.is_empty() { return; }
        let w = width.get();
        let h = height.get();
        let mut positions = node_positions_init.borrow_mut();
        // Remove nodes that no longer exist
        let current_ids: std::collections::HashSet<_> = ns.iter().map(|n| n.id.clone()).collect();
        positions.retain(|id, _| current_ids.contains(id));
        
        // Add new nodes
        for (i, node) in ns.iter().enumerate() {
            if !positions.contains_key(&node.id) {
                let angle = (i as f64) * 0.5;
                let radius = 60.0 + (i as f64).sqrt() * 15.0; 
                positions.insert(node.id.clone(), NodeState {
                    id: node.id.clone(),
                    x: w / 2.0 + radius * angle.cos(),
                    y: h / 2.0 + radius * angle.sin(),
                    vx: 0.0, vy: 0.0, is_dragged: false,
                });
            }
        }
        gloo_console::log!("Initialized positions for", ns.len(), "nodes. Width:", w, "Height:", h);
    });

    let node_positions_anim = node_positions.clone();
    create_effect(move |_| {
        let node_positions = node_positions_anim.clone();
        let handle = Rc::new(RefCell::new(None::<i32>));
        let handle_clone = handle.clone();
        #[allow(clippy::type_complexity)]
        let closure: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let closure_clone = closure.clone();

        *closure.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            let ns = nodes.get();
            let es = edges.get();
            let w = width.get();
            let h = height.get();
            
            if !ns.is_empty() {
                let mut positions = node_positions.borrow_mut();
                // Simulation Constants
                let repulsion = 15000.0;
                let spring_length = 200.0;
                let spring_k = 0.02;
                let damping = 0.85;
                let center_force = 0.02;
                let dt = 0.16;

                // Physics Steps
                 let ids: Vec<String> = positions.keys().cloned().collect();
                for i in 0..ids.len() {
                     let id1 = &ids[i];
                     let (p1_x, p1_y, is_dragged1) = {
                        let p = &positions[id1];
                        (p.x, p.y, p.is_dragged)
                    };

                    for id2 in ids.iter().skip(i + 1) {
                        
                        let (p2_x, p2_y, is_dragged2) = {
                             let p = &positions[id2];
                            (p.x, p.y, p.is_dragged)
                        };

                        let dx = p1_x - p2_x;
                        let dy = p1_y - p2_y;
                        let dist_sq = dx * dx + dy * dy;
                        let dist = dist_sq.sqrt().max(1.0);
                        let force = repulsion / dist_sq;
                        let fx = (dx / dist) * force;
                        let fy = (dy / dist) * force;
                        
                        if !is_dragged1 {
                             if let Some(n1) = positions.get_mut(id1) { n1.vx += fx * dt; n1.vy += fy * dt; }
                        }
                        if !is_dragged2 {
                             if let Some(n2) = positions.get_mut(id2) { n2.vx -= fx * dt; n2.vy -= fy * dt; }
                        }
                    }
                }
                
                for edge in &es {
                    // Quick check if both exist to avoid multiple lookups
                    if positions.contains_key(&edge.source) && positions.contains_key(&edge.target) {
                         let (p1_x, p1_y) = { let p = &positions[&edge.source]; (p.x, p.y) };
                         let (p2_x, p2_y) = { let p = &positions[&edge.target]; (p.x, p.y) };
                         
                        let dx = p2_x - p1_x;
                        let dy = p2_y - p1_y;
                        let dist = (dx * dx + dy * dy).sqrt().max(0.1);
                        let force = (dist - spring_length) * spring_k;
                        let fx = (dx / dist) * force;
                        let fy = (dy / dist) * force;
                        
                        if let Some(n1) = positions.get_mut(&edge.source) { if !n1.is_dragged { n1.vx += fx * dt; n1.vy += fy * dt; } }
                        if let Some(n2) = positions.get_mut(&edge.target) { if !n2.is_dragged { n2.vx -= fx * dt; n2.vy -= fy * dt; } }
                    }
                }

                for node in positions.values_mut() {
                    if node.is_dragged { node.vx = 0.0; node.vy = 0.0; continue; }
                    let dx = w / 2.0 - node.x;
                    let dy = h / 2.0 - node.y;
                    node.vx += dx * center_force * dt;
                    node.vy += dy * center_force * dt;
                    node.vx *= damping;
                    node.vy *= damping;
                    node.x += node.vx;
                    node.y += node.vy;
                }

                let mut render_list: Vec<(GraphNode, f64, f64)> = Vec::new();
                for node in ns {
                    if let Some(pos) = positions.get(&node.id) {
                        render_list.push((node.clone(), pos.x, pos.y));
                    }
                }
                set_render_nodes.set(render_list);
            }
            if let Ok(h) = window().request_animation_frame(closure_clone.borrow().as_ref().unwrap().as_ref().unchecked_ref()) {
                 *handle_clone.borrow_mut() = Some(h);
            }
        }) as Box<dyn FnMut()>));
        if let Ok(h) = window().request_animation_frame(closure.borrow().as_ref().unwrap().as_ref().unchecked_ref()) {
            *handle.borrow_mut() = Some(h);
        }
        on_cleanup(move || { if let Some(h) = *handle.borrow() { let _ = window().cancel_animation_frame(h); } });
    });

    // Interaction Handlers
    let handle_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let delta = -ev.delta_y() * 0.001;
        let (tx, ty, scale) = transform.get();
        set_transform.set((tx, ty, (scale + delta).clamp(0.1, 5.0)));
    };

    let node_positions_down = node_positions.clone();
    let handle_mouse_down = move |ev: MouseEvent, node_id: Option<String>| {
        if let Some(id) = node_id {
            ev.stop_propagation();
            set_dragged_node.set(Some(id.clone()));
            set_selected_node_id.set(Some(id.clone()));
            let mut positions = node_positions_down.borrow_mut();
            if let Some(node) = positions.get_mut(&id) { node.is_dragged = true; node.vx=0.0; node.vy=0.0; }
        } else {
            set_is_panning.set(true);
            set_last_mouse_pos.set((ev.client_x() as f64, ev.client_y() as f64));
            set_selected_node_id.set(None);
        }
    };

    let node_positions_up = node_positions.clone();
    let handle_mouse_up = move |_| {
        set_is_panning.set(false);
        if let Some(id) = dragged_node.get() {
            let mut positions = node_positions_up.borrow_mut();
            if let Some(node) = positions.get_mut(&id) { node.is_dragged = false; }
            set_dragged_node.set(None);
        }
    };

    let handle_mouse_move = move |ev: MouseEvent| {
        if is_panning.get() {
            let (lx, ly) = last_mouse_pos.get();
            let cx = ev.client_x() as f64;
            let cy = ev.client_y() as f64;
            let (tx, ty, s) = transform.get();
            set_transform.set((tx + cx - lx, ty + cy - ly, s));
            set_last_mouse_pos.set((cx, cy));
        } else if let Some(id) = dragged_node.get() {
             if let Some(div) = container_ref.get() {
                let rect = div.get_bounding_client_rect();
                let params = transform.get(); 
                let node_x = (ev.client_x() as f64 - rect.left() - params.0) / params.2;
                let node_y = (ev.client_y() as f64 - rect.top() - params.1) / params.2;
                let mut positions = node_positions.borrow_mut();
                if let Some(node) = positions.get_mut(&id) { node.x = node_x; node.y = node_y; node.vx=0.0; node.vy=0.0; }
            }
        }
    };
    
    // Resize Observer
    create_effect(move |_| {
         if let Some(div) = container_ref.get() {
            let w = div.client_width() as f64;
            let h = div.client_height() as f64;
            set_width.set(w);
            set_height.set(h);
            gloo_console::log!("ForceGraph Resize:", w, h);
        }
    });

    // Prepare clones for event handlers
    let on_mouse_down_container = handle_mouse_down.clone();
    let on_mouse_up_container = handle_mouse_up.clone();
    let on_mouse_leave_container = handle_mouse_up.clone();
    
    let on_mouse_down_node_template = handle_mouse_down.clone();

    view! {
        <div 
            node_ref=container_ref
            class="w-full h-full relative cursor-crosshair group overflow-hidden"
            on:wheel=handle_wheel
            on:mousemove=handle_mouse_move
            on:mouseup=on_mouse_up_container
            on:mouseleave=on_mouse_leave_container
            on:mousedown=move |ev| on_mouse_down_container(ev, None)
        >
            // Grid Background
            <div class="absolute inset-0 pointer-events-none opacity-20" 
                style="background-image: radial-gradient(#3B82F6 1px, transparent 1px); background-size: 30px 30px;">
            </div>

            <svg width="100%" height="100%" class="absolute inset-0">
                <defs>
                     <filter id="glow-node">
                        <feGaussianBlur stdDeviation="3.5" result="coloredBlur"/>
                        <feMerge>
                            <feMergeNode in="coloredBlur"/>
                            <feMergeNode in="SourceGraphic"/>
                        </feMerge>
                    </filter>
                    <radialGradient id="grad-node" cx="50%" cy="50%" r="50%" fx="50%" fy="50%">
                        <stop offset="0%" style="stop-color:white;stop-opacity:0.9" />
                        <stop offset="100%" style="stop-color:currentColor;stop-opacity:1" />
                    </radialGradient>
                </defs>
                
                <g transform=move || { let (tx, ty, s) = transform.get(); format!("translate({} {}) scale({})", tx, ty, s) }>
                    // Edges
                    <For
                        each=move || {
                            let nodes = render_nodes.get();
                            let pos_map: HashMap<String, (f64, f64)> = nodes.iter().map(|(n, x, y)| (n.id.clone(), (*x, *y))).collect();
                            edges.get().iter().filter_map(|e| {
                                let src = pos_map.get(&e.source)?;
                                let tgt = pos_map.get(&e.target)?;
                                Some((e.clone(), src.0, src.1, tgt.0, tgt.1))
                            }).collect::<Vec<_>>()
                        }
                        key=|(e, ..)| format!("{}-{}", e.source, e.target)
                        let:edge_data
                    >
                        <line
                            x1=edge_data.1 y1=edge_data.2 x2=edge_data.3 y2=edge_data.4
                            stroke="rgba(100, 116, 139, 0.3)"
                            stroke-width="1.5"
                            class="transition-opacity"
                        />
                    </For>

                    // Nodes
                    <For
                        each=move || render_nodes.get()
                        key=|(node, ..)| node.id.clone()
                        let:data
                    >
                        {
                            let (node, x, y) = data;
                            let id = node.id.clone();
                            let is_selected = selected_node_id.get() == Some(id.clone());
                            // Use semantic colors
                            let color = match node.kind.as_str() {
                                "file" => "var(--color-info)",
                                "function" => "var(--color-success)",
                                "tool" => "var(--color-warning)",
                                "error" => "var(--color-error)",
                                _ => "var(--color-primary-600)",
                            };
                            let radius = if is_selected { 16.0 } else { 12.0 + (node.mention_count as f64).sqrt() * 2.0 };
                            
                            let on_mousedown = on_mouse_down_node_template.clone();

                            view! {
                                <g 
                                    transform=format!("translate({}, {})", x, y)
                                    class="cursor-pointer transition-all duration-300"
                                    on:mousedown=move |ev| on_mousedown(ev, Some(id.clone()))
                                    style=format!("color: {};", color)
                                >
                                    // Outer Glow
                                    <circle r={radius * 1.5} fill=color fill-opacity="0.2" class={if is_selected {"animate-pulse"} else {""}} />
                                    // Main Body
                                    <circle r=radius fill=color stroke="white" stroke-width={if is_selected {"2"} else {"1"}} filter="url(#glow-node)" />
                                    // Label
                                    <text y={radius + 15.0} text-anchor="middle" font-size="10" 
                                        fill="white" class="pointer-events-none select-none drop-shadow-md font-bold tracking-wide"
                                        style="text-shadow: 0 2px 4px rgba(0,0,0,0.8);">
                                        {if node.label.len() > 12 && !is_selected { format!("{}...", &node.label[..10]) } else { node.label }}
                                    </text>
                                </g>
                            }
                        }
                    </For>
                </g>
            </svg>

            // Controls Overlay
            <div class="absolute bottom-6 left-6 flex gap-2 pointer-events-auto">
                <button 
                    class="p-2 bg-black/60 backdrop-blur text-white border border-white/10 rounded-lg hover:bg-white/10 transition-all active:scale-95"
                    on:click=move |_| set_transform.set((0.0, 0.0, 1.0))
                    title="Reset View"
                >
                    <span class="text-lg">"⤢"</span>
                </button>
                 <div class="px-3 py-2 bg-black/60 backdrop-blur border border-white/10 rounded-lg text-xs font-mono text-theme-info">
                    {move || format!("ZOOM: {:.0}%", transform.get().2 * 100.0)}
                </div>
            </div>

            // Legend
            <div class="absolute bottom-6 right-6 bg-black/60 backdrop-blur border border-white/10 rounded-xl p-4 pointer-events-auto">
                <div class="text-[10px] font-bold text-white/50 uppercase mb-3">"Semantic Types"</div>
                <div class="grid grid-cols-2 gap-x-6 gap-y-2">
                    <div class="flex items-center gap-2"><div class="w-2 h-2 rounded-full bg-theme-info shadow-[0_0_5px_var(--color-info)]"></div><span class="text-xs text-white/80">"File"</span></div>
                    <div class="flex items-center gap-2"><div class="w-2 h-2 rounded-full bg-theme-success shadow-[0_0_5px_var(--color-success)]"></div><span class="text-xs text-white/80">"Function"</span></div>
                    <div class="flex items-center gap-2"><div class="w-2 h-2 rounded-full bg-theme-warning shadow-[0_0_5px_var(--color-warning)]"></div><span class="text-xs text-white/80">"Tool"</span></div>
                    <div class="flex items-center gap-2"><div class="w-2 h-2 rounded-full bg-purple-500 shadow-[0_0_5px_#a855f7]"></div><span class="text-xs text-white/80">"System"</span></div>
                </div>
            </div>
            
            // Selection Detail Panel
             <Show when=move || selected_node().is_some()>
                <div class="absolute top-6 left-6 w-72 bg-black/80 backdrop-blur-xl border border-white/10 rounded-2xl p-6 shadow-2xl animate-in slide-in-from-left-4 duration-300">
                    {
                        let node = selected_node().unwrap();
                        let label = node.label.clone();
                        let kind = node.kind.clone();
                        let id_str = node.id.clone();
                        let mentions = node.mention_count;

                        view! {
                            <div class="space-y-4">
                                <div class="flex items-start justify-between">
                                    <h3 class="text-lg font-bold text-white break-words">{label}</h3>
                                    <button class="text-white/50 hover:text-white" on:click=move |_| set_selected_node_id.set(None)>"✕"</button>
                                </div>
                                <div class="flex items-center gap-2">
                                    <span class="px-2 py-0.5 rounded-full text-[10px] uppercase font-bold bg-white/10 text-white/80 border border-white/10">
                                        {kind}
                                    </span>
                                    <span class="text-xs text-white/40">"ID: " {if id_str.len() > 8 { format!("{}...", &id_str[..8]) } else { id_str }}</span>
                                </div>
                                <div class="grid grid-cols-2 gap-2 pt-2">
                                    <div class="bg-white/5 p-2 rounded text-center">
                                        <div class="text-xs text-white/40 uppercase">"Mentions"</div>
                                        <div class="text-lg font-bold text-theme-info">{mentions}</div>
                                    </div>
                                    // Placeholder for other stats
                                    <div class="bg-white/5 p-2 rounded text-center">
                                        <div class="text-xs text-white/40 uppercase">"Strength"</div>
                                        <div class="text-lg font-bold text-theme-success">"High"</div>
                                    </div>
                                </div>
                            </div>
                        }
                    }
                </div>
            </Show>
        </div>
    }
}
