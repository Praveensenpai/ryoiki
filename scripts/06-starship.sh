#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_DIR="$SCRIPT_DIR/configs"

echo "=========================================="
echo "        Installing Starship Prompt        "
echo "=========================================="

if ! command -v starship &>/dev/null; then
    echo "==> Installing Starship cross-shell prompt..."
    curl -sS https://starship.rs/install.sh | sh -s -- -y
else
    echo "==> Starship is already installed."
fi

# Ensure ~/.config exists and symlink starship.toml
mkdir -p "$HOME/.config"
ln -sf "$CONFIG_DIR/starship.toml" "$HOME/.config/starship.toml"
echo "==> Linked starship.toml -> $HOME/.config/starship.toml"

# Add Starship initialization to ~/.bashrc if missing
if ! grep -q "starship init bash" "$HOME/.bashrc"; then
    echo "==> Adding Starship hook to ~/.bashrc..."
    echo 'eval "$(starship init bash)"' >> "$HOME/.bashrc"
fi

echo "==> Starship prompt setup complete!"
starship --version
