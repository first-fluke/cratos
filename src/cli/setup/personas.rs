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
    Persona {
        name: "odin",
        display_en: "Odin (Product Owner)",
        display_ko: "Odin (프로덕트 오너)",
        domain: "PO",
        desc_en: "The Allfather - product vision, roadmaps, prioritization",
        desc_ko: "만물의 아버지 - 제품 비전, 로드맵, 우선순위",
    },
    Persona {
        name: "hestia",
        display_en: "Hestia (HR)",
        display_ko: "Hestia (인사)",
        domain: "HR",
        desc_en: "Guardian of the Hearth - team management, culture",
        desc_ko: "화로의 수호자 - 팀 관리, 문화",
    },
    Persona {
        name: "norns",
        display_en: "Norns (Business Analyst)",
        display_ko: "Norns (비즈니스 분석가)",
        domain: "BA",
        desc_en: "Weavers of Fate - requirements, process mapping",
        desc_ko: "운명의 직공 - 요구사항, 프로세스 매핑",
    },
    Persona {
        name: "apollo",
        display_en: "Apollo (UX Designer)",
        display_ko: "Apollo (UX 디자이너)",
        domain: "UX",
        desc_en: "God of Light - UI/UX design, prototyping, accessibility",
        desc_ko: "빛의 신 - UI/UX 설계, 프로토타이핑, 접근성",
    },
    Persona {
        name: "freya",
        display_en: "Freya (Customer Support)",
        display_ko: "Freya (고객 지원)",
        domain: "CS",
        desc_en: "Goddess of Love - user advocacy, issue resolution",
        desc_ko: "사랑의 여신 - 사용자 대변, 이슈 해결",
    },
    Persona {
        name: "tyr",
        display_en: "Tyr (Legal)",
        display_ko: "Tyr (법무)",
        domain: "LEGAL",
        desc_en: "God of Justice - compliance, licensing, privacy",
        desc_ko: "정의의 신 - 컴플라이언스, 라이선스, 개인정보",
    },
    Persona {
        name: "nike",
        display_en: "Nike (Marketing)",
        display_ko: "Nike (마케팅)",
        domain: "MARKETING",
        desc_en: "Goddess of Victory - growth, content, brand",
        desc_ko: "승리의 여신 - 성장, 콘텐츠, 브랜드",
    },
    Persona {
        name: "thor",
        display_en: "Thor (DevOps)",
        display_ko: "Thor (데브옵스)",
        domain: "DEVOPS",
        desc_en: "God of Thunder - CI/CD, infrastructure, monitoring",
        desc_ko: "천둥의 신 - CI/CD, 인프라, 모니터링",
    },
    Persona {
        name: "brok",
        display_en: "Brok (Developer)",
        display_ko: "Brok (개발자)",
        domain: "DEV",
        desc_en: "The Blue Dwarf - rapid prototyping, pragmatic building",
        desc_ko: "푸른 난쟁이 - 빠른 프로토타이핑, 실용적 개발",
    },
];
