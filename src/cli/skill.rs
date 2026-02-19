//! Skill CLI commands
//!
//! `cratos skill` - List, show, enable, and disable skills

use super::SkillCommands;
use anyhow::{Context, Result};
use chrono::Utc;
use cratos_replay::{default_db_path as default_replay_db_path, EventStore};
use cratos_skills::{
    PatternAnalyzer, PatternStatus, RemoteRegistry, Skill, SkillEcosystem, SkillGenerator,
    SkillStatus, SkillStore,
};
use cratos_skills::ecosystem::PortableSkillDef;

/// Status indicator for active skill
const ICON_ACTIVE: &str = "\u{1f7e2}"; // 🟢
/// Status indicator for inactive/disabled/draft skill
const ICON_INACTIVE: &str = "\u{1f534}"; // 🔴

/// Run skill command
pub async fn run(cmd: SkillCommands) -> Result<()> {
    let store = open_store().await?;

    match cmd {
        SkillCommands::List { active } => list(&store, active).await,
        SkillCommands::Show { name } => show(&store, &name).await,
        SkillCommands::Enable { name } => enable(&store, &name).await,
        SkillCommands::Disable { name } => disable(&store, &name).await,
        SkillCommands::Export { name, output, markdown } => export_skill(&store, &name, output, markdown).await,
        SkillCommands::Import { path } => import_skill(&store, &path).await,
        SkillCommands::Bundle { name, output } => export_bundle(&store, &name, output).await,
        SkillCommands::Search { query, registry } => search_remote(&query, registry).await,
        SkillCommands::Install { name, registry } => install_remote(&store, &name, registry).await,
        SkillCommands::Publish {
            name,
            token,
            registry,
        } => publish_remote(&store, &name, token, registry).await,
        SkillCommands::Analyze { dry_run } => analyze_patterns(dry_run).await,
        SkillCommands::Generate { dry_run, enable } => {
            generate_skills(&store, dry_run, enable).await
        }
        SkillCommands::Prune {
            older_than,
            dry_run,
            confirm,
        } => prune(&store, older_than, dry_run, confirm).await,
    }
}

/// Analyze recent usage patterns
async fn analyze_patterns(dry_run: bool) -> Result<()> {
    let db_path = default_replay_db_path();
    let event_store = EventStore::from_path(&db_path).await?;
    let skill_store = open_store().await?;

    println!("Analyzing execution history for recurring patterns...");
    println!("Database: {}", db_path.display());

    // Use default config for now
    let analyzer = PatternAnalyzer::default();
    let patterns = analyzer.detect_patterns(&event_store).await?;

    if patterns.is_empty() {
        println!("No significant patterns detected.");
        return Ok(());
    }

    println!("Detected {} potential patterns:", patterns.len());

    let mut saved_count = 0;

    for (i, p) in patterns.iter().enumerate() {
        println!("\n[{}] Pattern Configuration:", i + 1);
        println!(
            "  Confidence: {:.1}% ({} occurrences)",
            p.confidence_score * 100.0,
            p.occurrence_count
        );
        println!("  Detected: {}", p.detected_at.format("%Y-%m-%d %H:%M:%S"));

        // Show sequence
        println!("  Sequence: {:?}", p.tool_sequence);

        if !dry_run {
            // Check if already exists (naive check by sequence? or just save and let DB handle unique constraint?)
            // SkillStore logic handles uniqueness usually or upserts.
            match skill_store.save_pattern(p).await {
                Ok(_) => saved_count += 1,
                Err(e) => println!("  ⚠️ Failed to save: {}", e),
            }
        }
    }

    if !dry_run {
        println!("\nSaved {} patterns to skill store.", saved_count);
        println!("Run 'cratos skill generate' to convert them into skills.");
    } else {
        println!("\nDry run: No patterns were saved.");
    }

    Ok(())
}

/// Generate skills from detected patterns
async fn generate_skills(store: &SkillStore, dry_run: bool, auto_enable: bool) -> Result<()> {
    // 1. Fetch pending patterns
    let patterns = store.list_detected_patterns().await?;
    let pending: Vec<_> = patterns
        .into_iter()
        .filter(|p| p.status == PatternStatus::Detected)
        .collect();

    if pending.is_empty() {
        println!("No pending patterns found to generate skills from.");
        println!("Run 'cratos skill analyze' first.");
        return Ok(());
    }

    // 2. Initialize Semantic Search (if possible)
    let semantic_router = if !dry_run {
        println!("Initializing Semantic Search Engine...");
        match initialize_semantic_router(store).await {
            Ok(router) => {
                println!("  ✅ Semantic Search initialized.");
                Some(router)
            }
            Err(e) => {
                println!("  ⚠️ Semantic Search unavailable: {}. Falling back to exact match.", e);
                None
            }
        }
    } else {
        None
    };

    println!(
        "Found {} pending patterns. Generating skills...",
        pending.len()
    );

    let generator = SkillGenerator::default();
    let mut generated_count = 0;

    for pattern in pending {
        println!("\nProcessing Pattern: {}", pattern.id);

        // Generate skill definition
        match generator.generate_from_pattern(&pattern) {
            Ok(mut skill) => {
                println!("  Generated Skill: '{}'", skill.name);
                println!("  Description: {:?}", skill.description);

                if auto_enable {
                    skill.status = SkillStatus::Active;
                    println!("  Status: Active (Auto-enabled)");
                }

                if !dry_run {
                    // 3. Check for existing skills (Semantic + Exact)
                    let mut existing_skill = None;
                    
                    // 3.1 First check exact match
                    match store.get_skill_by_name(&skill.name).await {
                        Ok(Some(s)) => {
                            println!("  ℹ️ Found exact match: '{}'", s.name);
                            existing_skill = Some(s);
                        },
                        Err(e) => println!("  ⚠️ Error checking exact match: {}", e),
                        _ => {}
                    }

                    // 3.2 If no exact match, try semantic search
                    if existing_skill.is_none() {
                        if let Some(router) = &semantic_router {
                            match router.semantic_search(&skill.name) {
                                Ok(matches) => {
                                    if let Some((best_name, score)) = matches.first() {
                                        if *score > 0.85 {
                                            println!("  ℹ️ Found semantic match: '{}' (Score: {:.2})", best_name, score);
                                            match store.get_skill_by_name(best_name).await {
                                                Ok(Some(s)) => existing_skill = Some(s),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                Err(e) => println!("  ⚠️ Semantic search failed: {}", e),
                            }
                        }
                    }

                    // 4. Handle Merge or Create
                    match existing_skill {
                        Some(mut existing) => {
                            println!("  Using existing skill: '{}' for merge.", existing.name);
                            
                            // Merge triggers
                            let merged = merge_skill_content(&mut existing, &skill);

                            if merged {
                                match store.save_skill(&existing).await {
                                    Ok(_) => {
                                         println!("  ✅ Merged new triggers into existing skill.");
                                         if let Err(e) = generate_agent_skill_files(&existing) {
                                              println!("  ⚠️ Failed to update agent skill files: {}", e);
                                         }
                                    }
                                    Err(e) => println!("  ❌ Failed to save merged skill: {}", e),
                                }
                            } else {
                                println!("  ℹ️ No new content to merge.");
                            }

                            if let Err(e) = store.mark_pattern_converted(pattern.id, existing.id).await {
                                println!("  ⚠️ Failed to mark pattern converted: {}", e);
                            }
                        }
                        None => {
                            // Save new skill
                            match store.save_skill(&skill).await {
                                Ok(()) => {
                                    if let Err(e) = store.mark_pattern_converted(pattern.id, skill.id).await
                                    {
                                        println!("  ⚠️ Failed to mark pattern converted: {}", e);
                                    } else {
                                        generated_count += 1;
                                        println!("  ✅ Saved and linked to pattern.");
                                        
                                        if let Err(e) = generate_agent_skill_files(&skill) {
                                            println!("  ⚠️ Failed to generate agent skill files: {}", e);
                                        }
                                        
                                        // Index new skill immediately if router is active
                                        if let Some(router) = &semantic_router {
                                            if let Err(e) = router.index_skill(&skill) {
                                                println!("  ⚠️ Failed to index new skill: {}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => println!("  ❌ Failed to save skill: {}", e),
                            }
                        }
                    }
                } else {
                    println!("  (Dry run: Skill not saved)");
                }
            }
            Err(e) => {
                println!("  ❌ Generation failed: {}", e);
            }
        }
    }

    if !dry_run {
        println!("\nSuccessfully generated {} new skills.", generated_count);
    }

    Ok(())
}

/// Helper to initialize Semantic Router
async fn initialize_semantic_router(store: &SkillStore) -> Result<SemanticSkillRouter<SkillEmbeddingAdapter>> {
    // 1. Load config
    // Note: load_config loads from .env and files
    let config = crate::server::load_config().await?;
    
    // 2. Get Embedding Provider (default_embedding_provider from cratos_llm)
    let provider = cratos_llm::default_embedding_provider()
        .context("Failed to create embedding provider")?;
        
    let embedder = Arc::new(SkillEmbeddingAdapter {
        provider
    });

    // 3. Create Index
    let index_path = cratos_skills::default_skill_db_path().with_extension("index");
    let index = create_skill_index(embedder.dimensions(), Some(&index_path))?;

    // 4. Create Registry & Load Skills
    let registry = Arc::new(SkillRegistry::new());
    let active_skills = store.list_active_skills().await?;
    registry.load_all(active_skills).await?;

    // 5. Create Router
    let router = SemanticSkillRouter::new(registry, index, embedder);
    
    // 6. Reindex to ensure everything is up to date
    router.reindex_all()?;
    
    Ok(router)
}

/// Helper to merge content from new skill into existing skill
fn merge_skill_content(existing: &mut Skill, new_skill: &Skill) -> bool {
    let mut merged = false;
    for k in &new_skill.trigger.keywords {
        if !existing.trigger.keywords.contains(k) {
            existing.trigger.keywords.push(k.clone());
            merged = true;
        }
    }
    // Merge regex patterns if implementation allows
    if !new_skill.trigger.regex_patterns.is_empty() {
        for p in &new_skill.trigger.regex_patterns {
            if !existing.trigger.regex_patterns.contains(p) {
                existing.trigger.regex_patterns.push(p.clone());
                merged = true;
            }
        }
    }
    merged
}

/// Open the default skill store
async fn open_store() -> Result<SkillStore> {
    let db_path = cratos_skills::default_skill_db_path();
    SkillStore::from_path(&db_path)
        .await
        .context("Failed to open skill store")
}

/// List skills
async fn list(store: &SkillStore, active_only: bool) -> Result<()> {
    let skills = if active_only {
        store.list_active_skills().await
    } else {
        store.list_skills().await
    }
    .context("Failed to list skills")?;

    let active_count = skills.iter().filter(|s| s.is_active()).count();
    let total = skills.len();

    if skills.is_empty() {
        println!("\nNo skills found.");
        println!("Skills are auto-generated from usage patterns or created manually.\n");
        return Ok(());
    }

    let filter_label = if active_only { " (active only)" } else { "" };
    println!(
        "\nCratos Skills ({} total, {} active){}\n{}",
        total,
        active_count,
        filter_label,
        "-".repeat(56)
    );

    for skill in &skills {
        let icon = status_icon(skill.status);
        let origin_label = match skill.origin {
            cratos_skills::SkillOrigin::AutoGenerated => "auto",
            cratos_skills::SkillOrigin::UserDefined => "user",
            cratos_skills::SkillOrigin::Builtin => "built",
        };
        let rate = format_rate(skill.metadata.success_rate, skill.metadata.usage_count);
        let ago = skill
            .metadata
            .last_used_at
            .map(format_duration_since)
            .unwrap_or_else(|| "never".to_string());
        let disabled_tag = if skill.status == SkillStatus::Disabled {
            " [disabled]"
        } else if skill.status == SkillStatus::Draft {
            " [draft]"
        } else {
            ""
        };

        println!(
            "  {} {:<26} {:<10} {:<6} {:<14} {}{}",
            icon, skill.name, skill.category, origin_label, rate, ago, disabled_tag,
        );
    }

    println!();
    Ok(())
}

/// Show skill details
async fn show(store: &SkillStore, name: &str) -> Result<()> {
    let skill = store
        .get_skill_by_name(name)
        .await
        .context("Failed to query skill")?;

    match skill {
        Some(skill) => print_skill_detail(&skill, store).await,
        None => {
            println!("\nSkill not found: {name}");
            println!("Run `cratos skill list` to see available skills.\n");
            Ok(())
        }
    }
}

/// Print detailed skill information
async fn print_skill_detail(skill: &Skill, store: &SkillStore) -> Result<()> {
    let icon = status_icon(skill.status);
    let status_label = match skill.status {
        SkillStatus::Active => "Active",
        SkillStatus::Disabled => "Disabled",
        SkillStatus::Draft => "Draft",
    };

    println!("\nSkill: {}\n", skill.name);
    println!("  Status:     {} {}", icon, status_label);
    println!("  Category:   {}", skill.category);
    println!("  Origin:     {}", skill.origin);

    // Triggers
    if !skill.trigger.keywords.is_empty()
        || !skill.trigger.intents.is_empty()
        || !skill.trigger.regex_patterns.is_empty()
    {
        println!("\n  Triggers:");
        if !skill.trigger.keywords.is_empty() {
            println!("    Keywords: {}", skill.trigger.keywords.join(", "));
        }
        if !skill.trigger.intents.is_empty() {
            println!("    Intents:  {}", skill.trigger.intents.join(", "));
        }
        if !skill.trigger.regex_patterns.is_empty() {
            println!("    Patterns: {}", skill.trigger.regex_patterns.join(", "));
        }
    }

    // Steps
    if !skill.steps.is_empty() {
        println!("\n  Steps:");
        for step in &skill.steps {
            let on_err = format!("[{} on error]", step.on_error);
            let input_preview =
                serde_json::to_string(&step.input_template).unwrap_or_else(|_| "{}".to_string());
            // Truncate long input previews safely
            let input_short = truncate_str(&input_preview, 40);
            println!(
                "    {}. {:<14} {} {}",
                step.order, step.tool_name, input_short, on_err
            );
        }
    }

    // Metrics
    let (total, successes) = store
        .get_skill_execution_count(skill.id)
        .await
        .unwrap_or((0, 0));
    let rate_pct = if total > 0 {
        (successes as f64 / total as f64) * 100.0
    } else {
        skill.metadata.success_rate * 100.0
    };

    println!("\n  Metrics:");
    println!(
        "    Usage:        {} executions",
        skill.metadata.usage_count
    );
    println!(
        "    Success rate: {:.1}% ({}/{})",
        rate_pct, successes, total
    );
    if let Some(avg_ms) = skill.metadata.avg_duration_ms {
        let avg_secs = avg_ms as f64 / 1000.0;
        println!("    Avg duration: {avg_secs:.1}s");
    }
    let last_used = skill
        .metadata
        .last_used_at
        .map(format_duration_since)
        .unwrap_or_else(|| "never".to_string());
    println!("    Last used:    {last_used}");

    println!();
    Ok(())
}

/// Enable a skill (set status to Active)
async fn enable(store: &SkillStore, name: &str) -> Result<()> {
    let skill = store
        .get_skill_by_name(name)
        .await
        .context("Failed to query skill")?;

    match skill {
        Some(mut skill) => {
            if skill.is_active() {
                println!("Skill '{name}' is already active.");
            } else {
                skill.activate();
                store
                    .save_skill(&skill)
                    .await
                    .context("Failed to save skill")?;
                println!("{ICON_ACTIVE} Skill '{name}' enabled.");
            }
            Ok(())
        }
        None => {
            println!("Skill not found: {name}");
            println!("Run `cratos skill list` to see available skills.");
            Ok(())
        }
    }
}

/// Disable a skill
async fn disable(store: &SkillStore, name: &str) -> Result<()> {
    let skill = store
        .get_skill_by_name(name)
        .await
        .context("Failed to query skill")?;

    match skill {
        Some(mut skill) => {
            if skill.status == SkillStatus::Disabled {
                println!("Skill '{name}' is already disabled.");
            } else {
                skill.disable();
                store
                    .save_skill(&skill)
                    .await
                    .context("Failed to save skill")?;
                println!("{ICON_INACTIVE} Skill '{name}' disabled.");
            }
            Ok(())
        }
        None => {
            println!("Skill not found: {name}");
            println!("Run `cratos skill list` to see available skills.");
            Ok(())
        }
    }
}

/// Export a skill to a JSON file or Agent Markdown
async fn export_skill(
    store: &SkillStore,
    name: &str,
    output: Option<String>,
    markdown: bool,
) -> Result<()> {
    let eco = SkillEcosystem::new(store.clone());
    let portable = eco
        .export_skill_by_name(name)
        .await
        .context("Failed to export skill")?;

    if markdown {
        // portable.skill is expected to be accessible
        generate_agent_skill_files_from_def(&portable.skill)?;
        println!("Exported agent skill files for '{}'", name);
    } else {
        let output_path = output.unwrap_or_else(|| format!("{}.skill.json", name));
        let json = serde_json::to_string_pretty(&portable).context("Failed to serialize skill")?;
        std::fs::write(&output_path, json).context("Failed to write file")?;
        println!("Exported skill '{}' to {}", name, output_path);
    }

    Ok(())
}

/// Import a skill from a JSON file
async fn import_skill(store: &SkillStore, path: &str) -> Result<()> {
    let eco = SkillEcosystem::new(store.clone());
    let file_path = std::path::Path::new(path);

    if path.ends_with(".bundle.json") {
        let results = eco
            .import_bundle_from_file(file_path)
            .await
            .context("Failed to import bundle")?;
        let new_count = results.iter().filter(|r| r.is_new).count();
        let updated = results.len() - new_count;
        println!("Bundle import: {} new, {} updated", new_count, updated);
        for r in &results {
            let status = if r.is_new { "new" } else { "updated" };
            println!("  {} {}", status, r.skill.name);
        }
    } else {
        let result = eco
            .import_from_file(file_path)
            .await
            .context("Failed to import skill")?;
        let status = if result.is_new { "Imported" } else { "Updated" };
        println!("{} skill: {}", status, result.skill.name);
        for warning in &result.warnings {
            println!("  Warning: {}", warning);
        }
    }
    Ok(())
}

/// Export all active skills as a bundle
async fn export_bundle(store: &SkillStore, name: &str, output: Option<String>) -> Result<()> {
    let eco = SkillEcosystem::new(store.clone());
    let bundle = eco
        .export_bundle(name, &format!("Cratos skill bundle: {}", name))
        .await
        .context("Failed to export bundle")?;

    let output_path = output.unwrap_or_else(|| format!("{}.skill.bundle.json", name));
    let json = serde_json::to_string_pretty(&bundle).context("Failed to serialize bundle")?;
    std::fs::write(&output_path, json).context("Failed to write file")?;

    println!("Exported {} skills to {}", bundle.skills.len(), output_path);
    Ok(())
}

// ─── Remote registry commands ────────────────────────────────

/// Search remote skill registry
async fn search_remote(query: &str, registry: Option<String>) -> Result<()> {
    let reg = match registry {
        Some(url) => RemoteRegistry::new(&url),
        None => RemoteRegistry::default_registry(),
    };

    let results = reg
        .search(query)
        .await
        .context("Failed to search remote registry")?;

    if results.is_empty() {
        println!("\nNo skills found matching '{query}'.");
        return Ok(());
    }

    println!(
        "\nRemote Skills matching '{}' ({} found)\n{}",
        query,
        results.len(),
        "-".repeat(60)
    );

    for entry in &results {
        println!(
            "  {:<24} v{:<8} {:<10} by {}",
            entry.name, entry.version, entry.category, entry.author
        );
        println!("    {}", entry.description);
    }
    println!();

    Ok(())
}

/// Install a skill from remote registry
async fn install_remote(store: &SkillStore, name: &str, registry: Option<String>) -> Result<()> {
    let reg = match registry {
        Some(url) => RemoteRegistry::new(&url),
        None => RemoteRegistry::default_registry(),
    };

    println!("Fetching skill '{}' from registry...", name);

    let portable = reg
        .fetch_skill(name)
        .await
        .context("Failed to fetch skill from registry")?;

    let eco = SkillEcosystem::new(store.clone());
    let result = eco
        .import_skill(&portable)
        .await
        .context("Failed to import skill")?;

    let status = if result.is_new {
        "Installed"
    } else {
        "Updated"
    };
    println!("{} skill: {}", status, result.skill.name);
    for warning in &result.warnings {
        println!("  Warning: {}", warning);
    }

    Ok(())
}

/// Publish a skill to remote registry
async fn publish_remote(
    store: &SkillStore,
    name: &str,
    token: Option<String>,
    registry: Option<String>,
) -> Result<()> {
    let token = match token {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!(
                "Registry token required. Use --token <TOKEN> or set CRATOS_REGISTRY_TOKEN."
            ));
        }
    };

    let eco = SkillEcosystem::new(store.clone());
    let portable = eco
        .export_skill_by_name(name)
        .await
        .context("Failed to export skill")?;

    let reg = match registry {
        Some(url) => RemoteRegistry::new(&url),
        None => RemoteRegistry::default_registry(),
    };

    println!("Publishing skill '{}' to registry...", name);

    reg.publish(&portable, &token)
        .await
        .context("Failed to publish skill")?;

    println!("Skill '{}' published successfully.", name);
    Ok(())
}

/// Prune stale skills
async fn prune(store: &SkillStore, older_than: u32, dry_run: bool, confirm: bool) -> Result<()> {
    let stale = store.list_stale_skills(older_than).await?;

    if stale.is_empty() {
        println!("No stale skills found (unused for {} days).", older_than);
        return Ok(());
    }

    println!(
        "Found {} stale skills (unused for {} days):",
        stale.len(),
        older_than
    );
    for skill in &stale {
        let ago = skill
            .metadata
            .last_used_at
            .map(format_duration_since)
            .unwrap_or_else(|| "never".to_string());
        println!("  - {} (Last used: {})", skill.name, ago);
    }

    if dry_run {
        println!("\nDry run: No skills deleted.");
        return Ok(());
    }

    if !confirm {
        println!("\nTo proceed with deletion, run with --confirm");
        return Ok(());
    }

    let deleted = store.prune_stale_skills(older_than).await?;
    println!("\nSuccessfully pruned {} skills.", deleted);
    Ok(())
}

// ─── Helper functions ────────────────────────────────────────

/// Return status icon for a skill status
fn status_icon(status: SkillStatus) -> &'static str {
    match status {
        SkillStatus::Active => ICON_ACTIVE,
        SkillStatus::Disabled | SkillStatus::Draft => ICON_INACTIVE,
    }
}

/// Format success rate as "XX.X% (N/M)"
fn format_rate(rate: f64, usage: u64) -> String {
    if usage == 0 {
        return "-- (0/0)".to_string();
    }
    let successes = (rate * usage as f64).round() as u64;
    format!("{:.1}% ({}/{})", rate * 100.0, successes, usage)
}

/// Format a duration since a timestamp (e.g., "2h ago", "3d ago")
fn format_duration_since(timestamp: chrono::DateTime<Utc>) -> String {
    let elapsed = Utc::now().signed_duration_since(timestamp);
    let secs = elapsed.num_seconds();

    if secs < 0 {
        return "just now".to_string();
    }

    let minutes = secs / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{days}d ago")
    } else if hours > 0 {
        format!("{hours}h ago")
    } else if minutes > 0 {
        format!("{minutes}m ago")
    } else {
        "just now".to_string()
    }
}

/// Safely truncate a string, respecting UTF-8 char boundaries
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|(i, _)| *i < max_len.saturating_sub(3))
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...", &s[..end])
}

/// Generate agent skill files using the skill-creator template
fn generate_agent_skill_files(skill: &cratos_skills::Skill) -> Result<()> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let skill_name = &skill.name;
    let skill_dir = Path::new(".cratos/skills").join(skill_name);
    let resources_dir = skill_dir.join("resources");

    // Create directories
    fs::create_dir_all(&resources_dir).context("Failed to create skill directory")?;

    // Load template
    let template_path = Path::new(".cratos/skills/skill-creator/resources/skill-template.md");
    let template = if template_path.exists() {
        fs::read_to_string(template_path).context("Failed to read skill template")?
    } else {
        // Fallback minimal template
        format!(
            "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntriggers:\n{}\n---\n\n# {}\n\n{}\n",
            skill_name,
            skill.description,
            skill.trigger.keywords.iter().map(|k| format!("  - \"{}\"", k)).collect::<Vec<_>>().join("\n"),
            skill_name,
            skill.description
        )
    };

    // Replace placeholders (simple replacement for now)
    let content = template
        .replace("{{skill_name}}", skill_name)
        .replace("{{short_description}}", &skill.description)
        .replace("{{Skill Name}}", skill_name)
        .replace("{{Detailed description of the skill's purpose and context.}}", &skill.description)
        .replace("{{trigger_keyword_1}}", skill.trigger.keywords.first().map(|s| s.as_str()).unwrap_or("trigger"))
        .replace("{{trigger_keyword_2}}", skill.trigger.keywords.get(1).map(|s| s.as_str()).unwrap_or(""))
        .replace("{{Primary responsibility 1}}", "Auto-generated implementation step")
        .replace("{{Primary responsibility 2}}", "Protocol verification")
        .replace("{{Rule 1: Most critical constraint or behavior}}", "Follow standard Cratos conventions.")
        .replace("{{Step Name}}", "Execute Logic")
        .replace("{{Description of step 1 actions.}}", "Run the tool sequence defined in the generated skill logic.");
        
    // Write SKILL.md
    let skill_md_path = skill_dir.join("SKILL.md");

    // Check if SKILL.md already exists
    if skill_md_path.exists() {
        println!("  ℹ️ updating existing SKILL.md...");
        
        let existing_content = fs::read_to_string(&skill_md_path).context("Failed to read existing SKILL.md")?;
        
        // Simple frontmatter parser/merger
        // We assume frontmatter is between first two ---
        let parts: Vec<&str> = existing_content.splitn(3, "---").collect();
        if parts.len() >= 3 {
             let frontmatter = parts[1];
             let body = parts[2];
             
             // Check if triggers are present in frontmatter
             // This is a naive line-based check/append. For robust YAML parsing, we'd need serde_yaml
             // But for now, let's just ensure our new keywords are present
             let mut new_frontmatter = frontmatter.to_string();
             if !new_frontmatter.contains("triggers:") {
                 new_frontmatter.push_str("\ntriggers:\n");
             }
             
             for k in &skill.trigger.keywords {
                 if !new_frontmatter.contains(k) {
                     new_frontmatter.push_str(&format!("  - \"{}\"\n", k));
                 }
             }
             
             let new_content = format!("---{}---{}", new_frontmatter, body);
             let mut file = fs::File::create(&skill_md_path).context("Failed to update SKILL.md")?;
             file.write_all(new_content.as_bytes())?;
             println!("  ✅ Merged triggers into SKILL.md");
             return Ok(());
        }
    }

    // Write New SKILL.md
    let mut file = fs::File::create(&skill_md_path).context("Failed to create SKILL.md")?;
    file.write_all(content.as_bytes())?;

    println!("  Generated agent skill file: {}", skill_md_path.display());
    
    // Create execution-protocol.md if it doesn't exist
    let protocol_path = resources_dir.join("execution-protocol.md");
    if !protocol_path.exists() {
        let mut f = fs::File::create(&protocol_path)?;
        writeln!(f, "# Execution Protocol for {}\n\n1. Analyze input.\n2. Execute step 1.\n3. Verify result.", skill_name)?;
    }
    
    // Create examples.md if it doesn't exist
    let examples_path = resources_dir.join("examples.md");
    if !examples_path.exists() {
        let mut f = fs::File::create(&examples_path)?;
        writeln!(f, "# Examples for {}\n\n## Example 1\nInput: ...\nOutput: ...", skill_name)?;
    }

    Ok(())
}

/// Generate agent skill files from PortableSkillDef
fn generate_agent_skill_files_from_def(skill: &PortableSkillDef) -> Result<()> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let skill_name = &skill.name;
    let skill_dir = Path::new(".agent/skills").join(skill_name);
    let resources_dir = skill_dir.join("resources");

    // Create directories
    fs::create_dir_all(&resources_dir).context("Failed to create skill directory")?;

    // Load template
    let template_path = Path::new(".agent/skills/skill-creator/resources/skill-template.md");
    let template = if template_path.exists() {
        fs::read_to_string(template_path).context("Failed to read skill template")?
    } else {
        // Fallback minimal template
        format!(
            "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntriggers:\n{}\n---\n\n# {}\n\n{}\n",
            skill_name,
            skill.description,
            skill.trigger.keywords.iter().map(|k| format!("  - \"{}\"", k)).collect::<Vec<_>>().join("\n"),
            skill_name,
            skill.description
        )
    };

    // Replace placeholders
    let content = template
        .replace("{{skill_name}}", skill_name)
        .replace("{{short_description}}", &skill.description)
        .replace("{{Skill Name}}", skill_name)
        .replace("{{Detailed description of the skill's purpose and context.}}", &skill.description)
        .replace("{{trigger_keyword_1}}", skill.trigger.keywords.first().map(|s| s.as_str()).unwrap_or("trigger"))
        .replace("{{trigger_keyword_2}}", skill.trigger.keywords.get(1).map(|s| s.as_str()).unwrap_or(""))
        .replace("{{Primary responsibility 1}}", "Auto-generated implementation step")
        .replace("{{Primary responsibility 2}}", "Protocol verification")
        .replace("{{Rule 1: Most critical constraint or behavior}}", "Follow standard Cratos conventions.")
        .replace("{{Step Name}}", "Execute Logic")
        .replace("{{Description of step 1 actions.}}", "Run the tool sequence defined in the generated skill logic.");
        
    // Write SKILL.md
    let skill_md_path = skill_dir.join("SKILL.md");
    let mut file = fs::File::create(&skill_md_path).context("Failed to create SKILL.md")?;
    file.write_all(content.as_bytes())?;

    println!("  Generated agent skill file: {}", skill_md_path.display());
    
    // Create execution-protocol.md if it doesn't exist
    let protocol_path = resources_dir.join("execution-protocol.md");
    if !protocol_path.exists() {
        let mut f = fs::File::create(&protocol_path)?;
        writeln!(f, "# Execution Protocol for {}\n\n1. Analyze input.\n2. Execute step 1.\n3. Verify result.", skill_name)?;
    }
    
    // Create examples.md if it doesn't exist
    let examples_path = resources_dir.join("examples.md");
    if !examples_path.exists() {
        let mut f = fs::File::create(&examples_path)?;
        writeln!(f, "# Examples for {}\n\n## Example 1\nInput: ...\nOutput: ...", skill_name)?;
    }

    Ok(())
}
