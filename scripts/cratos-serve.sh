#!/bin/bash
# Cratos server startup script for LaunchAgent
# Waits for Redis to be ready, then starts cratos serve

export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
export RUST_LOG=info

CRATOS_BIN="/Volumes/gahyun_ex/projects/cratos/target/release/cratos"
CRATOS_DIR="/Volumes/gahyun_ex/projects/cratos"
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
