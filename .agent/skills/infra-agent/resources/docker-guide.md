# Docker 가이드

## Cratos 설치 특징

> **중요**: Cratos는 **PostgreSQL이 필요 없습니다!**
> - 내장 SQLite 사용 (`~/.cratos/cratos.db`)
> - 단일 바이너리 배포
> - Docker 없이도 실행 가능

## Rust 최적화 Dockerfile

```dockerfile
# syntax=docker/dockerfile:1.4

# 1. Chef 스테이지 (의존성 캐싱)
FROM rust:1.93-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# 2. Planner 스테이지
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# 3. Builder 스테이지
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# 의존성만 먼저 빌드 (캐시 활용)
RUN cargo chef cook --release --recipe-path recipe.json
# 소스 복사 후 빌드
COPY . .
RUN cargo build --release

# 4. Runtime 스테이지
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 비-root 사용자
RUN useradd -m -u 1000 cratos
USER cratos

# 데이터 디렉토리
RUN mkdir -p /home/cratos/.cratos

COPY --from=builder /app/target/release/cratos /usr/local/bin/

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["cratos"]
CMD ["serve"]
```

## docker-compose.yml (최소 구성)

```yaml
version: "3.8"

services:
  cratos:
    build: .
    ports:
      - "8080:8080"
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - TELOXIDE_TOKEN=${TELOXIDE_TOKEN}
    volumes:
      # SQLite 데이터 영속화
      - cratos_data:/home/cratos/.cratos

volumes:
  cratos_data:
```

## docker-compose.yml (Redis 추가, 선택적)

Redis는 세션 캐시/분산 환경에서만 필요합니다:

```yaml
version: "3.8"

services:
  cratos:
    build: .
    ports:
      - "8080:8080"
    environment:
      - REDIS_URL=redis://redis:6379
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - TELOXIDE_TOKEN=${TELOXIDE_TOKEN}
    volumes:
      - cratos_data:/home/cratos/.cratos
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

volumes:
  cratos_data:
  redis_data:
```

## 샌드박스 실행 (exec 도구)

보안을 위해 `exec` 도구는 Docker 샌드박스에서 실행 가능:

```yaml
services:
  cratos:
    # ...
    environment:
      - CRATOS_EXEC__SANDBOX_IMAGE=debian:bookworm-slim
      - CRATOS_EXEC__SANDBOX_MEMORY_LIMIT=512m
      - CRATOS_EXEC__SANDBOX_CPU_LIMIT=1.0
```

샌드박스 보안 플래그:
```
--network=none --read-only --pids-limit=64 --security-opt=no-new-privileges
```

## .dockerignore

```
target/
.git/
.env
.env.local
*.log
~/.cratos/
```

## 로컬 실행 (Docker 없이)

```bash
# 빌드
cargo build --release

# 실행
./target/release/cratos serve
```

데이터는 `~/.cratos/`에 자동 생성됩니다.
