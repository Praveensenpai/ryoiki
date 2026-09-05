#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "       Configuring Server Security        "
echo "=========================================="

echo "==> Installing UFW and Fail2Ban..."
sudo apt-get update
sudo apt-get install -y ufw fail2ban

echo "==> Configuring UFW firewall rules..."
# Default policies: block all unexpected incoming traffic
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow OpenSSH (Port 22) and Web ports (80, 443)
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp comment 'HTTP'
sudo ufw allow 443/tcp comment 'HTTPS'

# Enable firewall non-interactively
echo "==> Enabling UFW..."
sudo ufw --force enable
sudo ufw status verbose

echo "==> Configuring and enabling Fail2Ban..."
if [ ! -f /etc/fail2ban/jail.local ]; then
    sudo cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local
fi

sudo systemctl enable --now fail2ban
sudo systemctl restart fail2ban

echo "==> Security setup completed successfully!"
