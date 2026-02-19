# 워크플로우 실행 프로토콜

## 4단계 프로세스

### 1단계: 요청 분석

```
입력: 자연어 요청
출력: 작업 그래프 (DAG)

분석 항목:
- 목표 식별
- 필요 에이전트 식별
- 의존성 파악
- 병렬화 가능 여부
```

### 2단계: 계획 수립

```yaml
workflow:
  name: "Issue #123 해결"
  steps:
    - id: analyze
      agent: rust-agent
      action: 이슈 분석 및 코드 탐색
      depends_on: []

    - id: fix
      agent: rust-agent
      action: 코드 수정
      depends_on: [analyze]

    - id: test
      agent: qa-agent
      action: 테스트 실행
      depends_on: [fix]

    - id: commit
      agent: commit
      action: PR 생성
      depends_on: [test]
```

### 3단계: 실행

```
각 단계별:
1. 의존성 완료 확인
2. 에이전트 호출
3. 결과 수집
4. 실패 시 롤백 또는 재시도
```

### 4단계: 보고

```markdown
## 워크플로우 완료 리포트

### 실행 요약
- 총 단계: 4
- 성공: 4
- 실패: 0

### 단계별 결과
1. [analyze] ✅ 이슈 분석 완료
2. [fix] ✅ 코드 수정 완료 (+15 -3)
3. [test] ✅ 54 tests passed
4. [commit] ✅ PR #456 생성

### 산출물
- PR: https://github.com/user/repo/pull/456
```

## 에러 처리

| 상황 | 대응 |
|------|------|
| 에이전트 실패 | 재시도 (최대 2회) |
| 의존성 실패 | 의존 단계부터 재시도 |
| 전체 실패 | 롤백 + 사용자 알림 |
