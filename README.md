# 領域 (Ryoiki)

Personal Ubuntu server provisioning and setup scripts.

## Quickstart (One-liner Remote Install)

On any fresh Ubuntu server, run:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/ryoiki/main/remote-install.sh | bash
```

> **What this does:** Ensures `curl` and `git` are installed, clones `ryoiki`, and automatically runs the full provisioning pipeline.

---

### Manual Clone (Alternative)

```bash
git clone https://github.com/Praveensenpai/ryoiki.git
cd ryoiki
./install.sh
```

## Structure

```text
ryoiki/
├── remote-install.sh     # One-liner curl bootstrap script
├── install.sh            # Master setup script (runs all 0X scripts)
├── scripts/
│   ├── 00-git-ssh.sh     # Interactive Git config & GitHub SSH key setup
│   ├── 01-essentials.sh  # Installs git, gh, tmux, neovim
│   ├── 02-cli-tools.sh   # Installs eza, bat, zoxide, ble.sh & links configs
│   ├── 03-dev-tools.sh   # Installs Golang, Rust (rustup), uv, and Bun
│   ├── 04-security.sh    # UFW firewall & Fail2Ban brute-force protection
│   ├── 05-docker.sh      # Official Docker Engine & Compose plugin
│   ├── 06-starship.sh    # Starship cross-shell prompt
│   ├── 07-fastfetch.sh   # Fastfetch system info login banner
│   └── 08-toss.sh        # Installs toss-rs (FreeDesktop trash TUI & rm replacement)
└── configs/              # Dotfiles & configurations
    ├── .tmux.conf        # Mouse scrolling, 256 colors, vi mode
    ├── .bash_aliases     # Aliases & PATH for cargo/uv/bun/zoxide/toss
    └── starship.toml     # Minimal aesthetic prompt styling
```
