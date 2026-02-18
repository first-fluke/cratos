//! Skill initialization functions
//!
//! Contains functions for initializing default skills from persona TOML files.

use cratos_skills::{PersonaSkillStore, Skill, SkillCategory, SkillOrigin, SkillStatus, SkillStore, SkillTrigger};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Initialize default skills from persona TOML files
pub async fn init_default_skills(
    persona_skill_store: &Arc<PersonaSkillStore>,
    skill_store: &Arc<SkillStore>,
) {
    let persona_loader = cratos_core::pantheon::PersonaLoader::new();
    let persona_mapping = cratos_core::PersonaMapping::from_loader(&persona_loader);
    let mut default_skills_registered = 0usize;
    let mut skills_created = 0usize;

    for persona_name in persona_mapping.persona_names() {
        if let Some(preset) = persona_mapping.get_preset(persona_name) {
            for skill_name in &preset.skills.default {
                // Skip if already bound
                match persona_skill_store
                    .has_skill_by_name(persona_name, skill_name)
                    .await
                {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) => {
                        warn!(
                            persona = %persona_name,
                            skill = %skill_name,
                            error = %e,
                            "Failed to check skill binding"
                        );
                        continue;
                    }
                }

                // Find skill by name, or create if it doesn't exist
                let skill = match skill_store.get_skill_by_name(skill_name).await {
                    Ok(Some(s)) => s,
                    Ok(None) => {
                        // Create the skill automatically
                        let description = generate_skill_description(skill_name, persona_name);
                        let mut new_skill =
                            Skill::new(skill_name, &description, SkillCategory::System);
                        new_skill.origin = SkillOrigin::Builtin;
                        new_skill.status = SkillStatus::Active;
                        new_skill.trigger =
                            SkillTrigger::with_keywords(vec![skill_name.to_string()]);

                        if let Err(e) = skill_store.save_skill(&new_skill).await {
                            warn!(
                                skill = %skill_name,
                                error = %e,
                                "Failed to create default skill"
                            );
                            continue;
                        }
                        skills_created += 1;
                        debug!(skill = %skill_name, "Created default skill");
                        new_skill
                    }
                    Err(e) => {
                        warn!(
                            skill = %skill_name,
                            error = %e,
                            "Failed to lookup skill"
                        );
                        continue;
                    }
                };

                // Create the binding
                if let Err(e) = persona_skill_store
                    .create_default_binding(persona_name, skill.id, skill_name)
                    .await
                {
                    warn!(
                        persona = %persona_name,
                        skill = %skill_name,
                        error = %e,
                        "Failed to create default skill binding"
                    );
                } else {
                    default_skills_registered += 1;
                }
            }
        }
    }

    if skills_created > 0 {
        info!(
            "Created {} default skills from persona TOML files",
            skills_created
        );
    }
    if default_skills_registered > 0 {
        info!(
            "Registered {} default persona-skill bindings from TOML",
            default_skills_registered
        );
    }
}

/// Generate a description for a default skill based on its name
pub fn generate_skill_description(skill_name: &str, persona_name: &str) -> String {
    match skill_name {
        // Cratos skills
        "delegation" => {
            "Delegate tasks to appropriate personas based on domain expertise".to_string()
        }
        "orchestration" => "Coordinate multi-persona tasks and synthesize results".to_string(),
        "judgment" => "Make final decisions when domains overlap or conflict".to_string(),

        // Athena skills
        "sprint_planning" => "Plan and organize sprint tasks with clear objectives".to_string(),
        "roadmap_gen" => "Generate project roadmaps with milestones and timelines".to_string(),
        "risk_analysis" => {
            "Identify and assess project risks with mitigation strategies".to_string()
        }

        // Sindri skills
        "api_builder" => "Design and implement REST/GraphQL API endpoints".to_string(),
        "db_schema" => "Design database schemas with proper indexes and constraints".to_string(),
        "auth_module" => "Implement authentication and authorization modules".to_string(),

        // Brok skills
        "rapid_prototyping" => "Quickly build prototypes to validate ideas".to_string(),
        "scripting" => "Write automation scripts and utilities".to_string(),
        "debugging" => "Debug and fix code issues efficiently".to_string(),

        // Heimdall skills
        "security_review" => "Review code for security vulnerabilities".to_string(),
        "test_coverage" => "Analyze and improve test coverage".to_string(),
        "bug_triage" => "Triage and prioritize bug reports".to_string(),

        // Mimir skills
        "research" => {
            "Research topics and synthesize information from multiple sources".to_string()
        }
        "analysis" => "Analyze data and provide insights".to_string(),
        "documentation" => "Create and maintain technical documentation".to_string(),

        // Thor skills
        "ci_cd" => "Configure and maintain CI/CD pipelines".to_string(),
        "container_orchestration" => {
            "Manage container orchestration with Kubernetes/Docker".to_string()
        }
        "monitoring" => "Set up monitoring and alerting systems".to_string(),
        "incident_response" => "Handle incidents and implement recovery procedures".to_string(),

        // Odin skills
        "roadmap_planning" => "Create strategic product roadmaps".to_string(),
        "stakeholder_management" => "Manage stakeholder expectations and communication".to_string(),
        "prioritization" => "Prioritize features and tasks based on impact".to_string(),
        "okr_tracking" => "Track and manage OKRs (Objectives and Key Results)".to_string(),

        // Hestia skills
        "team_management" => "Manage team dynamics and performance".to_string(),
        "conflict_resolution" => "Resolve team conflicts constructively".to_string(),
        "onboarding" => "Onboard new team members effectively".to_string(),
        "culture_building" => "Build and maintain team culture".to_string(),

        // Norns skills
        "requirements_analysis" => "Analyze and document business requirements".to_string(),
        "process_mapping" => "Map and optimize business processes".to_string(),
        "data_modeling" => "Design data models for business domains".to_string(),
        "gap_analysis" => "Identify gaps between current and desired states".to_string(),

        // Apollo skills
        "ui_design" => "Design user interfaces with accessibility in mind".to_string(),
        "ux_research" => "Conduct user experience research".to_string(),
        "prototyping" => "Create interactive prototypes".to_string(),
        "design_systems" => "Build and maintain design systems".to_string(),

        // Freya skills
        "issue_triage" => "Triage and categorize customer issues".to_string(),
        "user_communication" => "Communicate effectively with users".to_string(),
        "knowledge_base" => "Create and maintain knowledge base articles".to_string(),
        "escalation_management" => "Manage issue escalations appropriately".to_string(),

        // Tyr skills
        "license_review" => "Review software licenses for compliance".to_string(),
        "compliance_check" => "Verify compliance with regulations".to_string(),
        "privacy_audit" => "Audit privacy practices and policies".to_string(),
        "policy_drafting" => "Draft legal and compliance policies".to_string(),

        // Nike skills
        "content_strategy" => "Develop content marketing strategies".to_string(),
        "growth_marketing" => "Execute growth marketing initiatives".to_string(),
        "brand_management" => "Manage brand identity and messaging".to_string(),
        "analytics" => "Analyze marketing metrics and KPIs".to_string(),
        "sns_automation" => "Automate social media workflows".to_string(),

        // Default fallback
        _ => format!(
            "Default skill '{}' for {} persona",
            skill_name.replace('_', " "),
            persona_name
        ),
    }
}
