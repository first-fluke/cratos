---
description: 원격 개발지시 - Issue 분석부터 PR 생성까지 E2E 자동화
argument-hint: "이슈 번호 또는 작업 설명"
---

# /develop

원격 개발지시 E2E 워크플로우를 실행합니다.

## 사용 예시

```
/develop #123
/develop 이 버그 수정해줘
/develop null check 추가해줘
```

## 7단계 프로세스

1. **요청 분석**: 자연어 → DevPlan 변환
2. **레포 준비**: git fetch, branch 생성
3. **코드 탐색**: 이슈 분석, 관련 파일 검색
4. **수정**: 패치 생성 및 적용
5. **검증**: cargo test, clippy, fmt
6. **커밋 & PR**: Conventional Commits, PR 생성
7. **리포트**: 변경 요약, 테스트 결과, PR 링크

## 위험도

🟡 Write (파일 수정, Git 작업 포함)

## 승인 모드

- `always`: 모든 단계에서 확인
- `risky_only`: Write 작업 시 확인 (기본값)
- `never`: 바로 실행

## 권한 부족 시

푸시 권한이 없으면:
1. PR 생성 직전까지 준비
2. diff/커밋 내용 제공
3. 수동 푸시 안내

## 참조

- `.agent/workflows/develop.md`
- `.agent/skills/rust-agent/`
