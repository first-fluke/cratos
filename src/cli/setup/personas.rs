//! Persona definitions for setup wizard.

use super::i18n::Language;

pub struct Persona {
    pub name: &'static str,
    pub display_en: &'static str,
    pub display_ko: &'static str,
    pub domain: &'static str,
    pub desc_en: &'static str,
    pub desc_ko: &'static str,
}

impl Persona {
    pub fn display(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.display_en,
            Language::Korean => self.display_ko,
        }
    }

    pub fn description(&self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.desc_en,
            Language::Korean => self.desc_ko,
        }
    }
}

pub const PERSONAS: &[Persona] = &[
    Persona {
        name: "cratos",
        display_en: "Cratos (Orchestrator)",
        display_ko: "Cratos (오케스트레이터)",
        domain: "ALL",
        desc_en: "Supreme orchestrator - routes tasks to specialized personas",
        desc_ko: "최상위 지휘관 - 전문 페르소나에게 작업 분배",
    },
    Persona {
        name: "sindri",
        display_en: "Sindri (Developer)",
        display_ko: "Sindri (개발자)",
        domain: "DEV",
        desc_en: "Master craftsman - coding, implementation, debugging",
        desc_ko: "장인 - 코딩, 구현, 디버깅",
    },
    Persona {
        name: "athena",
        display_en: "Athena (Project Manager)",
        display_ko: "Athena (프로젝트 매니저)",
        domain: "PM",
        desc_en: "Strategic wisdom - planning, architecture, design",
        desc_ko: "전략적 지혜 - 기획, 아키텍처, 설계",
    },
    Persona {
        name: "heimdall",
        display_en: "Heimdall (QA)",
        display_ko: "Heimdall (품질관리)",
        domain: "QA",
        desc_en: "All-seeing guardian - testing, security, quality",
        desc_ko: "만물의 감시자 - 테스트, 보안, 품질",
    },
    Persona {
        name: "mimir",
        display_en: "Mimir (Researcher)",
        display_ko: "Mimir (연구자)",
        domain: "RESEARCH",
        desc_en: "Knowledge keeper - research, documentation, analysis",
        desc_ko: "지식의 수호자 - 리서치, 문서화, 분석",
    },
];
