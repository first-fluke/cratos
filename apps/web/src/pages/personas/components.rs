use leptos::*;
use super::{PersonaDetailData};

#[component]
pub fn PersonaDetailModal<F>(
    selected_name: ReadSignal<Option<String>>,
    detail_data: ReadSignal<Option<PersonaDetailData>>,
    loading: ReadSignal<bool>,
    on_close: F,
) -> impl IntoView
where
    F: Fn() + Clone + 'static,
{
    view! {
        <Show when=move || selected_name.get().is_some()>
            {
                let on_close_c1 = on_close.clone();
                let on_close_c2 = on_close.clone();
                view! {
                    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200"
                        on:click=move |_| on_close_c1()>
                        <div class="bg-theme-card border border-theme-border-default rounded-2xl shadow-2xl w-full max-w-2xl max-h-[90vh] overflow-y-auto"
                            on:click=move |ev| ev.stop_propagation()>
                            
                            <div class="p-6 space-y-6">
                                // Modal Header
                                <div class="flex justify-between items-start">
                                    <div>
                                        <h2 class="text-2xl font-bold text-theme-primary capitalize">
                                            {move || selected_name.get().unwrap_or_default()}
                                        </h2>
                                        <p class="text-theme-secondary">"Detailed Analysis & Skills"</p>
                                    </div>
                                    <button class="text-theme-muted hover:text-theme-primary p-2" on:click=move |_| on_close_c2()>
                                        "✕"
                                    </button>
                                </div>

                                <Show when=move || loading.get()>
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
                }
            }
        </Show>
    }
}

#[component]
pub fn PersonaLegend() -> impl IntoView {
    view! {
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
    }
}
