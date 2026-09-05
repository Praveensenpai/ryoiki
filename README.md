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
│   ├── 01-essentials.sh  # Installs git, gh, tmux, neovim
│   └── 02-cli-tools.sh   # Installs eza, bat, zoxide, ble.sh & links configs
└── configs/              # Dotfiles & configurations
    ├── .tmux.conf        # Mouse scrolling, 256 colors, vi mode
    └── .bash_aliases     # Aliases: ls->eza, cat->bat, cd->z (zoxide)
```
