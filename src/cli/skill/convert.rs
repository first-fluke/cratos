use super::format_duration_since;
use anyhow::{Context, Result};
use cratos_skills::{RemoteRegistry, SkillEcosystem, SkillStore};

/// Export a skill to a JSON file or Agent Markdown
pub async fn export_skill(
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
        super::generate::generate_agent_skill_files_from_def(&portable.skill)?;
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
pub async fn import_skill(store: &SkillStore, path: &str) -> Result<()> {
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
pub async fn export_bundle(store: &SkillStore, name: &str, output: Option<String>) -> Result<()> {
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

/// Search remote skill registry
pub async fn search_remote(query: &str, registry: Option<String>) -> Result<()> {
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
pub async fn install_remote(
    store: &SkillStore,
    name: &str,
    registry: Option<String>,
) -> Result<()> {
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
pub async fn publish_remote(
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
pub async fn prune(
    store: &SkillStore,
    older_than: u32,
    dry_run: bool,
    confirm: bool,
) -> Result<()> {
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
