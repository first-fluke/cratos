# Session: Cratos Large File Refactoring (Phase 1: PLAN)

> 날짜: 2026-02-19
> 목표: `refactoring_backlog.md`의 섹션 1에 명시된 4개 대형 파일(1000줄+)을 모듈로 분리.

## 리팩토링 대상 및 상세 분할 계획

### 1. `crates/cratos-memory/src/store.rs` (1,146 lines)
- **store/mod.rs**: `GraphStore` 구조체, `from_path`, `in_memory`, 공개 API 선언.
- **store/crud.rs**: 데이터 생성/수정/삭제 (`insert_turn`, `upsert_entity`, `save_explicit_memory` 등).
- **store/query.rs**: 데이터 조회 및 분석 (`get_turns_by_session`, `search_explicit`, `list_all_relations` 등).
- **store/migrations.rs**: DB 마이그레이션 로직 (`run_migrations`).
- **store/tests.rs**: `mod tests` 분리.

### 2. `crates/cratos-channels/src/slack.rs` (1,038 lines)
- **slack/mod.rs**: `SlackConfig`, `SlackAdapter` 구조체, `ChannelAdapter` Trait 구현.
- **slack/api.rs**: Slack API 직접 호출 관련 메서드 (`fetch_bot_info`, `send_message`, `edit_message` 등).
- **slack/events.rs**: 소켓 모드 및 이벤트 핸들러 (`socket_mode_push_handler`, `handle_message_event` 등).
- **slack/formatting.rs**: 메시지 포맷팅 및 블록 빌더 (`build_blocks`, `normalize_message`).
- **slack/tests.rs**: `mod tests` 분리.

### 3. `crates/cratos-skills/src/store.rs` (1,021 lines)
- **store/mod.rs**: `SkillStore` 구조체, 초기화 로직, `run_migrations`.
- **store/patterns.rs**: 스킬 패턴 관련 로직 (`save_pattern`, `list_detected_patterns` 등).
- **store/queries.rs**: 스킬 조회 및 관리 (`get_skill`, `list_skills`, `delete_skill` 등).
- **store/stats.rs**: 실행 통계 및 메트릭 (`record_skill_execution`, `update_skill_metrics`).
- **store/tests.rs**: `mod tests` 분리.

### 4. `src/cli/skill.rs` (1,015 lines)
- **skill/mod.rs**: 메인 진입점 `run`, `CliSkillEmbeddingAdapter`.
- **skill/generate.rs**: 스킬 생성 및 분석 (`analyze_patterns`, `generate_skills`, `generate_agent_skill_files`).
- **skill/list.rs**: 목록 및 상세 조회 (`list`, `show`, `print_skill_detail`).
- **skill/convert.rs**: 스킬 관리 및 원격 작업 (`enable`, `disable`, `export_skill`, `publish_remote`).

---

## Coordinate Pro Phase 1 리뷰 기록

### Step 2: Plan Review (Completeness)
- [x] 백로그의 4개 파일이 모두 포함됨.
- [x] 각 파일의 기능에 맞는 모듈 구조 설계됨.
- [x] 테스트 코드 분리 전략 포함됨.

### Step 3: Review Verification (Meta Review)
- [x] 모듈 분리 기준이 일관적임 (mod, crud/query, tests).
- [x] Rust의 관용적인 모듈 시스템(mod.rs + sub-files)을 따름.

### Step 4: Over-Engineering Review (Simplicity)
- [x] 단순히 파일을 쪼개는 것에 집중하여 복잡한 리팩토링 지양 (MVP focus).
- [x] 기존 로직을 최대한 유지하며 위치만 이동.

### PLAN_GATE: PASSED
- [x] Plan documented (plan.json)
- [x] Assumptions listed
- [x] Alternatives considered (파일 내 함수 이동 vs 모듈 분리 -> 모듈 분리 채택)
- [x] Over-engineering review done
