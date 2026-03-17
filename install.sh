#!/bin/sh
# Symphony installer — https://github.com/broomva/symphony
# Usage: curl -fsSL https://raw.githubusercontent.com/broomva/symphony/master/install.sh | sh
set -e

REPO="broomva/symphony"
INSTALL_DIR="${SYMPHONY_INSTALL_DIR:-/usr/local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)   PLATFORM="linux" ;;
  Darwin)  PLATFORM="macos" ;;
  *)       echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  ARCH_SUFFIX="amd64" ;;
  arm64|aarch64) ARCH_SUFFIX="arm64" ;;
  *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ARTIFACT="symphony-${PLATFORM}-${ARCH_SUFFIX}"

# Get latest release tag
echo "Fetching latest release..."
TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$TAG" ]; then
  echo "No release found. Install from source instead:"
  echo "  cargo install symphony-cli"
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ARTIFACT}"
echo "Downloading Symphony ${TAG} for ${PLATFORM}/${ARCH_SUFFIX}..."

# Download to temp file
TMP=$(mktemp)
if ! curl -fsSL "$URL" -o "$TMP"; then
  echo ""
  echo "Binary not found for ${PLATFORM}/${ARCH_SUFFIX}."
  echo "Install from source instead:"
  echo "  cargo install symphony-cli"
  rm -f "$TMP"
  exit 1
fi

chmod +x "$TMP"

# Install
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP" "$INSTALL_DIR/symphony"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "$TMP" "$INSTALL_DIR/symphony"
fi

echo ""
echo "Symphony ${TAG} installed to ${INSTALL_DIR}/symphony"
echo ""
echo "Get started:"
echo "  symphony init                    # scaffold a WORKFLOW.md"
echo "  symphony validate WORKFLOW.md    # verify config"
echo "  symphony start WORKFLOW.md       # run the daemon"
echo ""
echo "Docs: https://github.com/${REPO}"
