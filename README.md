# 領域 (Ryoiki)

A minimalist, clutter-free, and aesthetic Ubuntu server provisioning orchestrator written in Rust.

```text
  ┌─────────────────────────────────────────────────────────────┐
  │  領域 (Ryoiki) — Ubuntu Server Provisioning                 │
  └─────────────────────────────────────────────────────────────┘

  Select modules to install: (Space to toggle, Enter to run)

  [✓] 1. Git & SSH Key Setup       Ed25519 key generation & GitHub link
  [✓] 2. System Essentials         git, tmux, neovim, and GitHub CLI (gh)
  [✓] 3. Modern CLI Suite          eza, bat, zoxide, fzf, and ble.sh
  [✓] 4. Dev Runtimes              Go, Rust (rustup), Python (uv), Bun
  [✓] 5. Server Security           UFW Firewall & Fail2Ban brute-force guard
  [✓] 6. Docker Platform           Docker Engine CE & Docker Compose plugin
  [✓] 7. Prompt & Banner           Starship prompt & Fastfetch system stats
  [✓] 8. Trash Manager             toss-rs (FreeDesktop trash TUI & rm alias)
  [✓] 9. Aesthetic Dotfiles        Deploy embedded dotfiles (tmux, aliases)
```

## ✨ Highlights
- 🔇 **Zero Visual Clutter**: Hides noisy `apt`, `dpkg`, and compiler scroll walls behind clean, single-line spinners. Full output is logged quietly to `~/.local/state/ryoiki/install.log`.
- 🖥️ **Interactive Ratatui TUI**: Seamless keyboard checklist navigation (`j`/`k`, `Space` to toggle, `Enter` to run, `a` to select all).
- 📦 **Zero-Clone Embedded Dotfiles**: All configurations (`.tmux.conf`, `.bash_aliases`, `starship.toml`) are compiled directly into the binary with `include_str!`.
- ⚡ **Dual Architecture Releases**: Pre-compiled static binaries for both `x86_64` and `aarch64` (ARM / AWS Graviton / Raspberry Pi).

---

## 🚀 Quickstart (One-Liner Binary Install)

On any fresh Ubuntu server, run:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/ryoiki/main/remote-install.sh | bash
```

> **What this does:** Detects architecture (`x86_64` or `aarch64`), downloads the pre-built `ryoiki` release binary into `~/.local/bin/ryoiki`, and launches the interactive setup.

---

## 🛠️ CLI Usage & Flags

```bash
# Interactive TUI selection (default)
ryoiki

# Run all modules automatically without prompts
ryoiki --all

# Audit system to check which tools are installed
ryoiki check

# Deploy or update embedded dotfiles only
ryoiki dotfiles

# Dry-run simulation (prints actions without modifying system)
ryoiki --dry-run

# Run specific modules by ID
ryoiki run dev_runtimes docker
```

---

## 📂 Repository Structure

```text
ryoiki/
├── src/
│   ├── main.rs           # CLI argument parsing, subcommands & coordinator
│   ├── tui.rs            # Interactive Ratatui checklist interface
│   ├── runner.rs         # Silent subprocess executor & spinners
│   ├── configs.rs        # Embedded dotfiles deployment
│   └── modules/          # Modular server setup components
│       ├── git_ssh.rs    # Git global config & Ed25519 SSH setup
│       ├── essentials.rs # git, tmux, neovim, GitHub CLI keyring
│       ├── cli_tools.rs  # eza, bat, zoxide, fzf, ble.sh
│       ├── dev_runtimes.rs # Go, Rustup, uv, Bun
│       ├── security.rs   # UFW Firewall & Fail2Ban
│       ├── docker.rs     # Docker CE & Docker Compose
│       ├── prompt.rs     # Starship prompt & Fastfetch banner
│       └── trash.rs      # toss-rs installation
├── configs/              # Source configuration dotfiles
│   ├── .tmux.conf        # Mouse scrolling, 256 colors, vi mode
│   ├── .bash_aliases     # Aliases & PATH for cargo/uv/bun/zoxide/toss
│   └── starship.toml     # Minimal aesthetic prompt styling
├── .github/workflows/
│   └── release.yml       # Dual-architecture release workflow (x86_64 & aarch64)
├── remote-install.sh     # One-liner remote binary bootstrapper
└── install.sh            # Local runner (compiles with Cargo or falls back)
```

## 📜 License
Distributed under the [MIT License](LICENSE).
