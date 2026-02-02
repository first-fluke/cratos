# 메모리 프로토콜

## Serena 메모리 사용 규칙

### 읽기 (read_memory)
```
언제: 작업 시작 전, 관련 컨텍스트 필요 시
도구: mcp__serena__read_memory
파일: .serena/memories/{task}-context.md
```

### 쓰기 (write_memory)
```
언제: 작업 완료 후, 중요 정보 발견 시
도구: mcp__serena__write_memory
형식: 마크다운 + YAML 프론트매터
```

### 수정 (edit_memory)
```
언제: 기존 정보 업데이트 시
도구: mcp__serena__edit_memory
모드: literal | regex
```

## 메모리 파일 명명 규칙

| 유형 | 패턴 | 예시 |
|------|------|------|
| 작업 컨텍스트 | `{task}-context.md` | `develop-context.md` |
| 에이전트 상태 | `{agent}-state.md` | `rust-agent-state.md` |
| 세션 요약 | `session-{id}.md` | `session-abc123.md` |
| 교훈 | `lessons-{domain}.md` | `lessons-rust.md` |

## 메모리 구조 템플릿

```markdown
---
type: context | state | summary | lesson
agent: rust-agent
created: 2024-01-01T00:00:00Z
updated: 2024-01-01T00:00:00Z
---

# {제목}

## 요약
{1-2줄 요약}

## 상세
{상세 내용}

## 관련 파일
- path/to/file.rs

## 다음 단계
- [ ] 할 일 1
- [ ] 할 일 2
```
