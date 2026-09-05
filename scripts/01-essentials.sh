#!/usr/bin/env bash
set -euo pipefail

echo "==> Updating package lists..."
sudo apt-get update

echo "==> Installing prerequisites..."
sudo apt-get install -y curl wget ca-certificates gnupg

echo "==> Installing git, tmux, and neovim..."
sudo apt-get install -y git tmux neovim

# Install GitHub CLI (gh) from official repo if not present
if ! command -v gh &>/dev/null; then
    echo "==> Setting up official GitHub CLI (gh) repository..."
    sudo mkdir -p -m 755 /etc/apt/keyrings
    wget -qO- https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null
    sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
    sudo apt-get update
    sudo apt-get install -y gh
fi

echo "==> Cleaning up unnecessary packages..."
sudo apt-get autoremove -y
sudo apt-get clean

echo "==> Essentials installed successfully:"
git --version
tmux -V
nvim --version | head -n 1
gh --version | head -n 1
