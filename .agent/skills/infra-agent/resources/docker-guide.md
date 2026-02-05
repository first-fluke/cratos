# Docker 가이드

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

COPY --from=builder /app/target/release/cratos /usr/local/bin/

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["cratos"]
CMD ["serve"]
```

## docker-compose.yml

```yaml
version: "3.8"

services:
  cratos:
    build: .
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgres://cratos:cratos@db:5432/cratos
      - REDIS_URL=redis://redis:6379
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - TELOXIDE_TOKEN=${TELOXIDE_TOKEN}
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_started

  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: cratos
      POSTGRES_PASSWORD: cratos
      POSTGRES_DB: cratos
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U cratos"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
```

## .dockerignore

```
target/
.git/
.env
.env.local
*.log
```
