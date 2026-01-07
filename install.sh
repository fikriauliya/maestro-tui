#!/usr/bin/env bash
set -euo pipefail

# maestro-tui installer
# Usage: curl -fsSL https://raw.githubusercontent.com/fikriauliya/maestro-tui/main/install.sh | bash

REPO="fikriauliya/maestro-tui"
BINARY="maestro-tui"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# Detect OS and architecture - returns Rust target triple
detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64)   echo "x86_64-unknown-linux-gnu" ;;
                *)              error "Unsupported Linux architecture: $arch" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64|amd64)   echo "x86_64-apple-darwin" ;;
                aarch64|arm64)  echo "aarch64-apple-darwin" ;;
                *)              error "Unsupported macOS architecture: $arch" ;;
            esac
            ;;
        *)
            error "Unsupported OS: $os"
            ;;
    esac
}

# Get the latest release version
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
    local platform version url tmpdir

    platform="$(detect_platform)"
    info "Detected platform: $platform"

    info "Fetching latest version..."
    version="$(get_latest_version)"
    if [[ -z "$version" ]]; then
        error "Could not determine latest version"
    fi
    info "Latest version: $version"

    url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${platform}.tar.gz"
    info "Downloading from: $url"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    if ! curl -fsSL "$url" -o "$tmpdir/${BINARY}.tar.gz"; then
        error "Failed to download release. Check if the release exists for your platform."
    fi

    info "Extracting..."
    tar -xzf "$tmpdir/${BINARY}.tar.gz" -C "$tmpdir"

    # Check if we need sudo
    if [[ -w "$INSTALL_DIR" ]]; then
        mv "$tmpdir/${BINARY}" "$INSTALL_DIR/${BINARY}"
    else
        info "Installing to $INSTALL_DIR (requires sudo)..."
        sudo mv "$tmpdir/${BINARY}" "$INSTALL_DIR/${BINARY}"
    fi

    chmod +x "$INSTALL_DIR/${BINARY}"

    info "Successfully installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"
    info "Run 'maestro-tui' to get started!"
}

# Check dependencies
check_deps() {
    for cmd in curl tar; do
        if ! command -v "$cmd" &> /dev/null; then
            error "Required command not found: $cmd"
        fi
    done
}

main() {
    info "Installing maestro-tui..."
    check_deps
    install
}

main "$@"
