#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - toss-rs (FreeDesktop.org trash manager & TUI)
# ==============================================================================
set -euo pipefail

echo "=========================================="
echo "         Installing toss (toss-rs)        "
echo "=========================================="

# Ensure ~/.local/bin is in PATH for this session
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) export PATH="$HOME/.local/bin:$PATH" ;;
esac

if ! command -v toss &>/dev/null; then
    echo "==> Installing toss via official curl installer..."
    curl -fsSL -H "Cache-Control: no-cache" https://raw.githubusercontent.com/Praveensenpai/toss-rs/main/install.sh | bash
else
    echo "==> toss is already installed."
fi

echo "==> toss installation complete!"
if command -v toss &>/dev/null; then
    toss --version
elif [ -x "$HOME/.local/bin/toss" ]; then
    "$HOME/.local/bin/toss" --version
fi
