# Rust 개발 실행 프로토콜

## 4단계 체인-오브-싱크

### 1단계: 컨텍스트 수집

```
1. 관련 파일 식별
   - Cargo.toml 확인 (의존성)
   - 모듈 구조 파악 (lib.rs, mod.rs)

2. 심볼 탐색 (Serena)
   - get_symbols_overview → 구조 파악
   - find_symbol → 수정 대상 찾기

3. 의존성 파악
   - find_referencing_symbols → 영향 범위
```

### 2단계: 설계

```
1. 변경 범위 결정
   - 수정할 파일 목록
   - 추가할 파일 목록
   - 영향받는 테스트

2. 인터페이스 설계
   - trait 정의
   - 타입 시그니처
   - 에러 타입

3. 검증 계획
   - 단위 테스트 계획
   - 통합 테스트 계획
```

### 3단계: 구현

```
1. 순서대로 구현
   - 의존성 먼저 (types, errors)
   - 핵심 로직
   - 테스트

2. 점진적 검증
   - cargo check (컴파일)
   - cargo test (테스트)
   - cargo clippy (린트)
```

### 4단계: 검증 & 정리

```
1. 전체 테스트
   - cargo test --all

2. 린트 & 포맷
   - cargo clippy -- -D warnings
   - cargo fmt

3. 문서화
   - rustdoc 주석
   - 필요시 README 업데이트
```

## 체크리스트

- [ ] `#![forbid(unsafe_code)]` 확인
- [ ] 모든 `Result` 적절히 처리
- [ ] `#[instrument]` 추적 추가
- [ ] 테스트 작성 완료
- [ ] clippy 경고 0개
- [ ] 포맷팅 완료
