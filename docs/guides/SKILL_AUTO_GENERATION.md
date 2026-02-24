# Skill Auto-generation - 자동 스킬 생성 시스템

## 개요

Cratos의 **자동 스킬 생성 시스템**은 사용자의 도구 사용 패턴을 학습하여 반복적인 작업을 자동화된 스킬로 변환합니다. 이것은 Cratos의 핵심 차별화 기능입니다.

생성된 스킬은 자율 에이전트(Autonomous Agent)의 ReAct 루프에 투입되어, LLM이 Plan-Act-Reflect 원칙에 따라 스킬을 자율적으로 선택하고 실행합니다. 현재 Cratos는 23개의 내장 도구를 제공하며, 스킬 시스템은 이 도구들의 조합을 학습하여 더 높은 수준의 자동화를 달성합니다.

### 핵심 특징

| 특징 | 설명 |
|------|------|
| **패턴 학습** | 3회 이상 반복된 도구 시퀀스를 자동 감지 |
| **높은 성공률** | 90%+ 성공률 목표로 스킬 생성 |
| **자동 제안** | 패턴 감지 시 사용자에게 스킬 생성 제안 |
| **편집 가능** | 생성된 스킬을 수정하거나 비활성화 가능 |
| **Docker 불필요** | SQLite 내장, 즉시 실행 가능 |

### 경쟁사 대비 장점

| 기능 | 기존 솔루션 | Cratos |
|------|-------------|--------|
| 스킬 생성 | 수동 마켓플레이스 | 자동 패턴 학습 |
| 최소 학습 횟수 | N/A | 3회 |
| 키워드 추출 | 수동 설정 | 자동 추출 |
| 변수 보간 | 제한적 | `{{variable}}` 문법 |

## 아키텍처

```
┌─────────────────────────────────────────────────────────────────────┐
│                        사용자 입력/작업                              │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    cratos-replay (EventStore)                        │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  • 실행 기록 저장                                               │ │
│  │  • 도구 호출 이벤트                                             │ │
│  │  • 사용자 입력 텍스트                                           │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      PatternAnalyzer                                 │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  • 도구 시퀀스 추출                                             │ │
│  │  • N-gram 분석 (2~5 도구 조합)                                  │ │
│  │  • 키워드 추출 (불용어 제거)                                     │ │
│  │  • 신뢰도 점수 계산                                             │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  설정:                                                              │
│  • min_occurrences: 3 (최소 발생 횟수)                              │
│  • min_confidence: 0.6 (최소 신뢰도)                                │
│  • max_sequence_length: 5 (최대 시퀀스 길이)                         │
│  • analysis_window_days: 30 (분석 기간)                             │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      SkillGenerator                                  │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  • 패턴 → 스킬 변환                                             │ │
│  │  • 트리거 키워드 설정                                           │ │
│  │  • 실행 단계 생성                                               │ │
│  │  • 입력 스키마 생성                                             │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  생성 옵션:                                                         │
│  • min_confidence: 0.7 (스킬 생성 임계값)                           │
│  • auto_activate: false (자동 활성화)                               │
│  • max_keywords: 5 (최대 키워드 수)                                 │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      SkillStore (SQLite)                             │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  테이블:                                                        │ │
│  │  • skills: 스킬 정의                                            │ │
│  │  • detected_patterns: 감지된 패턴                               │ │
│  │  • skill_executions: 실행 기록                                  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  저장 위치: ~/.cratos/skills.db                                     │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    사용자 요청 처리                                   │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    ▼                           ▼
┌─────────────────────────────┐   ┌─────────────────────────────────┐
│     SkillRouter              │   │   SemanticSkillRouter           │
│  ┌─────────────────────────┐ │   │   (semantic feature 필요)        │
│  │  • 키워드 매칭           │ │   │  ┌─────────────────────────────┐ │
│  │  • 정규식 패턴 매칭      │ │   │  │  • 벡터 임베딩 검색         │ │
│  │  • 인텐트 분류           │ │   │  │  • 하이브리드 매칭          │ │
│  │  • 우선순위 정렬         │ │   │  │  • 유사 의미 매칭           │ │
│  └─────────────────────────┘ │   │  └─────────────────────────────┘ │
└─────────────────────────────┘   └─────────────────────────────────┘
                    │                           │
                    └─────────────┬─────────────┘
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      SkillExecutor                                   │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  • 변수 보간 ({{variable}} → 실제 값)                           │ │
│  │  • 단계별 실행                                                  │ │
│  │  • 에러 처리 (Abort/Continue/Retry)                             │ │
│  │  • Dry-run 모드                                                 │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  보안 설정:                                                         │
│  • max_steps_per_skill: 50                                         │
│  • max_variable_value_length: 100KB                                │
│  • step_timeout_ms: 60000                                          │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│              자율 에이전트 루프 (Autonomous Agent Loop)               │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  • Plan-Act-Reflect 원칙에 따라 LLM이 도구 자율 선택            │ │
│  │  • 23개 내장 도구 (cratos-tools) 활용                           │ │
│  │  • 소프트 실패 시 _diagnosis 힌트 → LLM 자동 대안 탐색          │ │
│  │  • 연속 실패 시 [reflection] 프롬프트 자동 주입                  │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

## 핵심 컴포넌트

### 1. PatternAnalyzer

사용 기록에서 반복되는 도구 사용 패턴을 감지합니다.

```rust
use cratos_skills::{PatternAnalyzer, AnalyzerConfig};

// 기본 설정으로 분석기 생성
let analyzer = PatternAnalyzer::new();

// 커스텀 설정
let config = AnalyzerConfig {
    min_occurrences: 3,      // 최소 3회 반복
    min_confidence: 0.6,      // 60% 이상 신뢰도
    max_sequence_length: 5,   // 최대 5개 도구 시퀀스
    analysis_window_days: 30, // 최근 30일 분석
};
let analyzer = PatternAnalyzer::with_config(config);

// 패턴 감지
let patterns = analyzer.detect_patterns(&event_store).await?;

for pattern in &patterns {
    println!("패턴: {:?}", pattern.tool_sequence);
    println!("발생 횟수: {}", pattern.occurrence_count);
    println!("신뢰도: {:.1}%", pattern.confidence_score * 100.0);
    println!("추출 키워드: {:?}", pattern.extracted_keywords);
}
```

#### 패턴 감지 알고리즘

1. **이벤트 수집**: 최근 N일간의 실행 기록 조회
2. **시퀀스 추출**: 실행별 도구 호출 순서 추출
3. **N-gram 분석**: 2~5개 도구 조합의 빈도 계산
4. **신뢰도 계산**: `발생 횟수 / 전체 실행 수`
5. **키워드 추출**: 사용자 입력에서 불용어 제거 후 추출
6. **패턴 정렬**: 신뢰도 × 발생 횟수 순으로 정렬

### 2. SkillGenerator

감지된 패턴을 실행 가능한 스킬로 변환합니다.

```rust
use cratos_skills::{SkillGenerator, GeneratorConfig};

let config = GeneratorConfig {
    min_confidence: 0.7,   // 70% 이상만 생성
    auto_activate: false,   // 수동 활성화
    max_keywords: 5,        // 최대 5개 키워드
};
let generator = SkillGenerator::with_config(config);

// 단일 패턴 → 스킬
let skill = generator.generate_from_pattern(&pattern)?;
println!("생성된 스킬: {}", skill.name);
println!("트리거 키워드: {:?}", skill.trigger.keywords);

// 다수 패턴 일괄 변환
let skills = generator.generate_from_patterns(&patterns);
for (skill, pattern_id) in skills {
    println!("스킬 '{}' 생성 완료 (패턴: {})", skill.name, pattern_id);
}
```

#### 생성되는 스킬 구조

```rust
// 생성된 스킬 예시
Skill {
    name: "file_read_then_git_commit",
    description: "Auto-generated skill: file_read → git_commit (triggers: read, commit)",
    category: SkillCategory::Custom,
    origin: SkillOrigin::AutoGenerated,
    trigger: SkillTrigger {
        keywords: vec!["read", "commit"],
        regex_patterns: vec![],
        intents: vec![],
        priority: 0,
    },
    steps: vec![
        SkillStep {
            order: 1,
            tool_name: "file_read",
            input_template: json!({"path": "{{file_path}}"}),
            on_error: ErrorAction::Abort,
        },
        SkillStep {
            order: 2,
            tool_name: "git_commit",
            input_template: json!({"message": "{{commit_message}}"}),
            on_error: ErrorAction::Continue,
        },
    ],
    input_schema: json!({
        "type": "object",
        "properties": {
            "file_path": {"type": "string"},
            "commit_message": {"type": "string"}
        },
        "required": ["file_path", "commit_message"]
    }),
}
```

### 3. SkillStore

SQLite 기반 영구 저장소입니다.

```rust
use cratos_skills::{SkillStore, default_skill_db_path};

// 기본 경로 사용 (~/.cratos/skills.db)
let store = SkillStore::from_path(&default_skill_db_path()).await?;

// 스킬 저장
store.save_skill(&skill).await?;

// 스킬 조회
let skill = store.get_skill(skill_id).await?;
let skill = store.get_skill_by_name("file_reader").await?;

// 활성 스킬 목록
let active_skills = store.list_active_skills().await?;

// 패턴 저장 및 상태 관리
store.save_pattern(&pattern).await?;
store.mark_pattern_converted(pattern_id, skill_id).await?;
store.mark_pattern_rejected(pattern_id).await?;

// 실행 기록
store.record_skill_execution(
    skill_id,
    Some(execution_id),
    true,  // success
    Some(150),  // duration_ms
    &step_results,
).await?;
```

#### 데이터베이스 스키마

```sql
-- 스킬 테이블
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'custom',
    origin TEXT NOT NULL DEFAULT 'user_defined',
    status TEXT NOT NULL DEFAULT 'draft',
    trigger_keywords TEXT NOT NULL DEFAULT '[]',
    trigger_regex_patterns TEXT NOT NULL DEFAULT '[]',
    trigger_intents TEXT NOT NULL DEFAULT '[]',
    trigger_priority INTEGER NOT NULL DEFAULT 0,
    steps TEXT NOT NULL DEFAULT '[]',
    input_schema TEXT,
    usage_count INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 1.0,
    avg_duration_ms INTEGER,
    last_used_at TEXT,
    source_pattern_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 패턴 테이블
CREATE TABLE detected_patterns (
    id TEXT PRIMARY KEY,
    tool_sequence TEXT NOT NULL,
    occurrence_count INTEGER NOT NULL,
    confidence_score REAL NOT NULL,
    extracted_keywords TEXT NOT NULL DEFAULT '[]',
    sample_inputs TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'detected',
    converted_skill_id TEXT,
    detected_at TEXT NOT NULL,
    FOREIGN KEY (converted_skill_id) REFERENCES skills(id)
);

-- 실행 기록 테이블
CREATE TABLE skill_executions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    execution_id TEXT,
    success INTEGER NOT NULL,
    duration_ms INTEGER,
    step_results TEXT NOT NULL DEFAULT '[]',
    started_at TEXT NOT NULL,
    FOREIGN KEY (skill_id) REFERENCES skills(id)
);
```

### 4. SkillRouter

사용자 입력을 적절한 스킬에 매칭합니다.

```rust
use cratos_skills::{SkillRouter, SkillRegistry, RouterConfig};

// 레지스트리 설정
let registry = SkillRegistry::new();
let skills = store.list_active_skills().await?;
registry.load_all(skills).await?;

// 라우터 설정
let config = RouterConfig {
    min_score: 0.3,           // 최소 매칭 점수
    keyword_weight: 0.4,       // 키워드 가중치
    regex_weight: 0.5,         // 정규식 가중치
    intent_weight: 0.6,        // 인텐트 가중치
    priority_bonus: 0.1,       // 우선순위 보너스
    max_input_length: 10_000,  // 최대 입력 길이 (DoS 방지)
    max_pattern_length: 500,   // 최대 패턴 길이 (ReDoS 방지)
};
let mut router = SkillRouter::with_config(registry, config);

// 모든 매칭 스킬 조회
let results = router.route("파일 읽고 커밋해줘").await;
for result in results {
    println!("스킬: {} (점수: {:.2})", result.skill.name, result.score);
    println!("매칭 이유: {:?}", result.match_reason);
}

// 최적 스킬 선택
if let Some(best) = router.route_best("파일 읽고 커밋해줘").await {
    println!("선택된 스킬: {}", best.skill.name);
}
```

### 5. SkillExecutor

스킬을 실제로 실행합니다.

```rust
use cratos_skills::{SkillExecutor, ExecutorConfig, ToolExecutor};
use std::collections::HashMap;

// ToolExecutor 구현 필요
struct MyToolExecutor { /* ... */ }

#[async_trait]
impl ToolExecutor for MyToolExecutor {
    async fn execute_tool(&self, tool_name: &str, input: Value) -> Result<Value, String> {
        // 도구 실행 로직
    }
    fn has_tool(&self, tool_name: &str) -> bool { /* ... */ }
    fn tool_names(&self) -> Vec<String> { /* ... */ }
}

// 실행기 설정
let config = ExecutorConfig {
    max_retries: 3,
    dry_run: false,
    continue_on_failure: false,
    step_timeout_ms: 60_000,
    max_variable_value_length: 100_000,
    max_steps_per_skill: 50,
};

let executor = SkillExecutor::new(tool_executor)
    .with_config(config);

// 변수 준비
let mut variables = HashMap::new();
variables.insert("file_path".to_string(), json!("/path/to/file.txt"));
variables.insert("commit_message".to_string(), json!("Update file"));

// 실행
let result = executor.execute(&skill, &variables).await?;

if result.success {
    println!("스킬 실행 성공! ({}ms)", result.total_duration_ms);
    for step in &result.step_results {
        println!("  단계 {}: {} - 성공", step.step, step.tool_name);
    }
} else {
    println!("스킬 실행 실패: {:?}", result.error);
}
```

## 스킬 스키마

### Skill 정의

```rust
pub struct Skill {
    pub id: Uuid,                       // 고유 ID
    pub name: String,                   // 스킬 이름
    pub description: String,            // 설명
    pub category: SkillCategory,        // Custom | System
    pub origin: SkillOrigin,            // Builtin | UserDefined | AutoGenerated
    pub status: SkillStatus,            // Draft | Active | Disabled
    pub trigger: SkillTrigger,          // 트리거 설정
    pub steps: Vec<SkillStep>,          // 실행 단계
    pub input_schema: Option<Value>,    // JSON Schema
    pub metadata: SkillMetadata,        // 사용 통계
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### SkillStep 정의

```rust
pub struct SkillStep {
    pub order: u32,                    // 실행 순서 (1-based)
    pub tool_name: String,             // 도구 이름
    pub input_template: Value,         // 입력 템플릿 ({{var}} 문법)
    pub on_error: ErrorAction,         // Abort | Continue | Retry
    pub description: Option<String>,   // 단계 설명
    pub max_retries: u32,              // 최대 재시도 횟수
}
```

### SkillTrigger 정의

```rust
pub struct SkillTrigger {
    pub keywords: Vec<String>,         // 트리거 키워드
    pub regex_patterns: Vec<String>,   // 정규식 패턴
    pub intents: Vec<String>,          // 인텐트 분류
    pub priority: i32,                 // 우선순위
}
```

## 사용 예시

### 전체 흐름

```rust
use cratos_skills::*;
use cratos_replay::EventStore;

// 1. 저장소 초기화
let event_store = EventStore::from_path(&default_data_dir()).await?;
let skill_store = SkillStore::from_path(&default_skill_db_path()).await?;

// 2. 패턴 분석
let analyzer = PatternAnalyzer::new();
let patterns = analyzer.detect_patterns(&event_store).await?;

println!("감지된 패턴: {}개", patterns.len());

// 3. 스킬 생성
let generator = SkillGenerator::new();
for pattern in &patterns {
    // 이미 변환된 패턴 건너뛰기
    if pattern.status == PatternStatus::Converted {
        continue;
    }

    match generator.generate_from_pattern(pattern) {
        Ok(skill) => {
            // 사용자에게 제안
            println!("\n새로운 스킬을 발견했습니다!");
            println!("이름: {}", skill.name);
            println!("설명: {}", skill.description);
            println!("단계: {:?}", skill.steps.iter().map(|s| &s.tool_name).collect::<Vec<_>>());

            // 저장 및 패턴 상태 업데이트
            skill_store.save_skill(&skill).await?;
            skill_store.mark_pattern_converted(pattern.id, skill.id).await?;
        }
        Err(e) => {
            println!("스킬 생성 실패: {}", e);
        }
    }
}

// 4. 레지스트리 로드
let registry = SkillRegistry::new();
let active_skills = skill_store.list_active_skills().await?;
registry.load_all(active_skills).await?;

// 5. 라우팅 및 실행
let mut router = SkillRouter::new(registry);

let user_input = "파일 읽고 커밋해줘";
if let Some(result) = router.route_best(user_input).await {
    println!("매칭된 스킬: {} (점수: {:.2})", result.skill.name, result.score);

    // 실행
    let mut variables = HashMap::new();
    variables.insert("file_path".to_string(), json!("./README.md"));
    variables.insert("commit_message".to_string(), json!("Auto commit"));

    let exec_result = executor.execute(&result.skill, &variables).await?;
    println!("실행 결과: {:?}", exec_result.success);
}
```

### 스킬 수동 생성

```rust
use cratos_skills::*;

// 커스텀 스킬 정의
let skill = Skill::new(
    "daily_report",
    "일일 보고서 생성 및 전송",
    SkillCategory::Custom,
)
.with_trigger(
    SkillTrigger::with_keywords(vec![
        "일일".to_string(),
        "보고서".to_string(),
        "리포트".to_string(),
    ])
    .add_pattern(r"일일\s*(보고|리포트)")
    .with_priority(10),
)
.with_step(
    SkillStep::new(1, "git_log", json!({
        "since": "{{since_date}}",
        "format": "oneline"
    }))
    .with_description("커밋 로그 조회")
    .with_on_error(ErrorAction::Abort),
)
.with_step(
    SkillStep::new(2, "file_write", json!({
        "path": "{{output_path}}",
        "content": "{{step1_output}}"
    }))
    .with_description("보고서 파일 작성")
    .with_on_error(ErrorAction::Continue),
)
.with_step(
    SkillStep::new(3, "slack_send", json!({
        "channel": "{{channel}}",
        "message": "일일 보고서가 생성되었습니다."
    }))
    .with_description("Slack 알림 전송")
    .with_on_error(ErrorAction::Continue),
);

// 활성화 및 저장
skill.activate();
store.save_skill(&skill).await?;
```

## 설정 옵션

### AnalyzerConfig

| 옵션 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `min_occurrences` | `u32` | `3` | 패턴으로 인식할 최소 발생 횟수 |
| `min_confidence` | `f32` | `0.6` | 최소 신뢰도 점수 |
| `max_sequence_length` | `usize` | `5` | 분석할 최대 도구 시퀀스 길이 |
| `analysis_window_days` | `i64` | `30` | 분석 대상 기간 (일) |

### GeneratorConfig

| 옵션 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `min_confidence` | `f32` | `0.7` | 스킬 생성 최소 신뢰도 |
| `auto_activate` | `bool` | `false` | 생성 즉시 활성화 여부 |
| `max_keywords` | `usize` | `5` | 트리거에 포함할 최대 키워드 수 |

### RouterConfig

| 옵션 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `min_score` | `f32` | `0.3` | 매칭으로 인정할 최소 점수 |
| `keyword_weight` | `f32` | `0.4` | 키워드 매칭 가중치 |
| `regex_weight` | `f32` | `0.5` | 정규식 매칭 가중치 |
| `intent_weight` | `f32` | `0.6` | 인텐트 매칭 가중치 |
| `priority_bonus` | `f32` | `0.1` | 우선순위 보너스 |
| `max_input_length` | `usize` | `10000` | 최대 입력 길이 (보안) |
| `max_pattern_length` | `usize` | `500` | 최대 패턴 길이 (보안) |

### ExecutorConfig

| 옵션 | 타입 | 기본값 | 설명 |
|------|------|--------|------|
| `max_retries` | `u32` | `3` | 단계별 최대 재시도 횟수 |
| `dry_run` | `bool` | `false` | 테스트 모드 (실제 실행 안 함) |
| `continue_on_failure` | `bool` | `false` | 실패 시 계속 진행 |
| `step_timeout_ms` | `u64` | `60000` | 단계별 타임아웃 (ms) |
| `max_variable_value_length` | `usize` | `100000` | 변수 값 최대 길이 (보안) |
| `max_steps_per_skill` | `usize` | `50` | 스킬당 최대 단계 수 (보안) |

## 보안 고려사항

### 입력 검증

```rust
// 라우터: 입력 길이 제한 (DoS 방지)
if input_text.len() > config.max_input_length {
    return Vec::new();  // 거부
}

// 라우터: 정규식 길이 제한 (ReDoS 방지)
if pattern.len() > config.max_pattern_length {
    continue;  // 건너뛰기
}

// 실행기: 단계 수 제한
if skill.steps.len() > config.max_steps_per_skill {
    return Err(Error::Validation("too many steps"));
}

// 실행기: 변수 값 크기 제한
if value.len() > config.max_variable_value_length {
    return Err(Error::Validation("variable too large"));
}
```

### 추천 보안 설정

1. **프로덕션 환경**
   - `auto_activate: false` - 수동 검토 후 활성화
   - `max_input_length: 10000` - 입력 길이 제한
   - `step_timeout_ms: 30000` - 타임아웃 단축

2. **민감한 환경**
   - 민감한 도구(exec, shell)는 스킬에서 제외
   - 화이트리스트 기반 도구 허용

## API 레퍼런스

### 주요 타입

| 타입 | 설명 |
|------|------|
| `PatternAnalyzer` | 패턴 분석기 |
| `DetectedPattern` | 감지된 패턴 |
| `PatternStatus` | Detected, Converted, Rejected, Expired |
| `SkillGenerator` | 스킬 생성기 |
| `Skill` | 스킬 정의 |
| `SkillCategory` | Custom, System |
| `SkillOrigin` | Builtin, UserDefined, AutoGenerated |
| `SkillStatus` | Draft, Active, Disabled |
| `SkillStep` | 실행 단계 |
| `SkillTrigger` | 트리거 설정 |
| `ErrorAction` | Abort, Continue, Retry |
| `SkillStore` | SQLite 저장소 |
| `SkillRegistry` | 인메모리 레지스트리 |
| `SkillRouter` | 키워드/정규식 라우터 |
| `SemanticSkillRouter` | 의미 기반 라우터 (선택적) |
| `SkillExecutor` | 스킬 실행기 |
| `ToolExecutor` | 도구 실행 트레이트 |

### 에러 타입

```rust
pub enum Error {
    SkillNotFound(String),    // 스킬을 찾을 수 없음
    PatternNotFound(String),  // 패턴을 찾을 수 없음
    Database(String),         // 데이터베이스 오류
    Serialization(String),    // 직렬화 오류
    Validation(String),       // 검증 오류
    Execution(String),        // 실행 오류
    Configuration(String),    // 설정 오류
    Io(std::io::Error),       // IO 오류
    ReplayStore(cratos_replay::Error),  // Replay 저장소 오류
    Internal(String),         // 내부 오류
}
```

## Semantic Router (선택적)

`semantic` feature를 활성화하면 벡터 임베딩 기반 의미 검색을 사용할 수 있습니다.

```toml
[dependencies]
cratos-skills = { version = "0.1", features = ["semantic"] }
```

```rust
use cratos_skills::{SemanticSkillRouter, SemanticRouterConfig, SkillEmbedder};

// 임베딩 프로바이더 구현
struct MyEmbedder { /* ... */ }

#[async_trait]
impl SkillEmbedder for MyEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> { /* ... */ }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> { /* ... */ }
    fn dimensions(&self) -> usize { 768 }
}

// 시맨틱 라우터 생성
let index = create_skill_index(768, Some(&index_path))?;
let router = SemanticSkillRouter::new(registry, index, embedder);

// 스킬 인덱싱
router.reindex_all().await?;

// 하이브리드 검색 (키워드 + 의미)
let results = router.route("파일 저장해줘").await?;  // "backup" 스킬도 매칭 가능
```

## 향후 계획

1. **v1.0**: 기본 패턴 감지 및 스킬 생성
2. **v1.1**: 시맨틱 라우팅 (cratos-search 연동)
3. **v1.2**: LLM 기반 인텐트 분류
4. **v2.0**: 스킬 버전 관리 및 롤백
5. **v2.1**: 스킬 공유 및 마켓플레이스
