# Cratos Dockerfile
# Multi-stage build for optimal image size

# ===================
# Build Stage
# ===================
FROM rust:1.75-bookworm AS builder

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

# Create dummy source files for dependency caching
RUN mkdir -p src crates/cratos-core/src crates/cratos-channels/src \
    crates/cratos-tools/src crates/cratos-llm/src crates/cratos-replay/src \
    && echo "fn main() {}" > src/main.rs \
    && echo "// dummy" > crates/cratos-core/src/lib.rs \
    && echo "// dummy" > crates/cratos-channels/src/lib.rs \
    && echo "// dummy" > crates/cratos-tools/src/lib.rs \
    && echo "// dummy" > crates/cratos-llm/src/lib.rs \
    && echo "// dummy" > crates/cratos-replay/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src crates

# Copy actual source code
COPY . .

# Touch files to invalidate cache for actual build
RUN touch src/main.rs \
    && touch crates/cratos-core/src/lib.rs \
    && touch crates/cratos-channels/src/lib.rs \
    && touch crates/cratos-tools/src/lib.rs \
    && touch crates/cratos-llm/src/lib.rs \
    && touch crates/cratos-replay/src/lib.rs

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
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 cratos

# Copy binary from builder
COPY --from=builder /app/target/release/cratos /app/cratos

# Copy config
COPY config/default.toml /app/config/default.toml

# Set ownership
RUN chown -R cratos:cratos /app

USER cratos

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

# Run the binary
ENTRYPOINT ["/app/cratos"]
