#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIGS_DIR="$SCRIPT_DIR/configs"

echo "==> Installing eza, bat, zoxide, and fzf..."
sudo apt-get update
sudo apt-get install -y eza bat zoxide fzf curl tar xz-utils

# Fix bat naming on Ubuntu (batcat -> bat)
if command -v batcat &>/dev/null && ! command -v bat &>/dev/null; then
    echo "==> Creating symlink /usr/local/bin/bat -> /usr/bin/batcat..."
    sudo ln -sf /usr/bin/batcat /usr/local/bin/bat
fi

# Setup ble.sh (Bash Line Editor)
echo "==> Setting up ble.sh (Bash Line Editor)..."
BLESH_DIR="$HOME/.local/share/blesh"
if [ ! -d "$BLESH_DIR" ]; then
    mkdir -p "$BLESH_DIR"
    echo "==> Downloading ble.sh nightly release..."
    curl -fsSL https://github.com/akinomyoga/ble.sh/releases/download/nightly/ble-nightly.tar.xz | tar -xJ -C "$BLESH_DIR" --strip-components=1
    echo "==> ble.sh installed to $BLESH_DIR"
else
    echo "==> ble.sh is already installed at $BLESH_DIR"
fi

# Add ble.sh to ~/.bashrc if not present
if ! grep -q "blesh/ble.sh" "$HOME/.bashrc"; then
    echo "==> Adding ble.sh initialization to ~/.bashrc..."
    cat << 'EOF' >> "$HOME/.bashrc"

# ble.sh (Bash Line Editor)
[[ $- == *i* ]] && source ~/.local/share/blesh/ble.sh --attach=none
[[ ${BLE_VERSION-} ]] && ble-attach
EOF
fi

# Symlink configs
echo "==> Symlinking configuration files..."
ln -sf "$CONFIGS_DIR/.tmux.conf" "$HOME/.tmux.conf"
ln -sf "$CONFIGS_DIR/.bash_aliases" "$HOME/.bash_aliases"

echo "==> CLI tools and configs setup complete!"
echo "    - tmux: mouse scrolling & vi-mode configured (~/.tmux.conf)"
echo "    - eza: aliased as 'ls' and 'll'"
echo "    - bat: aliased as 'cat'"
echo "    - zoxide: installed and aliased to 'cd' with autocompletion"
echo "    - ble.sh: installed and enabled for bash"
