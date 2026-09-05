#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "  Installing Dev Runtimes & Package Tools "
echo "=========================================="

# 1. Golang & Prerequisites
echo "==> Installing Golang, build essentials, and unzip..."
sudo apt-get update
sudo apt-get install -y golang-go build-essential unzip

# 2. Rust (via official rustup)
if ! command -v rustc &>/dev/null; then
    echo "==> Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
else
    echo "==> Rust is already installed."
fi

# Source cargo environment for this script and ensure it is in ~/.bashrc
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
    if ! grep -q "cargo/env" "$HOME/.bashrc"; then
        echo 'source "$HOME/.cargo/env"' >> "$HOME/.bashrc"
    fi
fi

# 3. uv (Python package & project manager)
if ! command -v uv &>/dev/null; then
    echo "==> Installing uv (Astral Python package manager)..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
else
    echo "==> uv is already installed."
fi

# 4. Bun (JavaScript & TypeScript runtime and toolkit)
if ! command -v bun &>/dev/null; then
    echo "==> Installing Bun..."
    curl -fsSL https://bun.sh/install | bash
else
    echo "==> Bun is already installed."
fi

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.bun/bin:$PATH"

echo "==> Dev tools installed successfully:"
go version
rustc --version
cargo --version
uv --version
bun --version
