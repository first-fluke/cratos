---
description: Cratos 프로젝트 초기 설정 - Cargo workspace, 크레이트 스캐폴딩, Docker 설정
---

# /cratos-setup

Cratos 프로젝트 초기 설정을 수행합니다.

## 수행 작업

1. **Cargo Workspace 생성**
   - Cargo.toml (workspace 설정)
   - 공통 의존성 정의

2. **크레이트 스캐폴딩**
   - cratos-core: 핵심 오케스트레이션
   - cratos-channels: 채널 어댑터
   - cratos-tools: 도구 레지스트리
   - cratos-llm: LLM 프로바이더
   - cratos-replay: 리플레이 엔진

3. **설정 파일 생성**
   - .env.example
   - config/default.toml
   - .gitignore

4. **Docker 설정**
   - Dockerfile (멀티 스테이지 빌드)
   - docker-compose.yml

5. **GitHub Actions**
   - .github/workflows/ci.yml
   - .github/workflows/release.yml

## 참조 문서

- `.agent/workflows/setup.md`
- `.agent/skills/infra-agent/resources/docker-guide.md`
- `.agent/skills/infra-agent/resources/ci-cd-guide.md`

## 실행 후 확인

```bash
cargo build
cargo test
docker-compose build
```
