#!/bin/sh
# Cratos Installer for macOS and Linux
# Usage: curl -sSL https://raw.githubusercontent.com/first-fluke/cratos/main/scripts/install.sh | sh
#
# Environment variables:
#   CRATOS_INSTALL_DIR - Installation directory (default: /usr/local/bin or ~/.local/bin)
#   CRATOS_VERSION     - Version to install (default: latest)
#   CRATOS_NO_WIZARD   - Skip running wizard after install (default: false)

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
        curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download file
download() {
    URL="$1"
    DEST="$2"

    if command -v curl > /dev/null 2>&1; then
        curl -sSL "$URL" -o "$DEST"
    elif command -v wget > /dev/null 2>&1; then
        wget -q "$URL" -O "$DEST"
    else
        error "Neither curl nor wget found."
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

    # Add to config file
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

    # Detect platform
    TARGET=$(get_target)
    info "Detected platform: $TARGET"

    # Get version
    if [ -n "$CRATOS_VERSION" ]; then
        VERSION="$CRATOS_VERSION"
    else
        info "Fetching latest version..."
        VERSION=$(get_latest_version)
        if [ -z "$VERSION" ]; then
            error "Failed to fetch latest version. Please specify CRATOS_VERSION."
        fi
    fi
    info "Installing version: $VERSION"

    # Get installation directory
    INSTALL_DIR=$(get_install_dir)
    info "Installation directory: $INSTALL_DIR"

    # Create temporary directory
    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    # Download binary
    ARCHIVE_NAME="cratos-${TARGET}.tar.gz"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE_NAME"
    info "Downloading from: $DOWNLOAD_URL"

    download "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE_NAME" || {
        error "Failed to download. Check if the release exists at: https://github.com/$REPO/releases"
    }

    # Extract
    info "Extracting archive..."
    tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"

    # Install
    info "Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    else
        warn "Elevated permissions required. Using sudo..."
        sudo mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
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
        error "Installation verification failed."
    fi

    # Print version
    echo ""
    "$INSTALL_DIR/$BINARY_NAME" --version 2>/dev/null || true
    echo ""

    # Run wizard unless disabled
    if [ -z "$CRATOS_NO_WIZARD" ]; then
        echo ""
        info "Starting setup wizard..."
        echo ""
        "$INSTALL_DIR/$BINARY_NAME" wizard || true
    else
        echo ""
        success "Installation complete!"
        echo ""
        echo "  Next steps:"
        echo "    1. Run the setup wizard:  cratos wizard"
        echo "    2. Or run init:           cratos init"
        echo "    3. Start the server:      cratos serve"
        echo ""
    fi
}

# Run main
main
