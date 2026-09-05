#!/usr/bin/env bash
# ==============================================================================
#  領域 (Ryoiki) - Ubuntu Server Provisioning
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPTS_DIR="$SCRIPT_DIR/scripts"

echo "=========================================="
echo "      領域 (Ryoiki) Server Setup          "
echo "=========================================="

for script in "$SCRIPTS_DIR"/[0-9]*.sh; do
    if [ -f "$script" ]; then
        echo -e "\n==> Running $(basename "$script")..."
        bash "$script"
    fi
done

echo -e "\n=========================================="
echo "  All setup scripts finished successfully! "
echo "=========================================="
