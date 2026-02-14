//! Pantheon API - Persona management endpoints
//!
//! Provides REST endpoints for querying personas and their chronicles.

use std::sync::Arc;

use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Extension, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use cratos_core::chronicles::{Chronicle, ChronicleStore};
use cratos_skills::PersonaSkillStore;

/// Persona summary for list view
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonaSummary {
    pub name: String,
    pub level: u8,
    pub status: String,
    pub role: String,
    pub domain: String,
    pub rating: Option<f32>,
    pub objectives_count: usize,
    pub quests_completed: usize,
    pub quests_total: usize,
    pub skill_count: usize,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

impl<T> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data,
        })
    }
}

/// Persona metadata from TOML files
#[derive(Debug, Clone, Serialize)]
pub struct PersonaInfo {
    pub name: String,
    pub role: String,
    pub domain: String,
    pub level: u8,
}

/// Core personas from config/pantheon/
fn get_core_personas() -> Vec<PersonaInfo> {
    vec![
        PersonaInfo {
            name: "cratos".to_string(),
            role: "Orchestrator".to_string(),
            domain: "전체 통솔".to_string(),
            level: 255,
        },
        PersonaInfo {
            name: "athena".to_string(),
            role: "PM".to_string(),
            domain: "전략, 기획".to_string(),
            level: 3,
        },
        PersonaInfo {
            name: "sindri".to_string(),
            role: "DEV".to_string(),
            domain: "개발, 구현".to_string(),
            level: 1,
        },
        PersonaInfo {
            name: "heimdall".to_string(),
            role: "QA".to_string(),
            domain: "품질, 보안".to_string(),
            level: 2,
        },
        PersonaInfo {
            name: "mimir".to_string(),
            role: "Researcher".to_string(),
            domain: "리서치".to_string(),
            level: 4,
        },
        PersonaInfo {
            name: "odin".to_string(),
            role: "PO".to_string(),
            domain: "프로덕트 오너".to_string(),
            level: 5,
        },
        PersonaInfo {
            name: "hestia".to_string(),
            role: "HR".to_string(),
            domain: "인사, 조직 관리".to_string(),
            level: 2,
        },
        PersonaInfo {
            name: "norns".to_string(),
            role: "BA".to_string(),
            domain: "비즈니스 분석".to_string(),
            level: 3,
        },
        PersonaInfo {
            name: "apollo".to_string(),
            role: "UX".to_string(),
            domain: "UX 디자인".to_string(),
            level: 3,
        },
        PersonaInfo {
            name: "freya".to_string(),
            role: "CS".to_string(),
            domain: "고객 지원".to_string(),
            level: 2,
        },
        PersonaInfo {
            name: "tyr".to_string(),
            role: "Legal".to_string(),
            domain: "법무, 규정".to_string(),
            level: 4,
        },
        PersonaInfo {
            name: "nike".to_string(),
            role: "Marketing".to_string(),
            domain: "마케팅".to_string(),
            level: 2,
        },
        PersonaInfo {
            name: "thor".to_string(),
            role: "DevOps".to_string(),
            domain: "인프라, 운영".to_string(),
            level: 3,
        },
        PersonaInfo {
            name: "brok".to_string(),
            role: "DEV".to_string(),
            domain: "개발".to_string(),
            level: 1,
        },
    ]
}

/// List all personas with their chronicles
#[utoipa::path(
    get,
    path = "/api/v1/pantheon",
    tag = "pantheon",
    responses(
        (status = 200, description = "List of personas", body = Vec<PersonaSummary>)
    )
)]
pub async fn list_personas(
    Extension(persona_skill_store): Extension<Arc<PersonaSkillStore>>,
) -> impl IntoResponse {
    let personas = get_core_personas();
    let chronicle_store = ChronicleStore::new();
    let mut summaries = Vec::with_capacity(personas.len());

    for info in personas {
        // Load chronicle for this persona
        let chronicle = chronicle_store.load(&info.name).ok().flatten();

        // Get skill count from PersonaSkillStore
        let skill_count = persona_skill_store
            .get_persona_skills(&info.name)
            .await
            .map(|skills| skills.len())
            .unwrap_or(0);

        let (rating, quests_completed, quests_total, objectives_count, status) =
            extract_chronicle_data(&chronicle, &info);

        summaries.push(PersonaSummary {
            name: info.name,
            level: chronicle.as_ref().map(|c| c.level).unwrap_or(info.level),
            status,
            role: info.role,
            domain: info.domain,
            rating,
            objectives_count,
            quests_completed,
            quests_total,
            skill_count,
        });
    }

    ApiResponse::ok(summaries)
}

/// Extract data from chronicle for persona summary
fn extract_chronicle_data(
    chronicle: &Option<Chronicle>,
    _info: &PersonaInfo,
) -> (Option<f32>, usize, usize, usize, String) {
    match chronicle {
        Some(c) => (
            c.rating,
            c.completed_quests(),
            c.quests.len(),
            c.objectives.len(),
            format!("{:?}", c.status),
        ),
        None => (None, 0, 0, 0, "Active".to_string()),
    }
}

/// Get single persona details with full chronicle
#[utoipa::path(
    get,
    path = "/api/v1/pantheon/{name}",
    tag = "pantheon",
    params(
        ("name" = String, Path, description = "Persona name")
    ),
    responses(
        (status = 200, description = "Persona details with chronicle"),
        (status = 404, description = "Persona not found")
    )
)]
pub async fn get_persona(
    Extension(persona_skill_store): Extension<Arc<PersonaSkillStore>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let personas = get_core_personas();

    // Find persona info
    let info = match personas.iter().find(|p| p.name.to_lowercase() == name.to_lowercase()) {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("Persona '{}' not found", name)
                })),
            )
                .into_response();
        }
    };

    // Load chronicle
    let chronicle_store = ChronicleStore::new();
    let chronicle = chronicle_store.load(&name).ok().flatten();

    // Get skills for this persona
    let skills = persona_skill_store
        .get_persona_skills(&name)
        .await
        .unwrap_or_default();

    let skill_names: Vec<String> = skills.iter().map(|s| s.skill_name.clone()).collect();

    let response = serde_json::json!({
        "success": true,
        "data": {
            "persona": info,
            "chronicle": chronicle,
            "skills": skill_names,
        }
    });

    Json(response).into_response()
}

/// Create pantheon routes
pub fn pantheon_routes() -> Router {
    Router::new()
        .route("/api/v1/pantheon", get(list_personas))
        .route("/api/v1/pantheon/:name", get(get_persona))
}
