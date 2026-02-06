#!/bin/sh
# Cratos Installer for macOS and Linux
# Usage: curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
#
# Environment variables:
#   CRATOS_INSTALL_DIR - Installation directory (default: /usr/local/bin or ~/.local/bin)
#   CRATOS_VERSION     - Version to install (default: latest)
#   CRATOS_NO_WIZARD   - Skip running setup after install (default: false)
#   CRATOS_BUILD_FROM_SOURCE - Force build from source (default: false)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# GitHub repository
REPO="first-fluke/cratos"
BINARY_NAME="cratos"

# Print colored output
info() {
    printf "${BLUE}[INFO]${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}[OK]${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}[WARN]${NC} %s\n" "$1"
}

error() {
    printf "${RED}[ERROR]${NC} %s\n" "$1"
    exit 1
}

error_no_exit() {
    printf "${RED}[ERROR]${NC} %s\n" "$1"
}

# Detect OS
detect_os() {
    OS="$(uname -s)"
    case "$OS" in
        Linux*)     OS="linux" ;;
        Darwin*)    OS="darwin" ;;
        CYGWIN*|MINGW*|MSYS*) OS="windows" ;;
        *)          error "Unsupported OS: $OS" ;;
    esac
    echo "$OS"
}

# Detect architecture
detect_arch() {
    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64|amd64)   ARCH="x86_64" ;;
        aarch64|arm64)  ARCH="aarch64" ;;
        *)              error "Unsupported architecture: $ARCH" ;;
    esac
    echo "$ARCH"
}

# Get target triple
get_target() {
    OS=$(detect_os)
    ARCH=$(detect_arch)

    case "$OS-$ARCH" in
        darwin-x86_64)  echo "x86_64-apple-darwin" ;;
        darwin-aarch64) echo "aarch64-apple-darwin" ;;
        linux-x86_64)   echo "x86_64-unknown-linux-gnu" ;;
        linux-aarch64)  echo "aarch64-unknown-linux-gnu" ;;
        *)              error "Unsupported platform: $OS-$ARCH" ;;
    esac
}

# Get latest version from GitHub
get_latest_version() {
    if command -v curl > /dev/null 2>&1; then
        curl -sSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        echo ""
    fi
}

# Download file (returns 0 on success, 1 on failure)
download() {
    URL="$1"
    DEST="$2"

    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "$URL" -o "$DEST" 2>/dev/null
        return $?
    elif command -v wget > /dev/null 2>&1; then
        wget -q "$URL" -O "$DEST" 2>/dev/null
        return $?
    else
        return 1
    fi
}

# Determine installation directory
get_install_dir() {
    if [ -n "$CRATOS_INSTALL_DIR" ]; then
        echo "$CRATOS_INSTALL_DIR"
    elif [ -w "/usr/local/bin" ]; then
        echo "/usr/local/bin"
    else
        # Use ~/.local/bin if /usr/local/bin is not writable
        LOCAL_BIN="$HOME/.local/bin"
        mkdir -p "$LOCAL_BIN"
        echo "$LOCAL_BIN"
    fi
}

# Add directory to PATH if needed
add_to_path() {
    DIR="$1"

    # Check if already in PATH
    case ":$PATH:" in
        *":$DIR:"*) return 0 ;;
    esac

    # Determine shell config file
    SHELL_NAME="$(basename "$SHELL")"
    case "$SHELL_NAME" in
        bash)
            if [ -f "$HOME/.bash_profile" ]; then
                CONFIG_FILE="$HOME/.bash_profile"
            else
                CONFIG_FILE="$HOME/.bashrc"
            fi
            ;;
        zsh)
            CONFIG_FILE="$HOME/.zshrc"
            ;;
        fish)
            CONFIG_FILE="$HOME/.config/fish/config.fish"
            ;;
        *)
            CONFIG_FILE="$HOME/.profile"
            ;;
    esac

    # Add to config file (skip if already present)
    if [ -f "$CONFIG_FILE" ] && grep -qF "$DIR" "$CONFIG_FILE" 2>/dev/null; then
        return 0
    fi
    if [ -f "$CONFIG_FILE" ]; then
        echo "" >> "$CONFIG_FILE"
        echo "# Added by Cratos installer" >> "$CONFIG_FILE"
        if [ "$SHELL_NAME" = "fish" ]; then
            echo "set -gx PATH \"$DIR\" \$PATH" >> "$CONFIG_FILE"
        else
            echo "export PATH=\"$DIR:\$PATH\"" >> "$CONFIG_FILE"
        fi
        warn "Added $DIR to PATH in $CONFIG_FILE"
        warn "Please restart your shell or run: source $CONFIG_FILE"
    fi
}

# Check if Rust is installed
check_rust() {
    if command -v cargo > /dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Install Rust
install_rust() {
    info "Installing Rust..."
    if command -v curl > /dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- https://sh.rustup.rs | sh -s -- -y
    else
        error "Cannot install Rust: neither curl nor wget found."
    fi

    # Source cargo env
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
    fi
}

# Build from source
build_from_source() {
    info "Building from source..."

    # Check/install Rust
    if ! check_rust; then
        warn "Rust not found. Installing Rust first..."
        install_rust

        if ! check_rust; then
            error "Failed to install Rust. Please install manually: https://rustup.rs"
        fi
    fi

    # Create temp directory for source
    SRC_DIR=$(mktemp -d)

    info "Cloning repository..."
    if command -v git > /dev/null 2>&1; then
        if ! git clone --depth 1 "https://github.com/$REPO.git" "$SRC_DIR/cratos" 2>/dev/null; then
            rm -rf "$SRC_DIR"
            error "Failed to clone repository."
        fi
    else
        # Download as tarball if git not available
        info "Git not found, downloading source tarball..."
        if ! download "https://github.com/$REPO/archive/refs/heads/main.tar.gz" "$SRC_DIR/source.tar.gz"; then
            rm -rf "$SRC_DIR"
            error "Failed to download source."
        fi
        tar -xzf "$SRC_DIR/source.tar.gz" -C "$SRC_DIR"
        mv "$SRC_DIR/cratos-main" "$SRC_DIR/cratos"
    fi

    cd "$SRC_DIR/cratos"

    info "Compiling (this may take a few minutes)..."
    info "You'll see build progress below:"
    echo ""
    echo "  ────────────────────────────────────────────────────"
    if ! cargo build --release; then
        echo "  ────────────────────────────────────────────────────"
        echo ""
        error_no_exit "Build failed. You may need to install build dependencies:"
        echo "  macOS:  xcode-select --install"
        echo "  Ubuntu: sudo apt install build-essential pkg-config libssl-dev"
        echo "  Fedora: sudo dnf install gcc pkg-config openssl-devel"
        echo ""
        rm -rf "$SRC_DIR"
        return 1
    fi
    echo "  ────────────────────────────────────────────────────"
    echo ""

    if [ -f "target/release/$BINARY_NAME" ]; then
        cp "target/release/$BINARY_NAME" "$TMP_DIR/$BINARY_NAME"
        success "Build completed!"
        rm -rf "$SRC_DIR"
        return 0
    else
        error_no_exit "Build completed but binary not found."
        rm -rf "$SRC_DIR"
        return 1
    fi
}

# Try to download prebuilt binary
try_download_binary() {
    TARGET="$1"
    VERSION="$2"
    TMP_DIR="$3"

    ARCHIVE_NAME="cratos-${TARGET}.tar.gz"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE_NAME"

    info "Downloading from: $DOWNLOAD_URL"

    if download "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE_NAME"; then
        info "Extracting archive..."
        tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR" 2>/dev/null
        if [ -f "$TMP_DIR/$BINARY_NAME" ]; then
            return 0
        fi
    fi

    return 1
}

# Main installation
main() {
    echo ""
    echo "  ╔═══════════════════════════════════════════════════════════════╗"
    echo "  ║                                                               ║"
    echo "  ║           Cratos - AI-Powered Personal Assistant              ║"
    echo "  ║                       Installer                               ║"
    echo "  ║                                                               ║"
    echo "  ╚═══════════════════════════════════════════════════════════════╝"
    echo ""

    # Early Windows detection
    case "$(uname -s)" in
        CYGWIN*|MINGW*|MSYS*)
            error "Windows is not supported by this installer. Please build from source or use WSL."
            ;;
    esac

    # Detect platform
    TARGET=$(get_target)
    info "Detected platform: $TARGET"

    # Get installation directory
    INSTALL_DIR=$(get_install_dir)
    info "Installation directory: $INSTALL_DIR"

    # Create temporary directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    BINARY_READY=false

    # Check if forced to build from source
    if [ -n "$CRATOS_BUILD_FROM_SOURCE" ]; then
        info "Building from source (forced)..."
        build_from_source
        BINARY_READY=true
    else
        # Try to get version and download prebuilt binary
        if [ -n "$CRATOS_VERSION" ]; then
            VERSION="$CRATOS_VERSION"
        else
            info "Fetching latest version..."
            VERSION=$(get_latest_version)
        fi

        if [ -n "$VERSION" ]; then
            info "Found version: $VERSION"
            if try_download_binary "$TARGET" "$VERSION" "$TMP_DIR"; then
                BINARY_READY=true
            else
                warn "Failed to download prebuilt binary."
            fi
        else
            warn "No releases found."
        fi

        # Fallback to building from source
        if [ "$BINARY_READY" = false ]; then
            warn "Falling back to building from source..."
            echo ""
            echo "  This will:"
            echo "    1. Install Rust (if not present)"
            echo "    2. Clone the repository"
            echo "    3. Compile Cratos"
            echo ""
            echo "  This may take 5-10 minutes."
            echo ""

            build_from_source
            BINARY_READY=true
        fi
    fi

    # Install binary
    if [ "$BINARY_READY" = true ] && [ -f "$TMP_DIR/$BINARY_NAME" ]; then
        info "Installing to $INSTALL_DIR..."
        if [ -w "$INSTALL_DIR" ]; then
            mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
            chmod +x "$INSTALL_DIR/$BINARY_NAME"
        elif [ -t 0 ]; then
            warn "Elevated permissions required. Using sudo..."
            sudo mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
            sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
        else
            error_no_exit "Cannot write to $INSTALL_DIR (no interactive terminal for sudo)."
            INSTALL_DIR="$HOME/.local/bin"
            mkdir -p "$INSTALL_DIR"
            mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
            chmod +x "$INSTALL_DIR/$BINARY_NAME"
            warn "Installed to $INSTALL_DIR instead."
        fi
    else
        error "No binary to install."
    fi

    # Verify installation
    if ! command -v "$BINARY_NAME" > /dev/null 2>&1; then
        add_to_path "$INSTALL_DIR"
        export PATH="$INSTALL_DIR:$PATH"
    fi

    # Test
    if "$INSTALL_DIR/$BINARY_NAME" --version > /dev/null 2>&1; then
        success "Cratos installed successfully!"
    else
        warn "Binary installed but verification failed."
        warn "You may need to install runtime dependencies (e.g., libssl)."
        warn "Try running: $INSTALL_DIR/$BINARY_NAME --version"
    fi

    # Print version
    echo ""
    "$INSTALL_DIR/$BINARY_NAME" --version 2>/dev/null || true
    echo ""

    # Run wizard unless disabled or non-interactive
    if [ -z "$CRATOS_NO_WIZARD" ] && [ -t 0 ]; then
        echo ""
        "$INSTALL_DIR/$BINARY_NAME" init || true
    else
        echo ""
        success "Installation complete!"
        echo ""
        echo "  Next steps:"
        echo "    1. Run setup:             cratos init"
        echo "    2. Start the server:      cratos serve"
        echo ""
    fi
}

# Run main
main
