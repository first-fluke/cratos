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
    #[serde(default, alias = "objectives_count", alias = "objectivesCount")]
    pub objectives_count: usize,
    #[serde(default, alias = "quests_completed", alias = "questsCompleted")]
    pub quests_completed: usize,
    #[serde(default, alias = "quests_total", alias = "questsTotal")]
    pub quests_total: usize,
    #[serde(default, alias = "skill_count", alias = "skillCount")]
    pub skill_count: usize,
    // Fallback list fields
    #[serde(default)]
    pub objectives: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub quests: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub skills: Option<Vec<serde_json::Value>>,
}

/// API response
#[derive(Debug, Clone, Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: Vec<PersonaSummary>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PersonaDetailData {
    pub persona: PersonaInfo,
    #[serde(default)]
    pub chronicle: Option<Chronicle>,
    #[serde(default)]
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
struct Quest {
    pub description: String,
    pub completed: bool,
    #[serde(alias = "completedAt")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ChronicleEntry {
    pub timestamp: String,
    pub achievement: String,
    #[serde(alias = "lawReference")]
    pub law_reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Chronicle {
    #[serde(alias = "personaName")]
    pub persona_name: String,
    pub level: u8,
    pub status: String,
    pub rating: Option<f32>,
    pub objectives: Vec<String>,
    pub quests: Vec<Quest>,
    pub log: Vec<ChronicleEntry>,
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
                                                        each=move || skills_vec.clone().into_iter().enumerate()
                                                        key=|(i, _)| *i
                                                        let:skill_tuple
                                                    >
                                                        <div class="px-3 py-2 bg-theme-elevated rounded border border-theme-border-default text-sm flex items-center justify-between group hover:border-theme-info/50 transition-colors">
                                                            <span>{skill_tuple.1}</span>
                                                            <span class="opacity-0 group-hover:opacity-100 text-theme-info text-xs">"READY"</span>
                                                        </div>
                                                    </For>
                                                    <Show when=move || skills_empty>
                                                        <div class="col-span-full text-center text-theme-muted italic py-4">"No skills registered."</div>
                                                    </Show>
                                                </div>
                                            </div>

                                            // Chronicle Summary (Detailed)
                                            <div>
                                                <h3 class="text-lg font-bold mb-3 flex items-center gap-2">
                                                    <span class="w-2 h-2 rounded-full bg-theme-success"></span>
                                                    "Chronicle & Log"
                                                </h3>
                                                <div class="space-y-4">
                                                    {if let Some(chronicle) = data.chronicle.clone() {
                                                        let objectives = chronicle.objectives.clone();
                                                        let quests = chronicle.quests.clone();
                                                        let logs = chronicle.log.clone();
                                                        
                                                        let objectives_empty = objectives.is_empty();
                                                        let quests_empty = quests.is_empty();
                                                        let logs_empty = logs.is_empty();
                                                        
                                                        view! {
                                                            <div class="space-y-6">
                                                                // Objectives
                                                                <div class="space-y-2">
                                                                    <div class="text-xs font-bold text-theme-muted uppercase tracking-wider">"Core Objectives"</div>
                                                                    <div class="flex flex-wrap gap-2">
                                                                        <For
                                                                            each=move || objectives.clone()
                                                                            key=|o| o.clone()
                                                                            let:obj
                                                                        >
                                                                            <span class="px-2 py-1 bg-theme-success/10 text-theme-success border border-theme-success/20 rounded text-xs">
                                                                                {obj}
                                                                            </span>
                                                                        </For>
                                                                        <Show when=move || objectives_empty>
                                                                            <span class="text-theme-muted text-xs italic">"No objectives set."</span>
                                                                        </Show>
                                                                    </div>
                                                                </div>

                                                                // Quests
                                                                <div class="space-y-2">
                                                                    <div class="text-xs font-bold text-theme-muted uppercase tracking-wider">"Active Quests"</div>
                                                                    <div class="grid grid-cols-1 gap-2">
                                                                        <For
                                                                            each=move || quests.clone()
                                                                            key=|q| q.description.clone()
                                                                            let:quest
                                                                        >
                                                                            <div class="flex items-center gap-3 p-2 bg-black/20 rounded border border-theme-border-default/50 text-xs">
                                                                                <div class={format!("w-1.5 h-1.5 rounded-full {}", if quest.completed { "bg-theme-success" } else { "bg-theme-warning animate-pulse" })}></div>
                                                                                <span class={if quest.completed { "line-through text-theme-muted" } else { "text-theme-secondary" }}>
                                                                                    {quest.description}
                                                                                </span>
                                                                            </div>
                                                                        </For>
                                                                        <Show when=move || quests_empty>
                                                                            <div class="text-theme-muted text-xs italic">"No quests in progress."</div>
                                                                        </Show>
                                                                    </div>
                                                                </div>

                                                                // Log
                                                                <div class="space-y-2">
                                                                    <div class="text-xs font-bold text-theme-muted uppercase tracking-wider">"Recent Achievements"</div>
                                                                    <div class="space-y-2 max-h-48 overflow-y-auto pr-2 custom-scrollbar">
                                                                        <For
                                                                            each=move || logs.clone()
                                                                            key=|l| format!("{}-{}", l.timestamp, l.achievement)
                                                                            let:entry
                                                                        >
                                                                            <div class="p-2 bg-theme-elevated rounded border-l-2 border-theme-info text-xs">
                                                                                <div class="flex justify-between mb-1">
                                                                                    <span class="text-theme-info font-mono">{entry.timestamp.split('T').next().unwrap_or("").to_string()}</span>
                                                                                    {if let Some(law) = entry.law_reference {
                                                                                        view! { <span class="text-purple-400">"Article " {law}</span> }.into_view()
                                                                                    } else {
                                                                                        view! { }.into_view()
                                                                                    }}
                                                                                </div>
                                                                                <div class="text-theme-primary">{entry.achievement}</div>
                                                                            </div>
                                                                        </For>
                                                                        <Show when=move || logs_empty>
                                                                            <div class="text-theme-muted text-xs italic text-center py-4">"No logs recorded yet."</div>
                                                                        </Show>
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        }.into_view()
                                                    } else {
                                                        view! {
                                                            <div class="bg-black/30 rounded-xl p-4 font-mono text-xs text-theme-secondary h-32 flex items-center justify-center italic">
                                                                "No chronicle data available for this persona."
                                                            </div>
                                                        }.into_view()
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

        

        // Fallback counts from lists if needed

        let objectives_display = if persona.objectives_count > 0 {

            persona.objectives_count

        } else {

            persona.objectives.as_ref().map(|l| l.len()).unwrap_or(0)

        };

    

        let quests_total_display = if persona.quests_total > 0 {

            persona.quests_total

        } else {

            persona.quests.as_ref().map(|l| l.len()).unwrap_or(0)

        };

    

        let skills_display = if persona.skill_count > 0 {

            persona.skill_count

        } else {

            persona.skills.as_ref().map(|l| l.len()).unwrap_or(0)

        };

    

        // Quest Progress

        let quest_pct = if quests_total_display > 0 {

            (persona.quests_completed as f32 / quests_total_display as f32) * 100.0

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

                            <div class="text-lg font-bold text-theme-primary">{skills_display}</div>

                            <div class="text-[10px] uppercase tracking-wider text-theme-muted">"Skills"</div>

                        </div>

                        <div class="bg-theme-base/50 rounded-lg p-3 text-center border border-theme-border-default">

                            <div class="text-lg font-bold text-theme-primary">{objectives_display}</div>

                            <div class="text-[10px] uppercase tracking-wider text-theme-muted">"Goals"</div>

                        </div>

                    </div>

    

                    // Quest Progress Bar

                    <div class="space-y-2">

                        <div class="flex justify-between text-xs font-medium">

                            <span class="text-theme-secondary">"Quest Progress"</span>

                            <span class="text-theme-primary">{format!("{}/{}", persona.quests_completed, quests_total_display)}</span>

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

    