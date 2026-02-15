# TASK-02: 주요 UI 버그 수정

## 목표
현재 보고된 크리티컬한 UI 버그들을 수정하여 시스템 안정성을 확보합니다.

## 대상 이슈
1. **페르소나 디테일/스킬 누락 (Issue #2)**
   - 증상: `/personas`에서 페르소나 클릭 시 상세 정보와 스킬이 표시되지 않음.
   - 예상 원인: 데이터 페칭 실패 또는 프로퍼티 매핑 오류.

2. **Goals 및 Quest Progress 데이터 오류 (Issue #2)**
   - 증상: Goals가 모두 0으로 표시되고, Quest Progress가 0/0으로 표시됨.
   - 작업: 실제 데이터를 바인딩하거나, 데이터가 없을 경우 적절한 Empty State를 구현.

3. **History View 404 (Issue #3)**
   - 증상: `/history`에서 항목 클릭 시 View 페이지가 404 에러 반환.
   - 작업: 라우팅 설정(`apps/web/src/routes` 등) 및 상세 페이지 컴포넌트 유무 확인.

4. **Docs 404 (Issue #5)**
   - 증상: 대시보드에서 `/docs` 접근 시 404 에러.
   - 작업: 문서 페이지 라우팅 연결.

## 제약 사항
- 언어: TypeScript, React
- 스타일링: Tailwind CSS (기존 스타일 유지)
- 주석 및 식별자: 영어 (English)
