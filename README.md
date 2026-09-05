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
│   └── 01-essentials.sh  # Installs git, gh, tmux, neovim
└── configs/              # Dotfiles & configurations
```
