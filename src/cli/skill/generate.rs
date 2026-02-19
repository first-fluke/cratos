use super::open_store;
use anyhow::{Context, Result};
use cratos_replay::{default_db_path as default_replay_db_path, EventStore};
use cratos_skills::{
    create_skill_index, PatternAnalyzer, PatternStatus, SemanticSkillRouter, Skill, SkillEmbedder,
    SkillGenerator, SkillRegistry, SkillStatus, SkillStore,
};
use std::sync::Arc;

/// Analyze recent usage patterns
pub async fn analyze_patterns(dry_run: bool) -> Result<()> {
    let db_path = default_replay_db_path();
    let event_store = EventStore::from_path(&db_path).await?;
    let skill_store = open_store().await?;

    println!("Analyzing execution history for recurring patterns...");
    println!("Database: {}", db_path.display());

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

        println!("  Sequence: {:?}", p.tool_sequence);

        if !dry_run {
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
pub async fn generate_skills(store: &SkillStore, dry_run: bool, auto_enable: bool) -> Result<()> {
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

    let semantic_router = if !dry_run {
        println!("Initializing Semantic Search Engine...");
        match initialize_semantic_router(store).await {
            Ok(router) => {
                println!("  ✅ Semantic Search initialized.");
                Some(router)
            }
            Err(e) => {
                println!(
                    "  ⚠️ Semantic Search unavailable: {}. Falling back to exact match.",
                    e
                );
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

        match generator.generate_from_pattern(&pattern) {
            Ok(mut skill) => {
                println!("  Generated Skill: '{}'", skill.name);
                println!("  Description: {:?}", skill.description);

                if auto_enable {
                    skill.status = SkillStatus::Active;
                    println!("  Status: Active (Auto-enabled)");
                }

                if !dry_run {
                    let mut existing_skill = None;

                    match store.get_skill_by_name(&skill.name).await {
                        Ok(Some(s)) => {
                            println!("  ℹ️ Found exact match: '{}'", s.name);
                            existing_skill = Some(s);
                        }
                        Err(e) => println!("  ⚠️ Error checking exact match: {}", e),
                        _ => {}
                    }

                    if existing_skill.is_none() {
                        if let Some(router) = &semantic_router {
                            match router.route(&skill.name).await {
                                Ok(matches) => {
                                    if let Some(best) = matches.first() {
                                        if best.score > 0.85 {
                                            println!(
                                                "  ℹ️ Found semantic match: '{}' (Score: {:.2})",
                                                best.skill.name, best.score
                                            );
                                            existing_skill = Some((*best.skill).clone());
                                        }
                                    }
                                }
                                Err(e) => println!("  ⚠️ Semantic search failed: {}", e),
                            }
                        }
                    }

                    match existing_skill {
                        Some(mut existing) => {
                            println!("  Using existing skill: '{}' for merge.", existing.name);

                            let merged = merge_skill_content(&mut existing, &skill);

                            if merged {
                                match store.save_skill(&existing).await {
                                    Ok(_) => {
                                        println!("  ✅ Merged new triggers into existing skill.");
                                        if let Err(e) = generate_agent_skill_files(&existing) {
                                            println!(
                                                "  ⚠️ Failed to update agent skill files: {}",
                                                e
                                            );
                                        }
                                    }
                                    Err(e) => println!("  ❌ Failed to save merged skill: {}", e),
                                }
                            } else {
                                println!("  ℹ️ No new content to merge.");
                            }

                            if let Err(e) =
                                store.mark_pattern_converted(pattern.id, existing.id).await
                            {
                                println!("  ⚠️ Failed to mark pattern converted: {}", e);
                            }
                        }
                        None => match store.save_skill(&skill).await {
                            Ok(()) => {
                                if let Err(e) =
                                    store.mark_pattern_converted(pattern.id, skill.id).await
                                {
                                    println!("  ⚠️ Failed to mark pattern converted: {}", e);
                                } else {
                                    generated_count += 1;
                                    println!("  ✅ Saved and linked to pattern.");

                                    if let Err(e) = generate_agent_skill_files(&skill) {
                                        println!(
                                            "  ⚠️ Failed to generate agent skill files: {}",
                                            e
                                        );
                                    }

                                    if let Some(router) = &semantic_router {
                                        if let Err(e) = router.index_skill(&skill).await {
                                            println!("  ⚠️ Failed to index new skill: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => println!("  ❌ Failed to save skill: {}", e),
                        },
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

async fn initialize_semantic_router(
    store: &SkillStore,
) -> Result<SemanticSkillRouter<CliSkillEmbeddingAdapter>> {
    let _config = crate::server::load_config()?;

    let provider =
        cratos_llm::default_embedding_provider().context("Failed to create embedding provider")?;

    let embedder = Arc::new(CliSkillEmbeddingAdapter { provider });

    let index_path = cratos_skills::default_skill_db_path().with_extension("index");
    let index = create_skill_index(embedder.dimensions(), Some(&index_path))?;

    let registry = Arc::new(SkillRegistry::new());
    let active_skills = store.list_active_skills().await?;
    registry.load_all(active_skills).await?;

    let router = SemanticSkillRouter::new(registry, index, embedder);

    router.reindex_all().await?;

    Ok(router)
}

struct CliSkillEmbeddingAdapter {
    provider: std::sync::Arc<dyn cratos_llm::EmbeddingProvider>,
}

#[async_trait::async_trait]
impl cratos_skills::SkillEmbedder for CliSkillEmbeddingAdapter {
    async fn embed(&self, text: &str) -> cratos_skills::Result<Vec<f32>> {
        self.provider
            .embed(text)
            .await
            .map_err(|e| cratos_skills::Error::Internal(e.to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> cratos_skills::Result<Vec<Vec<f32>>> {
        self.provider
            .embed_batch(texts)
            .await
            .map_err(|e| cratos_skills::Error::Internal(e.to_string()))
    }

    fn dimensions(&self) -> usize {
        self.provider.dimensions()
    }
}

fn merge_skill_content(existing: &mut Skill, new_skill: &Skill) -> bool {
    let mut merged = false;
    for k in &new_skill.trigger.keywords {
        if !existing.trigger.keywords.contains(k) {
            existing.trigger.keywords.push(k.clone());
            merged = true;
        }
    }
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

fn generate_agent_skill_files(skill: &cratos_skills::Skill) -> Result<()> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let skill_name = &skill.name;
    let skill_dir = Path::new(".cratos/skills").join(skill_name);
    let resources_dir = skill_dir.join("resources");

    fs::create_dir_all(&resources_dir).context("Failed to create skill directory")?;

    let template_path = Path::new(".cratos/skills/skill-creator/resources/skill-template.md");
    let template = if template_path.exists() {
        fs::read_to_string(template_path).context("Failed to read skill template")?
    } else {
        format!(
            "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntriggers:\n{}\n---\n\n# {}\n\n{}\n",
            skill_name,
            skill.description,
            skill
                .trigger
                .keywords
                .iter()
                .map(|k| format!("  - \"{}\"", k))
                .collect::<Vec<_>>()
                .join("\n"),
            skill_name,
            skill.description
        )
    };

    let content = template
        .replace("{{skill_name}}", skill_name)
        .replace("{{short_description}}", &skill.description)
        .replace("{{Skill Name}}", skill_name)
        .replace(
            "{{Detailed description of the skill's purpose and context.}}",
            &skill.description,
        )
        .replace(
            "{{trigger_keyword_1}}",
            skill
                .trigger
                .keywords
                .first()
                .map(|s| s.as_str())
                .unwrap_or("trigger"),
        )
        .replace(
            "{{trigger_keyword_2}}",
            skill
                .trigger
                .keywords
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or(""),
        )
        .replace(
            "{{Primary responsibility 1}}",
            "Auto-generated implementation step",
        )
        .replace("{{Primary responsibility 2}}", "Protocol verification")
        .replace(
            "{{Rule 1: Most critical constraint or behavior}}",
            "Follow standard Cratos conventions.",
        )
        .replace("{{Step Name}}", "Execute Logic")
        .replace(
            "{{Description of step 1 actions.}}",
            "Run the tool sequence defined in the generated skill logic.",
        );

    let skill_md_path = skill_dir.join("SKILL.md");

    if skill_md_path.exists() {
        println!("  ℹ️ updating existing SKILL.md...");

        let existing_content =
            fs::read_to_string(&skill_md_path).context("Failed to read existing SKILL.md")?;

        let parts: Vec<&str> = existing_content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            let frontmatter = parts[1];
            let body = parts[2];

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

    let mut file = fs::File::create(&skill_md_path).context("Failed to create SKILL.md")?;
    file.write_all(content.as_bytes())?;

    println!("  Generated agent skill file: {}", skill_md_path.display());

    let protocol_path = resources_dir.join("execution-protocol.md");
    if !protocol_path.exists() {
        let mut f = fs::File::create(&protocol_path)?;
        writeln!(f, "# Execution Protocol for {}\n\n1. Analyze input.\n2. Execute step 1.\n3. Verify result.", skill_name)?;
    }

    let examples_path = resources_dir.join("examples.md");
    if !examples_path.exists() {
        let mut f = fs::File::create(&examples_path)?;
        writeln!(
            f,
            "# Examples for {}\n\n## Example 1\nInput: ...\nOutput: ...",
            skill_name
        )?;
    }

    Ok(())
}

pub(crate) fn generate_agent_skill_files_from_def(
    skill: &cratos_skills::ecosystem::PortableSkillDef,
) -> Result<()> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let skill_name = &skill.name;
    let skill_dir = Path::new(".cratos/skills").join(skill_name);
    let resources_dir = skill_dir.join("resources");

    fs::create_dir_all(&resources_dir).context("Failed to create skill directory")?;

    let template_path = Path::new(".cratos/skills/skill-creator/resources/skill-template.md");
    let template = if template_path.exists() {
        fs::read_to_string(template_path).context("Failed to read skill template")?
    } else {
        format!(
            "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntriggers:\n{}\n---\n\n# {}\n\n{}\n",
            skill_name,
            skill.description,
            skill
                .trigger
                .keywords
                .iter()
                .map(|k| format!("  - \"{}\"", k))
                .collect::<Vec<_>>()
                .join("\n"),
            skill_name,
            skill.description
        )
    };

    let content = template
        .replace("{{skill_name}}", skill_name)
        .replace("{{short_description}}", &skill.description)
        .replace("{{Skill Name}}", skill_name)
        .replace(
            "{{Detailed description of the skill's purpose and context.}}",
            &skill.description,
        )
        .replace(
            "{{trigger_keyword_1}}",
            skill
                .trigger
                .keywords
                .first()
                .map(|s| s.as_str())
                .unwrap_or("trigger"),
        )
        .replace(
            "{{trigger_keyword_2}}",
            skill
                .trigger
                .keywords
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or(""),
        )
        .replace(
            "{{Primary responsibility 1}}",
            "Auto-generated implementation step",
        )
        .replace("{{Primary responsibility 2}}", "Protocol verification")
        .replace(
            "{{Rule 1: Most critical constraint or behavior}}",
            "Follow standard Cratos conventions.",
        )
        .replace("{{Step Name}}", "Execute Logic")
        .replace(
            "{{Description of step 1 actions.}}",
            "Run the tool sequence defined in the generated skill logic.",
        );

    let skill_md_path = skill_dir.join("SKILL.md");
    let mut file = fs::File::create(&skill_md_path).context("Failed to create SKILL.md")?;
    file.write_all(content.as_bytes())?;

    println!("  Generated agent skill file: {}", skill_md_path.display());

    let protocol_path = resources_dir.join("execution-protocol.md");
    if !protocol_path.exists() {
        let mut f = fs::File::create(&protocol_path)?;
        writeln!(f, "# Execution Protocol for {}\n\n1. Analyze input.\n2. Execute step 1.\n3. Verify result.", skill_name)?;
    }

    let examples_path = resources_dir.join("examples.md");
    if !examples_path.exists() {
        let mut f = fs::File::create(&examples_path)?;
        writeln!(
            f,
            "# Examples for {}\n\n## Example 1\nInput: ...\nOutput: ...",
            skill_name
        )?;
    }

    Ok(())
}
