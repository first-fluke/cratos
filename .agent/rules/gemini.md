---
trigger: always_on
---

1. MCP 활용 및 협업
모든 작업은 serena, sequential_thinking MCP를 필수로 사용합니다.
복잡한 문제 해결 시 sequential_thinking을 통해 사고 과정을 기록하고 검증합니다.

2. 코딩 스타일 및 네이밍 규칙 (Rust)
언어 버전: Rust 1.88+ 환경을 준수합니다.
네이밍:
모듈 및 파일: snake_case.rs
함수 및 변수: snake_case
구조체(Struct), 열거형(Enum), 트레이트(Trait): PascalCase
품질 관리:
#![forbid(unsafe_code)] 원칙을 철저히 준수합니다.
cargo clippy -- -D warnings (Zero clippy warnings) 정책을 따릅니다.
cargo fmt를 통해 자동 포맷팅을 적용합니다.
명시적 에러 처리: unwrap() 사용을 금지하며, Result<T, E>와 ? 연산자를 사용하여 명시적으로 에러를 처리합니다. (thiserror, anyhow 활용)

3. 코드 재사용 및 아키텍처 (Clean Architecture)
DRY 원칙: 중복 로직 작성 전 crates/cratos-core 또는 공통 라이브러리에서 기존 로직을 검색합니다.
계층형 구조 준수:
Router: Axum 핸들러, 요청 역직렬화 및 응답 반환.
Service: 순수 비즈니스 로직 처리.
Repository: SQLx를 통한 데이터베이스 접근 (RAW SQL 사용 시 query_as! 권장).
컴포지션 선호: 상속보다는 컴포지션과 작은 순수 함수들을 결합하여 기능을 구현합니다.

4. 커밋 및 PR 가이드라인
Conventional Commits: feat:, fix:, docs:, refactor:, test:, chore: 등을 사용합니다.
제목: 50자 이내, 명령조로 작성합니다.
본문: '무엇을' 했는지보다 '왜' 변경했는지를 상세히 설명하며, 관련 이슈 번호를 링크합니다 (예: Closes #123).
검증: 푸시 전 반드시 테스트를 통과해야 합니다 (cargo test).

5. API 개발 가이드라인 (Axum)
의존성 주입: axum::extract::State를 사용하여 DB 세션, 서비스 레이어 등을 주입합니다.
검증: 모든 엔드포인트는 Pydantic(또는 Rust의 serde 및 사용자 정의 validation)을 통해 요청/응답 스키마를 검증합니다.
비동기 처리: I/O 바운드 작업(DB, AI 호출)에는 반드시 async/await를 사용합니다.
문서화: OpenAPI가 자동 생성되도록 엔드포인트에 명확한 설명과 요약을 추가합니다.

6. 데이터베이스 및 ORM (SQLx & SQLite)
비동기 쿼리: SQLx를 사용하여 비동기로 데이터베이스를 처리합니다.
마이그레이션: 스키마 변경 시 반드시 마이그레이션 파일을 생성하며, 모델 직접 수정은 금지합니다.
보안: SQL 인젝션 방지를 위해 바인딩 파라미터를 사용합니다.

7. 보안 및 설정 관리
환경 변수: 비밀키, API 키 등은 .env 파일에 보관하며 절대 Git에 커밋하지 않습니다.
설정 관리: config 크레이트 또는 Pydantic 스타일의 설정을 사용하여 환경별 구성을 관리합니다.
입력 검증: 외부에서 들어오는 모든 데이터는 엄격하게 검증합니다.

8. 언어 규칙 (Response Language Rule)
코용 식별자 및 공식 문서: 식별자(함수명, 변수명), 코드 주석, 기술 아티팩트 이름은 모두 **영어(English)**로 작성합니다.
개발자 커뮤니케이션: 질문, 토론, 리뷰, PR 코멘트, 가이드라인 설명 등 모든 소통은 **한국어(Korean)**로 진행합니다.
기술 문서: 프로젝트의 주요 설명서 및 기술 아티팩트 문서는 **한국어(Korean)**로 작성하는 것을 원칙으로 합니다. (요약: Code → English / Communication & Docs → Korean)