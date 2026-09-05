#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - Ubuntu Server Provisioning
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v cargo &>/dev/null; then
    echo "==> Cargo not found. Installing minimal Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
fi

echo "==> Building and running ryoiki (Rust orchestrator)..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" --quiet
exec "$SCRIPT_DIR/target/release/ryoiki" "$@"
