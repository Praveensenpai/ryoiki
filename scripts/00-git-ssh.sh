#!/usr/bin/env bash
set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${CYAN}===============================================================${NC}"
echo -e "${CYAN}       領域 (Ryoiki) - Git & GitHub SSH Setup Helper          ${NC}"
echo -e "${CYAN}===============================================================${NC}"

# ------------------------------------------------------------------------------
# 1. Configure Git Global Identity
# ------------------------------------------------------------------------------
CURRENT_USER="$(git config --global user.name || true)"
CURRENT_EMAIL="$(git config --global user.email || true)"

DEFAULT_USER="${CURRENT_USER:-Praveensenpai}"
DEFAULT_EMAIL="${CURRENT_EMAIL:-pvnt20@gmail.com}"

echo -e "\n${YELLOW}==> 1. Git Identity Configuration${NC}"
read -rp "Enter Git username [default: ${DEFAULT_USER}]: " INPUT_USER
GIT_USER="${INPUT_USER:-$DEFAULT_USER}"

read -rp "Enter Git email [default: ${DEFAULT_EMAIL}]: " INPUT_EMAIL
GIT_EMAIL="${INPUT_EMAIL:-$DEFAULT_EMAIL}"

git config --global user.name "$GIT_USER"
git config --global user.email "$GIT_EMAIL"
git config --global init.defaultBranch main

echo -e "${GREEN}✓ Git identity configured:${NC} $GIT_USER <$GIT_EMAIL>"

# ------------------------------------------------------------------------------
# 2. SSH Key Generation
# ------------------------------------------------------------------------------
KEY_PATH="$HOME/.ssh/id_ed25519"
PUBKEY_PATH="${KEY_PATH}.pub"

echo -e "\n${YELLOW}==> 2. Checking SSH Key...${NC}"
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"

if [ -f "$PUBKEY_PATH" ]; then
    echo -e "${GREEN}✓ Existing SSH key found at:${NC} $PUBKEY_PATH"
else
    echo -e "Generating new ed25519 SSH key..."
    ssh-keygen -t ed25519 -C "$GIT_EMAIL" -f "$KEY_PATH" -N ""
    chmod 600 "$KEY_PATH"
    chmod 644 "$PUBKEY_PATH"
    echo -e "${GREEN}✓ SSH key generated successfully!${NC}"
fi

# ------------------------------------------------------------------------------
# 3. Display Public Key & GitHub Instructions
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}===============================================================${NC}"
echo -e "${YELLOW}${BOLD}📋 COPY YOUR PUBLIC KEY BELOW:${NC}"
echo -e "${BOLD}===============================================================${NC}"
echo ""
cat "$PUBKEY_PATH"
echo ""
echo -e "${BOLD}===============================================================${NC}"

echo -e "\n${CYAN}Follow these steps to link the key:${NC}"
echo -e " 1. Open: ${BOLD}https://github.com/settings/keys${NC}"
echo -e " 2. Click ${BOLD}'New SSH key'${NC}"
echo -e " 3. Title: e.g. ${BOLD}Ubuntu Server ($(hostname))${NC}"
echo -e " 4. Key type: ${BOLD}Authentication Key${NC}"
echo -e " 5. Paste the key displayed above and click ${BOLD}'Add SSH key'${NC}"
echo ""

read -rp "Press [Enter] once you have added the key to GitHub to test connection..."

# ------------------------------------------------------------------------------
# 4. Test GitHub Authentication
# ------------------------------------------------------------------------------
echo -e "\n${YELLOW}==> Testing GitHub SSH authentication...${NC}"

# GitHub returns exit code 1 with successful auth greeting
TEST_OUTPUT="$(ssh -T -o StrictHostKeyChecking=accept-new git@github.com 2>&1 || true)"

if echo "$TEST_OUTPUT" | grep -q "successfully authenticated"; then
    echo -e "${GREEN}${BOLD}✓ Authentication successful!${NC}"
    echo -e "${GREEN}$TEST_OUTPUT${NC}"
else
    echo -e "${YELLOW}Notice: Authentication check response:${NC}"
    echo -e "$TEST_OUTPUT"
fi

echo -e "\n${GREEN}===============================================================${NC}"
echo -e "${GREEN}  Git & SSH setup complete!                                    ${NC}"
echo -e "${GREEN}===============================================================${NC}"
