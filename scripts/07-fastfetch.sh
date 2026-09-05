#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "          Installing Fastfetch            "
echo "=========================================="

echo "==> Installing fastfetch..."
sudo apt-get update
sudo apt-get install -y fastfetch

# Add fastfetch to ~/.bashrc for interactive logins if not present
if ! grep -q "fastfetch" "$HOME/.bashrc"; then
    echo "==> Adding fastfetch login banner to ~/.bashrc..."
    cat << 'EOF' >> "$HOME/.bashrc"

# Display fastfetch system banner on interactive terminal start
if [[ $- == *i* ]] && command -v fastfetch &>/dev/null; then
    fastfetch
fi
EOF
fi

echo "==> Fastfetch installed successfully:"
fastfetch --version
