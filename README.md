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
  [✓]  8. Shell Prompt               Starship cross-shell prompt & Fastfetch CLI
  [✓]  9. Trash Manager              toss-rs (FreeDesktop trash TUI & rm alias)
  [✓] 10. Tailscale Mesh VPN         WireGuard mesh & MagicDNS (hostname SSH)
  [✓] 11. Aesthetic Dotfiles         Deploy embedded dotfiles (tmux, aliases)

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
| `08` | **Shell Prompt** | `prompt` | Cross-shell Starship prompt with Nerd Font glyphs & Fastfetch CLI |
| `09` | **Trash Manager** | `trash` | `toss-rs` safe terminal trash TUI with FreeDesktop spec & safe `rm` alias |
| `10` | **Tailscale Mesh VPN** | `tailscale` | WireGuard mesh, MagicDNS (hostname SSH) & Tailscale SSH without static IP |
| `11` | **Aesthetic Dotfiles** | `dotfiles` | Zero-clone deployment of embedded `~/.tmux.conf`, `~/.bash_aliases`, and `starship.toml` |

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
```

---

## 📂 Repository Layout

```text
ryoiki/
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
│   ├── modules.rs            # Module registry, sudo requirements & dispatcher
│   ├── runner.rs             # Subprocess runner, elapsed timers & quiet output logging
│   ├── tui.rs                # Ratatui interactive checklist interface
│   ├── configs.rs            # Embedded dotfile deployment routines
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
