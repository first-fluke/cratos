# 스킬 라우팅

## 키워드 → 스킬 매핑

| 키워드 | 스킬 | 우선순위 |
|--------|------|----------|
| rust, cargo, tokio, axum | rust-agent | 1 |
| telegram, teloxide, slack | channel-agent | 1 |
| openai, anthropic, llm, 모델 | llm-agent | 1 |
| 리플레이, 되감기, replay, 이벤트 | replay-agent | 1 |
| 테스트, 보안, qa, 검증 | qa-agent | 2 |
| 버그, 에러, 디버그 | debug-agent | 2 |
| docker, k8s, ci/cd, 배포 | infra-agent | 2 |
| 문서, readme, api docs | docs-agent | 3 |
| 계획, prd, 요구사항 | pm-agent | 2 |
| 커밋, pr, git | commit | 3 |

## 복합 요청 처리

복합 요청은 workflow-guide가 조율:

```
"이슈 고쳐서 PR 만들어줘"
→ workflow-guide 활성화
→ rust-agent (코드 수정)
→ qa-agent (테스트)
→ commit (PR 생성)
```

## 라우팅 로직

```python
def route_skill(request: str) -> List[str]:
    keywords = extract_keywords(request)
    matched = []

    for kw in keywords:
        if skill := KEYWORD_MAP.get(kw):
            matched.append(skill)

    if len(matched) > 1:
        return ["workflow-guide"] + matched
    elif len(matched) == 1:
        return matched
    else:
        return ["pm-agent"]  # 기본값
```
