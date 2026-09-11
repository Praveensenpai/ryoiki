<div align="center">

# 🌸 領域 (Ryoiki)

**Minimalist, zero-clutter Ubuntu server provisioning orchestrator written in Rust.**

[![Release](https://img.shields.io/github/v/release/Praveensenpai/ryoiki?style=flat-square&color=cba6f7)](https://github.com/Praveensenpai/ryoiki/releases/latest)
[![CI Status](https://img.shields.io/github/actions/workflow/status/Praveensenpai/ryoiki/ci.yml?branch=main&label=CI&style=flat-square&color=a6e3a1)](https://github.com/Praveensenpai/ryoiki/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-89b4fa?style=flat-square)](LICENSE)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021%20Edition-f38ba8?style=flat-square&logo=rust&logoColor=white)](Cargo.toml)
[![Platform: Ubuntu](https://img.shields.io/badge/Platform-Ubuntu%20%7C%20Debian-fab387?style=flat-square&logo=ubuntu&logoColor=white)](https://ubuntu.com/)
[![Arch: x86_64 | aarch64](https://img.shields.io/badge/Arch-x86__64%20%7C%20aarch64-94e2d5?style=flat-square)](https://github.com/Praveensenpai/ryoiki/releases)

</div>

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  領域 (Ryoiki) — Ubuntu Server Provisioning                             │
└─────────────────────────────────────────────────────────────────────────┘

Select modules to install: (Space to toggle, Enter to run)

  [✓]  1. Git & SSH Key Setup        Ed25519 key generation & GitHub link
  [✓]  2. System Essentials          git, tmux, neovim, adb, and GitHub CLI (gh)
  [✓]  3. Modern CLI Suite           eza, bat, zoxide, fzf, and ble.sh
  [✓]  4. Dev Runtimes               Go, Rust (rustup), Python (uv), Bun
  [✓]  5. Server Security            UFW Firewall (ports 22, 80, 443)
  [✓]  6. Docker Platform            Docker Engine CE & Docker Compose plugin
  [ ]  7. Jellyfin Media Server      Dockerized media streaming with Intel GPU
  [ ]  8. qBittorrent Server         Dockerized BitTorrent client with Web UI
  [✓]  9. Shell Prompt               Starship cross-shell prompt & Fastfetch CLI
  [✓] 10. Trash Manager              toss-rs (FreeDesktop trash TUI & rm alias)
  [✓] 11. Tailscale Mesh VPN         WireGuard mesh & MagicDNS (hostname SSH)
  [✓] 12. Aesthetic Dotfiles         Deploy embedded dotfiles (tmux, aliases)

  [↑/↓/j/k] Navigate   [Space] Toggle   [a] All   [n] None   [Enter] Launch   [q] Quit
```

---

## ⚡ Quickstart

Bootstrap any fresh Ubuntu / Debian server with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/ryoiki/main/remote-install.sh | bash
```

> [!TIP]
> **Zero build dependencies required:** The script automatically detects your CPU architecture (`x86_64` or `aarch64`), streams the pre-compiled standalone binary from the latest GitHub Release to `~/.local/bin/ryoiki`, and immediately boots into the interactive TUI.

---

## ✨ Features

- 🔇 **Zero Visual Clutter** — Hides noisy `apt`, `dpkg`, and compiler scroll walls behind clean, single-line spinners. Full execution logs are saved quietly to `~/.local/state/ryoiki/install.log`.
- 🖥️ **Interactive Ratatui TUI** — Lightweight, keyboard-driven terminal checklist. Effortlessly select exactly what you want installed.
- 🌐 **Tailscale & MagicDNS** — Built-in WireGuard mesh. Connect via `ssh user@hostname` from anywhere without static IPs, port forwarding, or firewall holes.
- ⏱️ **Granular Adaptive Timers** — Section, subsection, and overall total duration timings with dynamic unit formatting (`<1ms`, `420ms`, `12.4s`, `1m 24s`).
- 📦 **Zero-Clone Embedded Dotfiles** — All configuration templates (`.tmux.conf`, `.bash_aliases`, `starship.toml`) are compiled directly into the binary with `include_str!`.
- 🎬 **Jellyfin Media Server** — Turn your server into a personal Netflix with automated Docker Compose deployment and Intel QuickSync (QSV) hardware transcoding.
- 📥 **qBittorrent Web UI** — Remote torrent management out of the box via Docker Compose on port 6881 and UFW firewall configuration.
- 🛡️ **Hardened Server Security** — Automated UFW firewall configuration (SSH, HTTP, HTTPS) and unneeded daemon cleanup.
- 🚀 **Dual Architecture Releases** — Native static binaries built for both `x86_64` and `aarch64` (AWS Graviton, Ampere, Raspberry Pi).
- 🦀 **Strict Rust Standards** — Built under strict quality gates: `<300` LOC per file, `<40` LOC per function, zero unhandled `unwrap()` calls, and zero Clippy warnings.

---

## 🧩 Provisioning Modules

| # | Module | Identifier | Included Components & Configuration |
|:---:|:---|:---|:---|
| `01` | **Git & SSH Key** | `git_ssh` | Ed25519 SSH keypair generation, GitHub CLI association & connection test |
| `02` | **System Essentials** | `essentials` | `git`, `tmux`, `neovim`, `adb`, `curl`, `build-essential`, official GitHub CLI (`gh`) |
| `03` | **Modern CLI Suite** | `cli_tools` | `eza` (modern ls), `bat` (cat with wings), `zoxide` (smart cd), `fzf`, `ble.sh` |
| `04` | **Dev Runtimes** | `dev_runtimes` | Latest stable Go, Rust toolchain (`rustup`), Python (`uv`), JavaScript (`bun`) |
| `05` | **Server Security** | `security` | UFW Firewall (ports 22, 80, 443) & unneeded daemon cleanup |
| `06` | **Docker Platform** | `docker` | Official Docker CE Engine, `containerd`, and Docker Compose v2 plugin |
| `07` | **Jellyfin Media Server** | `jellyfin` | Dockerized media streaming with Intel QuickSync / VAAPI GPU acceleration (Optional) |
| `08` | **qBittorrent Server** | `torrent` | Dockerized BitTorrent client with Web UI (port 6881) & peer ports (Optional) |
| `09` | **Shell Prompt** | `prompt` | Cross-shell Starship prompt with Nerd Font glyphs & Fastfetch CLI |
| `10` | **Trash Manager** | `trash` | `toss-rs` safe terminal trash TUI with FreeDesktop spec & safe `rm` alias |
| `11` | **Tailscale Mesh VPN** | `tailscale` | WireGuard mesh, MagicDNS (hostname SSH) & Tailscale SSH without static IP |
| `12` | **Aesthetic Dotfiles** | `dotfiles` | Zero-clone deployment of embedded `~/.tmux.conf`, `~/.bash_aliases`, and `starship.toml` |

---

## ⌨️ TUI Keybindings

| Key | Action |
|:---:|:---|
| <kbd>↑</kbd> / <kbd>k</kbd> | Move selection up |
| <kbd>↓</kbd> / <kbd>j</kbd> | Move selection down |
| <kbd>Space</kbd> | Toggle selected module on/off |
| <kbd>a</kbd> | Select all modules |
| <kbd>n</kbd> | Deselect all modules |
| <kbd>Enter</kbd> | Confirm selection and begin provisioning |
| <kbd>q</kbd> / <kbd>Esc</kbd> | Cancel and exit |

---

## 🛠️ CLI Usage & Commands

```bash
# Launch interactive TUI checklist (default)
ryoiki

# Run all modules automatically without prompts (ideal for headless setups)
ryoiki --all

# Audit the host system and report which tools are installed
ryoiki check

# Deploy or update embedded dotfiles only (~/.tmux.conf, ~/.bash_aliases, starship.toml)
ryoiki dotfiles

# Dry-run simulation (prints all planned actions without modifying system state)
ryoiki --dry-run

# Run specific modules by identifier
ryoiki run dev_runtimes docker security

# In-place self-update to latest release from GitHub
ryoiki update

# Start 2-way Telegram bot & embedded webhook gateway (:9119)
ryoiki bot

# Send custom Telegram alert from any script or cron job
ryoiki notify send "Database backup completed successfully" --title "Backup" --level success

# Dispatch system boot metrics notification
ryoiki notify boot

# Dispatch SSH login security alert (called via PAM or shell profile)
ryoiki notify login --user neko --ip 198.51.100.42

# Fire alert via local HTTP webhook (works from Docker containers, Python, curl)
curl -d "text=Service restart completed" http://127.0.0.1:9119/notify

# Install systemd boot service & PAM login notification hooks
ryoiki notify install-hooks

# Send AC power plug/unplug notification (normally called by udev rule automatically)
ryoiki notify power plugged
ryoiki notify power unplugged

# Start battery watch daemon — fires Telegram alert at 50%, 40%, 30%, 25%, 15%, 5%, 1%
ryoiki notify battery-watch
```

---

## 📂 Repository Layout

```text
ryoiki/
├── .agent/
│   ├── CODEBASE.md           # Authoritative codebase architecture map
│   └── rules/rust.md         # Rust code quality rules (enforced in CI)
├── .github/
│   ├── release.yml           # Categorized GitHub release changelog config
│   └── workflows/
│       ├── ci.yml            # Automated CI: fmt, clippy (-D warnings), test & line limits
│       └── release.yml       # Multi-arch binary builder & publisher (x86_64 & aarch64)
├── configs/                  # Embedded configuration templates
│   ├── .tmux.conf            # 256-color, vi-keys, mouse-scrolling tmux config
│   ├── .bash_aliases         # Modern tool aliases (eza, bat, toss) & PATH exports
│   └── starship.toml         # Minimalist Catppuccin-styled prompt with Nerd Font glyphs
├── src/
│   ├── main.rs               # CLI coordinator, argument parser & execution banner
│   ├── modules.rs            # Module registry, dependency resolver & dispatcher
│   ├── runner.rs             # Subprocess runner, elapsed timers & quiet output logging
│   ├── state.rs              # Run state persistence & resume-on-interruption
│   ├── tui.rs                # Ratatui interactive checklist interface ([Space], [a], [n])
│   ├── updater.rs            # In-place binary self-update from GitHub releases
│   ├── configs.rs            # Embedded dotfile deployment routines
│   ├── notify.rs             # Central notification dispatcher & CLI handler
│   ├── notify/               # Notification subsystem
│   │   ├── config.rs         # Telegram config loader with legacy fallback
│   │   ├── client.rs         # Telegram HTTP API client & HTML card generator
│   │   ├── system.rs         # Boot metrics, PAM login hook & custom alerts
│   │   ├── power.rs          # AC plug/unplug events & battery watch daemon
│   │   └── server.rs         # Embedded loopback HTTP webhook server (:9119)
│   └── modules/              # Single-responsibility provisioning modules
│       ├── cli_tools.rs      # eza, bat, zoxide, fzf, ble.sh
│       ├── dev_runtimes.rs   # Go, Rustup, uv, Bun
│       ├── docker.rs         # Docker Engine CE & Docker Compose
│       ├── essentials.rs     # Base utilities & official GitHub CLI
│       ├── git_ssh.rs        # Git identity, Ed25519 SSH key & GitHub verification
│       ├── jellyfin.rs       # Jellyfin media server & Intel QuickSync GPU
│       ├── prompt.rs         # Starship prompt & Fastfetch system stats
│       ├── security.rs       # UFW firewall & daemon cleanup
│       ├── tailscale.rs      # Tailscale WireGuard mesh VPN & MagicDNS
│       ├── torrent.rs        # qBittorrent server & Web UI
│       ├── torrent/          # Torrent API, Telegram bot & notification hooks
│       └── trash.rs          # toss-rs safe trash manager
├── Cargo.toml                # Rust 2021 package manifest with strict lints
├── remote-install.sh         # Instant remote bootstrap script (curl | bash)
└── install.sh                # Local installer script
```

---

## 🛡️ Code Quality Standards

The codebase enforces strict, automated quality rules checked via CI on every pull request:

- **Strict File Limits**: Max 300 lines (soft) / 400 lines (hard gate).
- **Strict Function Limits**: Max 40 lines (soft) / 60 lines (hard gate).
- **Zero Unhandled Errors**: No raw `unwrap()` or `expect()` in production modules.
- **Zero Warnings**: `-D warnings` on both `cargo build` and `cargo clippy --all-targets --all-features`.
- **Modern Module Structure**: Clean 2018+ module tree (`src/modules.rs` with `src/modules/*.rs`).

---

## 📜 License

Distributed under the [MIT License](LICENSE).
