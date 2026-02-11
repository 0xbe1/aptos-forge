#!/bin/sh
set -e

REPO="0xbe1/aptly"
BINARY="aptly"
INSTALL_DIR="/usr/local/bin"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    darwin) OS="darwin" ;;
    linux) OS="linux" ;;
    *) echo "Unsupported OS: $OS" && exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH="amd64" ;;
    arm64|aarch64) ARCH="arm64" ;;
    *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
esac

# Get latest aptly-cli release tag
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100" \
  | grep -o '"tag_name":[[:space:]]*"aptly-cli-v[^"]*"' \
  | head -n1 \
  | sed -E 's/.*"([^"]+)"/\1/')

if [ -z "$TAG" ]; then
    echo "Failed to fetch latest aptly-cli release tag"
    exit 1
fi

# Release assets for aptly include the full tag string.
VERSION="$TAG"

echo "Installing $BINARY ${VERSION} (${TAG}) for ${OS}/${ARCH}..."

# Download and extract
ARCHIVE="${BINARY}_${VERSION}_${OS}_${ARCH}.tar.gz"
URL="https://github.com/$REPO/releases/download/$TAG/$ARCHIVE"

curl -fsSL "$URL" | tar xz

# Install
if [ -w "$INSTALL_DIR" ]; then
    mv "$BINARY" "$INSTALL_DIR/"
else
    echo "Installing to $INSTALL_DIR (requires sudo)..."
    sudo mv "$BINARY" "$INSTALL_DIR/"
fi

echo "Installed $BINARY $VERSION to $INSTALL_DIR/$BINARY"
