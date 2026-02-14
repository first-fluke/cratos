#!/bin/bash
# Build Web UI for Cratos
#
# Prerequisites:
#   - Rust with cargo installed
#   - trunk (cargo install trunk)
#   - wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
WEB_DIR="$PROJECT_ROOT/apps/web"

echo "Building Web UI..."

if [ ! -d "$WEB_DIR" ]; then
    echo "Error: Web UI directory not found at $WEB_DIR"
    echo "Please create the Web UI project first."
    exit 1
fi

cd "$WEB_DIR"

# Check if trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "Installing trunk..."
    cargo install trunk
fi

# Build the Web UI
trunk build --release

echo ""
echo "Web UI built successfully!"
echo "Output: $WEB_DIR/dist/"
echo ""
echo "Files:"
ls -la "$WEB_DIR/dist/" 2>/dev/null || echo "(dist directory will be created on build)"
