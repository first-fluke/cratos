---
name: Security Audit
description: This skill should be used when performing security audits on Cratos - command injection analysis, REST API authorization review, WebSocket authentication, tool security, and generating actionable fix plans.
version: 1.0.0
---

# Security Audit Skill

Cratos 프로젝트의 보안 감사를 수행하는 스킬. 코드 분석 → 취약점 식별 → 심각도 분류 → 수정 플랜 생성까지 E2E로 처리한다.

## 감사 범위

### 1. Command Injection (exec/bash 도구)
대상 파일:
- `crates/cratos-tools/src/builtins/exec.rs` — 직접 실행 도구
- `crates/cratos-tools/src/builtins/bash.rs` — PTY 기반 셸 도구 (5-layer security)
- `crates/cratos-core/src/security/injection.rs` — InjectionDetector 패턴

체크리스트:
- [ ] 셸 메타문자 차단 (`;|&$\`()<>\n\r!#`)
- [ ] 위험 커맨드 블랙리스트 (rm, sudo, bash, python, curl 등 70+)
- [ ] 버전 인터프리터 우회 (`python3.11`, `perl5.34` 등 prefix match)
- [ ] 커맨드 래퍼 차단 (`env`, `xargs`, `nohup`, `osascript`)
- [ ] 위험 경로 차단 (`/etc`, `/root`, `/var/log` 등)
- [ ] args 안전성 (Command::new() 셸 미경유 확인)
- [ ] Docker 샌드박스 격리 (network=none, read-only, pids-limit)

bash 도구 5-Layer:
- [ ] Layer 1: 입력 검증 (LD_PRELOAD, $(curl, heredoc, process substitution)
- [ ] Layer 2: 파이프라인 분석 (세그먼트별 커맨드 블록, glob/alias/function 차단)
- [ ] Layer 3: 환경 격리 (env 화이트리스트, workspace jail, 경로 차단)
- [ ] Layer 4: 리소스 제한 (rate limit, 출력 제한, 세션 제한)
- [ ] Layer 5: 출력 검증 (시크릿 마스킹, base64 데이터 마스킹)

### 2. REST API 인가
대상 파일:
- `src/middleware/auth.rs` — RequireAuth 추출자
- `src/server.rs` — 라우터 구성, AuthConfig
- `src/api/*.rs` — 모든 REST 핸들러

체크리스트:
- [ ] 모든 엔드포인트에 RequireAuth 적용 여부
- [ ] Scope 세분화 (ConfigRead/ConfigWrite/SchedulerWrite/Admin 등)
- [ ] 기본 인증 설정 (enabled 기본값, 프로덕션 강제)
- [ ] 민감 정보 노출 (/health/detailed, /metrics)

### 3. WebSocket 인증
대상 파일:
- `src/websocket/chat.rs` — WS 채팅
- `src/websocket/events.rs` — 이벤트 스트림

체크리스트:
- [ ] WS upgrade 시 토큰 검증
- [ ] `?token=` 쿼리 파라미터 추출 → AuthStore 검증
- [ ] 인증 실패 시 연결 거부

### 4. 스케줄러 보안
대상 파일:
- `src/api/scheduler.rs` — 스케줄러 API
- `src/server.rs` — task_executor 클로저

체크리스트:
- [ ] Shell Action이 exec 보안 필터 경유하는지
- [ ] API를 통한 악성 작업 등록 방어
- [ ] Scope 분리 (read/write)

### 5. 입력 검증
대상 파일:
- `crates/cratos-core/src/security/injection.rs` — InjectionDetector
- `crates/cratos-core/src/security/mod.rs` — 보안 모듈

체크리스트:
- [ ] SQL injection 패턴 (sqlx 파라미터 바인딩)
- [ ] SSRF (web_search 도구 URL 필터링)
- [ ] Path traversal (`../` 차단)

## 감사 수행 절차

### Phase 1: 정보 수집
1. `cargo check --all-targets` — 빌드 확인
2. `cargo test -p cratos-tools` — 보안 테스트 실행
3. 관련 소스 파일 읽기 (위 대상 파일 목록)

### Phase 2: 취약점 식별
각 취약점에 대해:
- **위치**: 파일:라인번호
- **심각도**: CRITICAL / HIGH / MEDIUM / LOW
- **영향**: 공격 시나리오
- **코드 증거**: 관련 코드 스니펫

### Phase 3: 보고서 생성
심각도별 분류 테이블:

```markdown
| 코드 | 심각도 | 위치 | 설명 | 상태 |
|------|--------|------|------|------|
| V2-1 | CRITICAL | server.rs:98 | 기본 인증 비활성화 | 미수정 |
```

### Phase 4: 수정 플랜
우선순위별 수정 가이드:
1. **CRITICAL** — 즉시 수정 (PR 분리)
2. **HIGH** — 같은 스프린트 내 수정
3. **MEDIUM** — 다음 스프린트
4. **LOW** — 백로그

## 이전 감사 결과 참조

- `~/.claude/projects/-Volumes-gahyun-ex-projects-cratos/memory/security-audit.md` — v1 (2026-02-08)
- `~/.claude/projects/-Volumes-gahyun-ex-projects-cratos/memory/security-audit-v2.md` — v2 (2026-02-11)

### 알려진 양호 영역
- Command Injection: exec/bash 모두 안전 (9/10)
- 비밀 관리: OS 키체인 + zeroize (8/10)
- Docker 샌드박스: network=none, read-only, pids-limit (9/10)

### 알려진 취약 영역 (v2 기준)
- REST API 인가 (6/10) — 5건 취약점
- WS 인증 없음 — CRITICAL
- /health/detailed 정보 노출 — HIGH

## 보안 테스트 커맨드

```bash
# exec 도구 보안 테스트
cargo test -p cratos-tools -- exec::tests --nocapture

# bash 도구 보안 테스트
cargo test -p cratos-tools -- bash::tests --nocapture

# 전체 보안 관련 테스트
cargo test -p cratos-tools --nocapture
cargo test -p cratos-core -- security --nocapture

# 의존성 취약점
cargo audit
```
