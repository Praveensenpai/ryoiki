#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "    Installing Docker & Docker Compose    "
echo "=========================================="

if command -v docker &>/dev/null; then
    echo "==> Docker is already installed:"
    docker --version
    exit 0
fi

echo "==> Installing prerequisites..."
sudo apt-get update
sudo apt-get install -y ca-certificates curl gnupg lsb-release

echo "==> Setting up official Docker GPG key..."
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor --yes -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

UBUNTU_CODENAME="$(lsb_release -cs 2>/dev/null || grep VERSION_CODENAME /etc/os-release | cut -d= -f2)"

echo "==> Adding Docker APT repository for ${UBUNTU_CODENAME}..."
echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
  ${UBUNTU_CODENAME} stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

echo "==> Installing Docker Engine, CLI, and Compose plugin..."
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

# Enable running docker without sudo for current non-root user
if [ "$USER" != "root" ]; then
    sudo usermod -aG docker "$USER"
    echo "==> Added user '$USER' to the 'docker' group."
fi

# Enable and start Docker service
sudo systemctl enable --now docker

echo "==> Docker installed successfully:"
docker --version
docker compose version
echo "(Note: To use docker without sudo, log out and log back in, or run 'newgrp docker')"
