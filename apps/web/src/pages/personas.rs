//! Personas Page
//!
//! Displays the Olympus OS persona pantheon with premium visual cards.

use leptos::*;
use leptos_router::*;
use serde::Deserialize;

use crate::api::ApiClient;

/// Persona summary from API
#[derive(Debug, Clone, Deserialize)]
pub struct PersonaSummary {
    pub name: String,
    pub level: u8,
    pub status: String,
    pub role: String,
    pub domain: String,
    #[serde(default, alias = "rating_score")]
    pub rating: Option<f32>,
    #[serde(default, alias = "objectivesCount")]
    pub objectives_count: usize,
    #[serde(default, alias = "questsCompleted")]
    pub quests_completed: usize,
    #[serde(default, alias = "questsTotal")]
    pub quests_total: usize,
    #[serde(default, alias = "skillCount")]
    pub skill_count: usize,
}

/// API response
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    data: Vec<PersonaSummary>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PersonaDetailData {
    pub persona: PersonaInfo,
    pub chronicle: Option<Chronicle>,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PersonaInfo {
    pub name: String,
    pub role: String,
    pub domain: String,
    pub level: u8,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Chronicle {
    pub rating: Option<f32>,
    pub objectives: Vec<String>, // Simplified for display
    pub quests: Vec<String>, // Simplified
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PersonaDetailResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: PersonaDetailData,
}

/// Personas page showing the Olympus pantheon
#[component]
pub fn Personas() -> impl IntoView {
    let (personas, set_personas) = create_signal::<Vec<PersonaSummary>>(Vec::new());
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);
    
    // Detail Modal State
    let (selected_persona_name, set_selected_persona_name) = create_signal::<Option<String>>(None);
    let (detail_data, set_detail_data) = create_signal::<Option<PersonaDetailData>>(None);
    let (detail_loading, set_detail_loading) = create_signal(false);

    // Fetch personas on mount
    create_effect(move |_| {
        spawn_local(async move {
            let client = ApiClient::new();
            match client.get::<ApiResponse>("/api/v1/pantheon").await {
                Ok(resp) => {
                    set_personas.set(resp.data);
                }
                Err(e) => {
                    gloo_console::error!("Failed to fetch personas:", e.clone());
                    set_error.set(Some(e));
                }
            }
            set_loading.set(false);
        });
    });

    // Fetch details when selection changes
    create_effect(move |_| {
        if let Some(name) = selected_persona_name.get() {
            set_detail_loading.set(true);
            set_detail_data.set(None);
            spawn_local(async move {
                let client = ApiClient::new();
                match client.get::<PersonaDetailResponse>(&format!("/api/v1/pantheon/{}", name)).await {
                    Ok(resp) => {
                        set_detail_data.set(Some(resp.data));
                    }
                    Err(e) => {
                        gloo_console::error!("Failed to fetch persona details:", e);
                    }
                }
                set_detail_loading.set(false);
            });
        } else {
            set_detail_data.set(None);
        }
    });

    let on_close = move || set_selected_persona_name.set(None);

    view! {
        <div class="space-y-8 animate-in fade-in duration-500 relative">
            // Header
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-theme-primary to-purple-400">
                        "Olympus Pantheon"
                    </h1>
                    <p class="text-theme-secondary mt-1 text-lg">"Active Intelligence Agents"</p>
                </div>
            </div>

            // Error display
            <Show when=move || error.get().is_some()>
                <div class="bg-theme-error/10 border border-theme-error/30 rounded-lg p-4 text-theme-error">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            // Loading indicator
            <Show when=move || loading.get()>
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    <div class="h-64 rounded-xl bg-theme-elevated animate-pulse"></div>
                    <div class="h-64 rounded-xl bg-theme-elevated animate-pulse"></div>
                    <div class="h-64 rounded-xl bg-theme-elevated animate-pulse"></div>
                </div>
            </Show>

            // Personas grid
            <Show when=move || !loading.get()>
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
                    <For
                        each=move || personas.get()
                        key=|p| p.name.clone()
                        let:persona
                    >
                         <PersonaCard 
                            persona=persona.clone() 
                            on_click=move || set_selected_persona_name.set(Some(persona.name.clone())) 
                        />
                    </For>
                </div>
            </Show>

            // Detail Modal
            <Show when=move || selected_persona_name.get().is_some()>
                <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200"
                    on:click=move |_| on_close()>
                    <div class="bg-theme-card border border-theme-border-default rounded-2xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-y-auto"
                        on:click=move |ev| ev.stop_propagation()>
                        
                        <div class="p-6 space-y-6">
                            // Modal Header
                            <div class="flex justify-between items-start">
                                <div>
                                    <h2 class="text-2xl font-bold text-theme-primary capitalize">
                                        {move || selected_persona_name.get().unwrap_or_default()}
                                    </h2>
                                    <p class="text-theme-secondary">"Detailed Analysis & Skills"</p>
                                </div>
                                <button class="text-theme-muted hover:text-theme-primary p-2" on:click=move |_| on_close()>
                                    "✕"
                                </button>
                            </div>

                            <Show when=move || detail_loading.get()>
                                <div class="flex justify-center py-12">
                                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-theme-primary"></div>
                                </div>
                            </Show>

                            <Show when=move || detail_data.get().is_some()>
                                {
                                    let data = detail_data.get().unwrap();
                                    let skills_vec = data.skills.clone();
                                    let skills_empty = skills_vec.is_empty();
                                    view! {
                                        <div class="space-y-6">
                                            // Info & Stats
                                            <div class="grid grid-cols-2 gap-4">
                                                <div class="bg-theme-base/50 p-4 rounded-xl">
                                                    <div class="text-sm text-theme-muted uppercase">"Role"</div>
                                                    <div class="font-bold text-lg">{data.persona.role}</div>
                                                </div>
                                                <div class="bg-theme-base/50 p-4 rounded-xl">
                                                    <div class="text-sm text-theme-muted uppercase">"Domain"</div>
                                                    <div class="font-bold text-lg">{data.persona.domain}</div>
                                                </div>
                                            </div>

                                            // Skills List
                                            <div>
                                                <h3 class="text-lg font-bold mb-3 flex items-center gap-2">
                                                    <span class="w-2 h-2 rounded-full bg-theme-info"></span>
                                                    "Aquired Skills"
                                                </h3>
                                                <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
                                                    <For
                                                        each=move || skills_vec.clone()
                                                        key=|s| s.clone()
                                                        let:skill
                                                    >
                                                        <div class="px-3 py-2 bg-theme-elevated rounded border border-theme-border-default text-sm flex items-center justify-between group hover:border-theme-info/50 transition-colors">
                                                            <span>{skill}</span>
                                                            <span class="opacity-0 group-hover:opacity-100 text-theme-info text-xs">"READY"</span>
                                                        </div>
                                                    </For>
                                                    <Show when=move || skills_empty>
                                                        <div class="col-span-full text-center text-theme-muted italic py-4">"No skills registered."</div>
                                                    </Show>
                                                </div>
                                            </div>

                                            // Chronicle Summary (Placeholder if empty)
                                            <div>
                                                <h3 class="text-lg font-bold mb-3 flex items-center gap-2">
                                                    <span class="w-2 h-2 rounded-full bg-theme-success"></span>
                                                    "Chronicle Log"
                                                </h3>
                                                <div class="bg-black/30 rounded-xl p-4 font-mono text-xs text-theme-secondary h-32 overflow-y-auto">
                                                    <div>"// Recent activity log"</div>
                                                    <div>"// Accessing secure archives..."</div>
                                                    {if data.chronicle.is_none() { 
                                                        view! { <div>"No chronicle data available."</div> }.into_view()
                                                    } else {
                                                        view! { <div>"Chronicle sync active. Stats updated."</div> }.into_view()
                                                    }}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }
                            </Show>
                        </div>
                    </div>
                </div>
            </Show>

            // Legend (Styled)
            <div class="mt-12 bg-theme-card/50 backdrop-blur border border-theme-border-default rounded-xl p-6">
                <h3 class="text-sm font-bold uppercase tracking-wider text-theme-muted mb-4 opacity-70">"Hierarchy Levels"</h3>
                <div class="grid grid-cols-2 md:grid-cols-4 gap-6">
                    <div class="flex items-center gap-3">
                        <div class="w-2 h-8 rounded bg-theme-info shadow-[0_0_10px_var(--color-info)]"></div>
                        <div>
                            <span class="block text-theme-primary font-bold">"Lv 1-2"</span>
                            <span class="text-xs text-theme-secondary">"Operator"</span>
                        </div>
                    </div>
                    // ... (rest of legend same)
                    <div class="flex items-center gap-3">
                        <div class="w-2 h-8 rounded bg-theme-success shadow-[0_0_10px_var(--color-success)]"></div>
                        <div>
                            <span class="block text-theme-primary font-bold">"Lv 3-4"</span>
                            <span class="text-xs text-theme-secondary">"Specialist"</span>
                        </div>
                    </div>
                    <div class="flex items-center gap-3">
                        <div class="w-2 h-8 rounded bg-purple-500 shadow-[0_0_10px_#a855f7]"></div>
                        <div>
                            <span class="block text-theme-primary font-bold">"Lv 5"</span>
                            <span class="text-xs text-theme-secondary">"Leader"</span>
                        </div>
                    </div>
                    <div class="flex items-center gap-3">
                        <div class="w-2 h-8 rounded bg-theme-warning shadow-[0_0_10px_var(--color-warning)]"></div>
                        <div>
                            <span class="block text-theme-primary font-bold">"Lv 255"</span>
                            <span class="text-xs text-theme-secondary">"Architect"</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Persona card component (Hero Style)
#[component]
fn PersonaCard<F>(persona: PersonaSummary, on_click: F) -> impl IntoView 
where F: Fn() + 'static + Clone
{
    let (level_color, border_color) = match persona.level {
        255 => ("text-theme-warning", "border-theme-warning/50"),
        5 => ("text-purple-500", "border-purple-500/50"),
        3..=4 => ("text-theme-success", "border-theme-success/50"),
        _ => ("text-theme-info", "border-theme-info/50"),
    };

    let status_indicator = match persona.status.as_str() {
        "Active" => "bg-theme-success shadow-[0_0_8px_var(--color-success)]",
        "Silenced" => "bg-theme-error",
        "Promoted" => "bg-purple-500 shadow-[0_0_8px_#a855f7]",
        _ => "bg-theme-muted",
    };

    let rating_display = persona.rating.unwrap_or(0.0);
    
    // Quest Progress
    let quest_pct = if persona.quests_total > 0 {
        (persona.quests_completed as f32 / persona.quests_total as f32) * 100.0
    } else {
        0.0
    };

    view! {
        <div 
            class="group relative bg-theme-card hover:bg-theme-elevated transition-all duration-300 rounded-2xl border border-theme-border-default hover:border-theme-border-hover overflow-hidden shadow-xl hover:shadow-2xl hover:-translate-y-1 cursor-pointer"
            on:click=move |_| on_click()
        >
            // Decorative background glow
            <div class={format!("absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-transparent to-current opacity-5 rounded-bl-full pointer-events-none {}", level_color)}></div>

            <div class="p-6 space-y-6">
                // Header
                <div class="flex items-start justify-between">
                    <div class="flex items-center gap-4">
                        // Avatar placeholder
                        <div class={format!("w-12 h-12 rounded-xl flex items-center justify-center text-xl font-bold bg-theme-base border {}", border_color)}>
                            <span class={level_color}>{persona.name.chars().next().unwrap_or('?')}</span>
                        </div>
                        <div>
                            <h3 class="text-xl font-bold text-theme-primary">{persona.name.clone()}</h3>
                            <div class="flex items-center gap-2 text-sm text-theme-secondary">
                                <span>{persona.role}</span>
                                <span class="text-theme-muted">"·"</span>
                                <span>{persona.domain}</span>
                            </div>
                        </div>
                    </div>
                    <div class="text-right">
                        <div class={format!("text-2xl font-black {}", level_color)}>
                            "Lv " {persona.level}
                        </div>
                        <div class="flex items-center justify-end gap-1.5 mt-1">
                            <div class={format!("w-2 h-2 rounded-full {}", status_indicator)}></div>
                            <span class="text-xs font-medium text-theme-secondary uppercase">{persona.status}</span>
                        </div>
                    </div>
                </div>

                // Stats Grid
                <div class="grid grid-cols-3 gap-2 py-2">
                    <div class="bg-theme-base/50 rounded-lg p-3 text-center border border-theme-border-default">
                        <div class="text-lg font-bold text-theme-primary">{format!("{:.1}", rating_display)}</div>
                        <div class="text-[10px] uppercase tracking-wider text-theme-muted">"Rating"</div>
                    </div>
                    <div class="bg-theme-base/50 hover:bg-theme-elevated transition-colors rounded-lg p-3 text-center border border-theme-border-default block">
                        <div class="text-lg font-bold text-theme-primary">{persona.skill_count}</div>
                        <div class="text-[10px] uppercase tracking-wider text-theme-muted">"Skills"</div>
                    </div>
                    <div class="bg-theme-base/50 rounded-lg p-3 text-center border border-theme-border-default">
                        <div class="text-lg font-bold text-theme-primary">{persona.objectives_count}</div>
                        <div class="text-[10px] uppercase tracking-wider text-theme-muted">"Goals"</div>
                    </div>
                </div>

                // Quest Progress Bar
                <div class="space-y-2">
                    <div class="flex justify-between text-xs font-medium">
                        <span class="text-theme-secondary">"Quest Progress"</span>
                        <span class="text-theme-primary">{format!("{}/{}", persona.quests_completed, persona.quests_total)}</span>
                    </div>
                    <div class="h-2 bg-theme-base rounded-full overflow-hidden">
                        <div 
                            class="h-full bg-gradient-to-r from-theme-info to-theme-success transition-all duration-500 rounded-full"
                            style=format!("width: {}%", quest_pct)
                        ></div>
                    </div>
                </div>
            </div>
        </div>
    }
}