#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - One-Line Remote Installer
# ==============================================================================
set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}  領域 (Ryoiki) - Remote Bootstrapper                 ${NC}"
echo -e "${CYAN}======================================================${NC}"

TARGET_DIR="$HOME/ryoiki"
REPO_URL="https://github.com/Praveensenpai/ryoiki.git"

# 1. Ensure git and curl are installed
if ! command -v git &>/dev/null; then
    echo -e "${YELLOW}==> Git not found. Installing git and curl via apt...${NC}"
    sudo apt-get update
    sudo apt-get install -y git curl
fi

# 2. Clone or update repository
if [ -d "$TARGET_DIR/.git" ]; then
    echo -e "${YELLOW}==> Existing ryoiki installation detected at $TARGET_DIR. Updating...${NC}"
    git -C "$TARGET_DIR" pull --ff-only origin main || true
else
    echo -e "${GREEN}==> Cloning ryoiki into $TARGET_DIR...${NC}"
    rm -rf "$TARGET_DIR"
    git clone "$REPO_URL" "$TARGET_DIR"
fi

# 3. Make all scripts executable and run master installer
echo -e "\n${GREEN}==> Launching ryoiki installer...${NC}\n"
cd "$TARGET_DIR"
chmod +x install.sh scripts/*.sh
exec ./install.sh
