#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - Remote Binary Bootstrapper
#  Usage: curl -fsSL https://raw.githubusercontent.com/Praveensenpai/ryoiki/main/remote-install.sh | bash
# ==============================================================================
set -euo pipefail

REPO="Praveensenpai/ryoiki"
BINARY="ryoiki"
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "🌸 ========================================= 🌸"
echo "        領域 (Ryoiki) Server Setup            "
echo "🌸 ========================================= 🌸"

# ── Detect architecture ────────────────────────────────────────────────────────
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)   TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64)  TARGET="aarch64-unknown-linux-gnu" ;;
    *)
        echo "❌ Architecture $ARCH is not supported."
        exit 1
        ;;
esac

# ── Fetch latest release tag ───────────────────────────────────────────────────
echo "==> Fetching latest release..."
TAG=$(curl -4 -sSL \
    -H "Cache-Control: no-cache" \
    -H "Pragma: no-cache" \
    "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | grep '"tag_name":' \
    | sed -E 's/.*"([^"]+)".*/\1/' \
    || true)

if [ -z "${TAG:-}" ]; then
    echo "❌ Could not fetch latest release tag from GitHub."
    echo "   Check your internet connection or visit: https://github.com/$REPO/releases"
    exit 1
fi

# ── Download and install binary ────────────────────────────────────────────────
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/ryoiki-${TARGET}.tar.gz"
echo "==> Downloading ryoiki ${TAG} (${TARGET})..."

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

if ! curl -4 -fsSL "$DOWNLOAD_URL" | tar -xz -C "$TMP_DIR" 2>/dev/null; then
    echo "❌ Failed to download release binary from:"
    echo "   $DOWNLOAD_URL"
    echo "   Visit https://github.com/$REPO/releases to download manually."
    exit 1
fi

install -m 755 "$TMP_DIR/$BINARY" "$INSTALL_DIR/$BINARY"
echo "✔ Installed ryoiki ${TAG} to $INSTALL_DIR/$BINARY"

# ── Ensure ~/.local/bin is in PATH ─────────────────────────────────────────────
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) export PATH="$INSTALL_DIR:$PATH" ;;
esac

if [ -f "$HOME/.bashrc" ] && ! grep -q '\.local/bin' "$HOME/.bashrc"; then
    printf '\n# User local binaries\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$HOME/.bashrc"
fi

# ── Symlink to /usr/local/bin for global access ────────────────────────────────
if [ "${EUID:-$(id -u)}" -eq 0 ]; then
    ln -sf "$INSTALL_DIR/$BINARY" "/usr/local/bin/$BINARY" 2>/dev/null || true
elif command -v sudo &>/dev/null && sudo -n true 2>/dev/null; then
    sudo ln -sf "$INSTALL_DIR/$BINARY" "/usr/local/bin/$BINARY" 2>/dev/null || true
fi

# ── Authenticate sudo upfront if needed ───────────────────────────────────────
if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    if ! sudo -n true 2>/dev/null; then
        echo "==> Sudo privileges required. Please authenticate:"
        if [ -c /dev/tty ]; then
            sudo -v < /dev/tty
        else
            sudo -v
        fi
    fi
fi

# ── Install system hooks (udev power rule + systemd services) ─────────────────
echo "==> Installing notification hooks..."
if command -v sudo &>/dev/null && sudo -n true 2>/dev/null; then
    sudo "$INSTALL_DIR/$BINARY" notify install-hooks 2>/dev/null || true
elif [ "${EUID:-$(id -u)}" -eq 0 ]; then
    "$INSTALL_DIR/$BINARY" notify install-hooks 2>/dev/null || true
else
    "$INSTALL_DIR/$BINARY" notify install-hooks 2>/dev/null || true
fi

# ── Launch ryoiki TUI ──────────────────────────────────────────────────────────
if [ -c /dev/tty ]; then
    exec "$INSTALL_DIR/$BINARY" "$@" < /dev/tty
else
    exec "$INSTALL_DIR/$BINARY" "$@"
fi
