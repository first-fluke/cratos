//! Skill management REST API
//!
//! Provides CRUD operations for skills and marketplace integration.

use axum::{
    extract::{Extension, Path, Query},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use cratos_skills::{
    ExportFormat, PortableSkill, RegistryEntry, RemoteRegistry, Skill, SkillEcosystem,
    SkillStatus, SkillStore,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info};
use utoipa::ToSchema;
use uuid::Uuid;

/// Skill list query parameters
#[derive(Debug, Deserialize)]
pub struct SkillListQuery {
    /// Filter by category
    pub category: Option<String>,
    /// Only show active skills
    pub active_only: Option<bool>,
    /// Limit number of results
    pub limit: Option<usize>,
}

/// Skill info response
#[derive(Debug, Serialize, ToSchema)]
pub struct SkillInfo {
    /// Skill ID
    pub id: String,
    /// Skill name
    pub name: String,
    /// Description
    pub description: String,
    /// Category
    pub category: String,
    /// Status
    pub status: String,
    /// Origin
    pub origin: String,
    /// Creation timestamp
    pub created_at: String,
}

impl From<Skill> for SkillInfo {
    fn from(s: Skill) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            category: s.category.as_str().to_string(),
            status: format!("{:?}", s.status),
            origin: format!("{:?}", s.origin),
            created_at: s.created_at.to_rfc3339(),
        }
    }
}

/// Export request query
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    /// Format: "json" or "yaml" (default)
    pub format: Option<String>,
}

/// Bundle export request body
#[derive(Debug, Deserialize)]
pub struct BundleExportRequest {
    /// Bundle name
    pub name: String,
    /// Bundle description
    pub description: String,
    /// Skill IDs to include
    pub skill_ids: Vec<String>,
}

/// Registry search query
#[derive(Debug, Deserialize)]
pub struct RegistrySearchQuery {
    /// Search query
    pub query: String,
    /// Max results
    pub limit: Option<usize>,
}

/// Registry install request
#[derive(Debug, Deserialize)]
pub struct RegistryInstallRequest {
    /// Skill name to install
    pub name: String,
}

/// Create skills routes
pub fn skills_routes() -> Router {
    Router::new()
        // List and CRUD
        .route("/api/v1/skills", get(list_skills))
        .route("/api/v1/skills/:id", get(get_skill).delete(delete_skill))
        .route("/api/v1/skills/:id/enable", post(enable_skill))
        .route("/api/v1/skills/:id/disable", post(disable_skill))
        // Export/Import
        .route("/api/v1/skills/:id/export", get(export_skill))
        .route("/api/v1/skills/export/bundle", post(export_bundle))
        .route("/api/v1/skills/import", post(import_skill))
        // Remote registry
        .route("/api/v1/registry/search", get(registry_search))
        .route("/api/v1/registry/install", post(registry_install))
}

/// List all skills
#[utoipa::path(
    get,
    path = "/api/v1/skills",
    tag = "skills",
    responses(
        (status = 200, description = "List of skills", body = Vec<SkillInfo>),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_skills(
    Query(query): Query<SkillListQuery>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let skills = match store.list_skills().await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "Failed to list skills");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    let mut infos: Vec<SkillInfo> = skills
        .into_iter()
        .filter(|s| {
            // Filter by category if specified
            if let Some(ref cat) = query.category {
                if s.category.as_str() != cat {
                    return false;
                }
            }
            // Filter by active status if specified
            if query.active_only.unwrap_or(false) && !s.is_active() {
                return false;
            }
            true
        })
        .map(SkillInfo::from)
        .collect();

    // Apply limit
    if let Some(limit) = query.limit {
        infos.truncate(limit);
    }

    Json(infos).into_response()
}

/// Get single skill details
#[utoipa::path(
    get,
    path = "/api/v1/skills/{id}",
    tag = "skills",
    params(
        ("id" = String, Path, description = "Skill UUID")
    ),
    responses(
        (status = 200, description = "Skill details", body = SkillInfo),
        (status = 400, description = "Invalid UUID"),
        (status = 404, description = "Skill not found")
    )
)]
pub async fn get_skill(
    Path(id): Path<String>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid UUID").into_response(),
    };

    match store.get_skill(uuid).await {
        Ok(skill) => Json(SkillInfo::from(skill)).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Skill not found").into_response(),
    }
}

/// Delete a skill
async fn delete_skill(
    Path(id): Path<String>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match store.delete_skill(uuid).await {
        Ok(_) => {
            info!(skill_id = %id, "Skill deleted");
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::NOT_FOUND,
    }
}

/// Enable a skill
async fn enable_skill(
    Path(id): Path<String>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match store.get_skill(uuid).await {
        Ok(mut skill) => {
            skill.activate();
            if store.save_skill(&skill).await.is_ok() {
                info!(skill_id = %id, "Skill enabled");
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        Err(_) => StatusCode::NOT_FOUND,
    }
}

/// Disable a skill
async fn disable_skill(
    Path(id): Path<String>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match store.get_skill(uuid).await {
        Ok(mut skill) => {
            skill.status = SkillStatus::Disabled;
            if store.save_skill(&skill).await.is_ok() {
                info!(skill_id = %id, "Skill disabled");
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        Err(_) => StatusCode::NOT_FOUND,
    }
}

/// Export a skill
async fn export_skill(
    Path(id): Path<String>,
    Query(query): Query<ExportQuery>,
    Extension(store): Extension<Arc<SkillStore>>,
) -> impl IntoResponse {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid UUID").into_response(),
    };

    let ecosystem = SkillEcosystem::new((*store).clone());

    let format = match query.format.as_deref() {
        Some("json") => ExportFormat::JsonPretty,
        _ => ExportFormat::Yaml,
    };

    match ecosystem.export_skill(uuid).await {
        Ok(portable) => {
            let (content, content_type) = match format {
                ExportFormat::Yaml => (
                    serde_yaml::to_string(&portable).unwrap_or_default(),
                    "application/x-yaml",
                ),
                _ => (
                    serde_json::to_string_pretty(&portable).unwrap_or_default(),
                    "application/json",
                ),
            };

            (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], content).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to export skill");
            (StatusCode::NOT_FOUND, "Skill not found").into_response()
        }
    }
}

/// Export multiple skills as bundle
async fn export_bundle(
    Extension(store): Extension<Arc<SkillStore>>,
    Json(req): Json<BundleExportRequest>,
) -> impl IntoResponse {
    let skill_ids: Vec<Uuid> = req
        .skill_ids
        .iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();

    let ecosystem = SkillEcosystem::new((*store).clone());

    match ecosystem
        .export_skills_as_bundle(&req.name, &req.description, &skill_ids)
        .await
    {
        Ok(bundle) => {
            let yaml = serde_yaml::to_string(&bundle).unwrap_or_default();
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/x-yaml")], yaml).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to export bundle");
            (StatusCode::INTERNAL_SERVER_ERROR, "Export failed").into_response()
        }
    }
}

/// Import a skill from JSON/YAML
async fn import_skill(
    Extension(store): Extension<Arc<SkillStore>>,
    body: String,
) -> impl IntoResponse {
    // Try YAML first, then JSON
    let portable: PortableSkill = match serde_yaml::from_str(&body) {
        Ok(p) => p,
        Err(_) => match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("Invalid format: {e}")).into_response();
            }
        },
    };

    let ecosystem = SkillEcosystem::new((*store).clone());

    match ecosystem.import_skill(&portable).await {
        Ok(result) => {
            info!(
                skill = %result.skill.name,
                is_new = result.is_new,
                "Skill imported"
            );
            Json(serde_json::json!({
                "success": true,
                "skill_id": result.skill.id.to_string(),
                "is_new": result.is_new,
                "warnings": result.warnings,
            }))
            .into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to import skill");
            (StatusCode::BAD_REQUEST, format!("Import failed: {e}")).into_response()
        }
    }
}

/// Search remote registry
async fn registry_search(Query(q): Query<RegistrySearchQuery>) -> impl IntoResponse {
    let registry = RemoteRegistry::default_registry();

    match registry.search(&q.query).await {
        Ok(entries) => {
            let limited: Vec<RegistryEntry> =
                entries.into_iter().take(q.limit.unwrap_or(20)).collect();
            debug!(count = limited.len(), "Registry search results");
            Json(limited).into_response()
        }
        Err(e) => {
            error!(error = %e, "Registry search failed");
            (StatusCode::SERVICE_UNAVAILABLE, "Registry unavailable").into_response()
        }
    }
}

/// Install skill from remote registry
async fn registry_install(
    Extension(store): Extension<Arc<SkillStore>>,
    Json(req): Json<RegistryInstallRequest>,
) -> impl IntoResponse {
    let registry = RemoteRegistry::default_registry();

    // Fetch skill from registry
    let portable = match registry.fetch_skill(&req.name).await {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::NOT_FOUND, format!("Skill not found: {e}")).into_response();
        }
    };

    // Import into local store
    let ecosystem = SkillEcosystem::new((*store).clone());

    match ecosystem.import_skill(&portable).await {
        Ok(result) => {
            info!(skill = %req.name, "Installed from registry");
            Json(serde_json::json!({
                "success": true,
                "skill_id": result.skill.id.to_string(),
                "is_new": result.is_new,
            }))
            .into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Install failed: {e}")).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_info_serialize() {
        let info = SkillInfo {
            id: "test-id".to_string(),
            name: "test".to_string(),
            description: "desc".to_string(),
            category: "workflow".to_string(),
            status: "Active".to_string(),
            origin: "UserDefined".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-id"));
    }
}
