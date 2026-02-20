pub mod router;
#[cfg(feature = "semantic")]
pub mod semantic;

pub use router::{MatchReason, RouterConfig, RoutingResult, SkillRouter};

#[cfg(feature = "semantic")]
pub use semantic::{
    create_skill_index, SemanticMatchReason, SemanticRouterConfig, SemanticRoutingResult,
    SemanticSkillRouter, SkillEmbedder,
};
