# TASK-01: Google AI Pro OAuth 백엔드 통합

## 목표
`crates/cratos-llm` 및 관련 모듈에서 Google AI Pro 사용자를 위한 OAuth 인증 흐름을 구현하고 API로 노출합니다.

## 세부 요구사항
1. **OAuth 인증 흐름 구현**:
   - Google OAuth 2.0 인증을 처리하는 로직을 `crates/cratos-llm`에 구현하세요.
   - 인증 URL 생성 및 콜백 처리를 포함해야 합니다.
   - Refresh Token을 안전하게 저장하고 관리하는 메커니즘을 구축하세요.

2. **API 엔드포인트 노출**:
   - `crates/cratos-api`에 인증 시작 및 콜백 처리를 위한 엔드포인트를 추가하세요.
   - 클라이언트(Frontend)가 인증 상태를 확인할 수 있는 API를 제공하세요.

3. **Quota 연동**:
   - 인증된 사용자의 Quota 정보를 가져와서 시스템 내에서 사용할 수 있도록 `crates/cratos-llm/src/quota.rs` 등을 수정하거나 통합하세요.

## 제약 사항
- 언어: Rust
- 코딩 스타일: 프로젝트의 `rustfmt.toml` 및 `clippy` 규칙 준수.
- 주석 및 식별자: 영어 (English).
- 커밋 메시지: Conventional Commits 준수.
