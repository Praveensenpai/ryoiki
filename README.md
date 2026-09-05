# 領域 (Ryoiki)

Personal Ubuntu server provisioning and setup scripts.

## Quickstart (Fresh Server)

To replicate this environment on a fresh Ubuntu server:

```bash
git clone git@github.com:Praveensenpai/ryoiki.git
cd ryoiki
bash install.sh
```

## Structure

```text
ryoiki/
├── install.sh            # Master setup script
├── scripts/
│   ├── 00-git-ssh.sh     # Interactive Git config & GitHub SSH key setup
│   ├── 01-essentials.sh  # Installs git, gh, tmux, neovim
│   ├── 02-cli-tools.sh   # Installs eza, bat, zoxide, ble.sh & links configs
│   ├── 03-dev-tools.sh   # Installs Golang, Rust (rustup), and uv
│   ├── 04-security.sh    # UFW firewall & Fail2Ban brute-force protection
│   ├── 05-docker.sh      # Official Docker Engine & Compose plugin
│   ├── 06-starship.sh    # Starship cross-shell prompt
│   └── 07-fastfetch.sh   # Fastfetch system info login banner
└── configs/              # Dotfiles & configurations
    ├── .tmux.conf        # Mouse scrolling, 256 colors, vi mode
    ├── .bash_aliases     # Aliases & PATH for cargo/uv/zoxide
    └── starship.toml     # Minimal aesthetic prompt styling
```
