#!/usr/bin/env bash
set -euo pipefail

REPO="yetone/ctxgrep"
BINARY="ctxgrep"
INSTALL_DIR="${CTXGREP_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "For Windows, please download from: https://github.com/$REPO/releases/latest"
            exit 1
            ;;
        *)
            echo "Unsupported OS: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    # x86_64-apple-darwin is not supported (no ONNX Runtime prebuilt binary)
    if [ "$arch" = "x86_64" ] && [ "$os" = "apple-darwin" ]; then
        echo "Error: x86_64 macOS is not supported. Please use an Apple Silicon Mac."
        exit 1
    fi

    echo "${arch}-${os}"
}

# Get latest release tag
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/'
}

main() {
    echo "Installing ctxgrep..."

    local platform version url tmpdir

    platform="$(detect_platform)"
    version="$(get_latest_version)"

    if [ -z "$version" ]; then
        echo "Error: Could not determine latest version."
        echo "Please check: https://github.com/$REPO/releases"
        exit 1
    fi

    echo "  Version:  $version"
    echo "  Platform: $platform"
    echo "  Install:  $INSTALL_DIR/$BINARY"

    url="https://github.com/$REPO/releases/download/$version/$BINARY-$platform.tar.gz"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    echo "Downloading $url ..."
    curl -fsSL "$url" -o "$tmpdir/release.tar.gz"

    echo "Extracting..."
    tar xzf "$tmpdir/release.tar.gz" -C "$tmpdir"

    echo "Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
    else
        sudo mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
    fi
    chmod +x "$INSTALL_DIR/$BINARY"

    echo ""
    echo "ctxgrep $version installed successfully!"
    echo ""
    echo "Get started:"
    echo "  ctxgrep index ~/notes --recursive"
    echo "  ctxgrep search \"your query\""
    echo "  ctxgrep doctor"
}

main "$@"
