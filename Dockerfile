# Cratos Dockerfile
# Multi-stage build for optimal image size

# ===================
# Build Stage
# ===================
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/cratos-core/Cargo.toml crates/cratos-core/
COPY crates/cratos-channels/Cargo.toml crates/cratos-channels/
COPY crates/cratos-tools/Cargo.toml crates/cratos-tools/
COPY crates/cratos-llm/Cargo.toml crates/cratos-llm/
COPY crates/cratos-replay/Cargo.toml crates/cratos-replay/
COPY crates/cratos-skills/Cargo.toml crates/cratos-skills/
COPY crates/cratos-search/Cargo.toml crates/cratos-search/
COPY crates/cratos-audio/Cargo.toml crates/cratos-audio/
COPY crates/cratos-canvas/Cargo.toml crates/cratos-canvas/

# Create dummy source files for dependency caching
RUN mkdir -p src \
    crates/cratos-core/src \
    crates/cratos-channels/src \
    crates/cratos-tools/src \
    crates/cratos-llm/src \
    crates/cratos-replay/src \
    crates/cratos-skills/src \
    crates/cratos-search/src \
    crates/cratos-audio/src \
    crates/cratos-canvas/src \
    && echo "fn main() {}" > src/main.rs \
    && echo "// dummy" > crates/cratos-core/src/lib.rs \
    && echo "// dummy" > crates/cratos-channels/src/lib.rs \
    && echo "// dummy" > crates/cratos-tools/src/lib.rs \
    && echo "// dummy" > crates/cratos-llm/src/lib.rs \
    && echo "// dummy" > crates/cratos-replay/src/lib.rs \
    && echo "// dummy" > crates/cratos-skills/src/lib.rs \
    && echo "// dummy" > crates/cratos-search/src/lib.rs \
    && echo "// dummy" > crates/cratos-audio/src/lib.rs \
    && echo "// dummy" > crates/cratos-canvas/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src crates

# Copy actual source code
COPY src ./src
COPY crates ./crates
COPY config ./config

# Touch files to invalidate cache for actual build
RUN find src crates -name "*.rs" -exec touch {} \;

# Build the actual binary
RUN cargo build --release

# ===================
# Runtime Stage
# ===================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 cratos

# Copy binary from builder
COPY --from=builder /app/target/release/cratos /app/cratos

# Copy config
COPY --from=builder /app/config/default.toml /app/config/default.toml
COPY --from=builder /app/config/pantheon /app/config/pantheon
COPY --from=builder /app/config/decrees /app/config/decrees

# Set ownership
RUN chown -R cratos:cratos /app

USER cratos

# Create data directory
RUN mkdir -p /home/cratos/.cratos

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

# Run the binary
ENTRYPOINT ["/app/cratos"]
