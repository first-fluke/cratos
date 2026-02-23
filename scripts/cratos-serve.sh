#!/bin/bash
# Cratos server startup script for LaunchAgent
# Waits for Redis to be ready, then starts cratos serve

export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export RUST_LOG=info

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATOS_BIN="${CRATOS_DIR}/target/release/cratos"

# Allow override via environment
CRATOS_BIN="${CRATOS_BIN_OVERRIDE:-$CRATOS_BIN}"
LOG_FILE="/tmp/cratos_serve.log"

cd "$CRATOS_DIR" || exit 1

# Load environment variables (.env)
if [ -f "$CRATOS_DIR/.env" ]; then
    set -a
    source "$CRATOS_DIR/.env"
    set +a
fi

# Wait for Redis (up to 60 seconds)
for i in $(seq 1 60); do
    if redis-cli ping >/dev/null 2>&1; then
        echo "$(date): Redis ready" >> "$LOG_FILE"
        break
    fi
    sleep 1
done

if ! redis-cli ping >/dev/null 2>&1; then
    echo "$(date): WARNING - Redis not available, starting anyway" >> "$LOG_FILE"
fi

# Start cratos
exec "$CRATOS_BIN" serve >> "$LOG_FILE" 2>&1
