#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - Remote Binary Bootstrapper
# ==============================================================================
set -euo pipefail

REPO="Praveensenpai/ryoiki"
BINARY="ryoiki"
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "🌸 ========================================= 🌸"
echo "        領域 (Ryoiki) Server Setup            "
echo "🌸 ========================================= 🌸"

# Detect Architecture (x86_64 or aarch64)
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)
        TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        TARGET="aarch64-unknown-linux-gnu"
        ;;
    *)
        echo "❌ Architecture $ARCH is not supported."
        exit 1
        ;;
esac

# 1. Fetch latest release tag from GitHub
TAG=$(curl -4 -sSL -H "Cache-Control: no-cache" -H "Pragma: no-cache" "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [ -n "$TAG" ]; then
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/ryoiki-${TARGET}.tar.gz"
    echo "==> Downloading pre-compiled ryoiki (${TARGET} - $TAG)..."
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    if curl -4 -fsSL "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR" 2>/dev/null; then
        install -m 755 "$TMP_DIR/$BINARY" "$INSTALL_DIR/$BINARY"
        echo "✔ Installed ryoiki binary to $INSTALL_DIR/$BINARY"
    else
        echo "⚠️  Failed to download release binary. Falling back to local clone..."
        TAG=""
    fi
fi

# 2. Fallback if no release tag found yet
if [ -z "${TAG:-}" ]; then
    TARGET_DIR="$HOME/ryoiki"
    if ! command -v git &>/dev/null; then
        echo "==> Installing git and curl via apt..."
        sudo apt-get update -y
        sudo apt-get install -y git curl
    fi

    if [ -d "$TARGET_DIR/.git" ]; then
        git -C "$TARGET_DIR" pull --ff-only origin main || true
    else
        rm -rf "$TARGET_DIR"
        git clone "https://github.com/$REPO.git" "$TARGET_DIR"
    fi

    cd "$TARGET_DIR"
    if ! command -v cargo &>/dev/null; then
        echo "==> Installing minimal Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
        if [ -f "$HOME/.cargo/env" ]; then
            # shellcheck source=/dev/null
            source "$HOME/.cargo/env"
        fi
    fi

    echo "==> Building ryoiki with Cargo..."
    cargo build --release
    install -m 755 target/release/ryoiki "$INSTALL_DIR/$BINARY"
fi

# Ensure ~/.local/bin is in PATH for this session
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) export PATH="$INSTALL_DIR:$PATH" ;;
esac

# 3. Authenticate sudo upfront if needed
if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    if ! sudo -n true 2>/dev/null; then
        echo "==> Sudo privileges required for server provisioning. Please authenticate:"
        if [ -c /dev/tty ]; then
            sudo -v < /dev/tty
        else
            sudo -v
        fi
    fi
fi

# 4. Launch ryoiki with terminal input attached
if [ -c /dev/tty ]; then
    exec "$INSTALL_DIR/$BINARY" "$@" < /dev/tty
else
    exec "$INSTALL_DIR/$BINARY" "$@"
fi
