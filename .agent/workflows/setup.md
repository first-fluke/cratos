---
name: setup
description: 초기 설정 워크플로우
triggers:
  - "/setup"
  - "프로젝트 설정"
  - "초기화"
---

# /setup - 초기 설정

## Cratos 프로젝트 초기 설정

### Step 1: Cargo Workspace

```bash
# workspace 생성
cargo new cratos --name cratos
cd cratos

# Cargo.toml 설정
cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "crates/cratos-core",
    "crates/cratos-channels",
    "crates/cratos-tools",
    "crates/cratos-llm",
    "crates/cratos-replay",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.88"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tracing = "0.1"
thiserror = "1"
anyhow = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
EOF
```

### Step 2: 크레이트 생성

```bash
mkdir -p crates
cargo new crates/cratos-core --lib
cargo new crates/cratos-channels --lib
cargo new crates/cratos-tools --lib
cargo new crates/cratos-llm --lib
cargo new crates/cratos-replay --lib
```

### Step 3: 설정 파일

```bash
# .env.example
cat > .env.example << 'EOF'
DATABASE_URL=postgres://cratos:cratos@localhost:5432/cratos
REDIS_URL=redis://localhost:6379
OPENAI_API_KEY=sk-your-key
ANTHROPIC_API_KEY=sk-ant-your-key
TELOXIDE_TOKEN=your-telegram-token
SLACK_BOT_TOKEN=xoxb-your-token
SLACK_SIGNING_SECRET=your-secret
EOF

# config/default.toml
mkdir -p config
cat > config/default.toml << 'EOF'
[server]
host = "0.0.0.0"
port = 8080

[database]
max_connections = 10

[llm]
default_provider = "anthropic"
model_routing = true

[approval]
default_mode = "risky_only"
EOF
```

### Step 4: Docker 설정

```bash
# Dockerfile
cat > Dockerfile << 'EOF'
FROM rust:1.93 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/cratos /usr/local/bin/
CMD ["cratos", "serve"]
EOF

# docker-compose.yml
cat > docker-compose.yml << 'EOF'
version: "3.8"
services:
  cratos:
    build: .
    ports:
      - "8080:8080"
    env_file: .env
    depends_on:
      - db
      - redis
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: cratos
      POSTGRES_PASSWORD: cratos
      POSTGRES_DB: cratos
  redis:
    image: redis:7-alpine
EOF
```

### Step 5: GitHub Actions

```bash
mkdir -p .github/workflows
# CI 워크플로우 생성 (infra-agent 참조)
```

## 검증

```bash
# 빌드 확인
cargo build

# 테스트 확인
cargo test

# Docker 빌드
docker-compose build
```
