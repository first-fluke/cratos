# 컨텍스트 예산 관리

## 토큰 예산 배분

| 구성 요소 | 최대 토큰 | 비율 |
|-----------|----------|------|
| 시스템 프롬프트 | 2,000 | 10% |
| 스킬 SKILL.md | 1,000 | 5% |
| 스킬 리소스 | 4,000 | 20% |
| 코드 컨텍스트 | 8,000 | 40% |
| 대화 히스토리 | 3,000 | 15% |
| 응답 여유 | 2,000 | 10% |
| **합계** | **20,000** | **100%** |

## 파일 읽기 전략

### 1. 심볼 우선 (Serena)
```
좋음: find_symbol("ToolRegistry") → 해당 구조체만
나쁨: 전체 파일 읽기
```

### 2. 점진적 확장
```
1단계: get_symbols_overview → 구조 파악
2단계: find_symbol → 필요한 심볼만
3단계: include_body=true → 코드 필요 시
```

### 3. 검색 범위 제한
```rust
// 좋음: 경로 제한
find_symbol("execute", relative_path="crates/cratos-tools/")

// 나쁨: 전체 검색
find_symbol("execute")
```

## 리소스 로드 조건

| 조건 | 로드할 리소스 |
|------|--------------|
| 새 크레이트 추가 | tech-stack.md |
| 에러 발생 | error-playbook.md |
| 복잡한 구현 | execution-protocol.md |
| 패턴 필요 | snippets.md |
| 검증 필요 | checklist.md |
