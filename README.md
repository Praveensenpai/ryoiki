<div align="center">

# 🌸 領域 (Ryoiki)

**Aesthetic Ubuntu server control center: provision, tune battery health, inspect services, and update from one terminal.**

[![Release](https://img.shields.io/github/v/release/Praveensenpai/ryoiki?style=flat-square&color=cba6f7)](https://github.com/Praveensenpai/ryoiki/releases/latest)
[![CI Status](https://img.shields.io/github/actions/workflow/status/Praveensenpai/ryoiki/ci.yml?branch=main&label=CI&style=flat-square&color=a6e3a1)](https://github.com/Praveensenpai/ryoiki/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-89b4fa?style=flat-square)](LICENSE)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021%20Edition-f38ba8?style=flat-square&logo=rust&logoColor=white)](Cargo.toml)
[![Platform: Ubuntu](https://img.shields.io/badge/Platform-Ubuntu%20%7C%20Debian-fab387?style=flat-square&logo=ubuntu&logoColor=white)](https://ubuntu.com/)
[![Arch: x86_64 | aarch64](https://img.shields.io/badge/Arch-x86__64%20%7C%20aarch64-94e2d5?style=flat-square)](https://github.com/Praveensenpai/ryoiki/releases)

</div>

```text
┌──────────────────────────────────────────────────────────────────────────┐
│  領域 Ryoiki · Control Center                              v0.1.122      │
│  ● mochi   │  AC connected  │  62%  │  up 1h  │  Battery                   │
├──────────────────┬───────────────────────────────────────────────────────┤
│ Menu             │ Battery  Charge ceiling and low-battery policy        │
│  1 Dashboard     │  Max charge ceiling            ● on                   │
│  2 Provision     │  Ceiling percent               ◀ 60% ▶                │
│  3 Battery       │  ─────────────────────────────────────────────────    │
│  4 Notifications │  Auto poweroff                 ● on                   │
│  5 Services      │  Shutdown at                   ◀ 5% ▶                 │
│  6 Media         │  ─────────────────────────────────────────────────    │
│  7 Updates       │  Apply ceiling to hardware     enter ↵                │
│  8 Settings      │  Restart battery watcher       enter ↵                │
│                  │  Edit alert thresholds         enter ↵                │
│  5s auto-refresh │  Alert thresholds  50% · 40% · 30% · 25% · 15% · 5%   │
├──────────────────┴───────────────────────────────────────────────────────┤
│ j/k move · h/l tabs · ←/→ adjust · Enter · Tab pane · ? help · q quit    │
└──────────────────────────────────────────────────────────────────────────┘
```


---

## ⚡ Quickstart

> [!IMPORTANT]
> **One command. No git clone. No Rust toolchain. No build step.**

```bash
curl -fsSL https://raw.githubusercontent.com/Praveensenpai/ryoiki/main/remote-install.sh | bash
```

That's it. Paste and run on any fresh Ubuntu / Debian server. The script:

1. **Detects your CPU arch** — `x86_64` or `aarch64` (AWS Graviton, Raspberry Pi, etc.)
2. **Downloads the pre-compiled binary** — straight from the latest GitHub Release, no compilation
3. **Installs to `~/.local/bin/ryoiki`** — no root required for the binary itself
4. **Installs system hooks** — udev power rule, battery-watch service, boot & login notifications
5. **Launches the control center** — live system dashboard with provisioning, battery health, services, media, updates, and settings

---

## ✨ Features

- 🎛️ **Persistent Control Center** — An eight-tab Ratatui interface for live metrics, provisioning, battery policy, Telegram notifications, systemd services, media, updates, and local settings.
- 🔋 **Battery Health Guardrails** — Set a charge ceiling and optionally power the host off at a chosen battery percentage, once per discharge cycle. The watcher reads settings live and logs every shutdown decision.
- 🖥️ **Live Dashboard** — CPU, memory, disk, battery gauge, AC state, charge backend, watcher health, and managed-service counts in one glance; slow host probes refresh on a background worker so navigation stays instant.
- 🛡️ **Safe Provisioning Routing** — Bare non-interactive `ryoiki` only prints a plain dashboard. Installing modules always requires an explicit `ryoiki install`, `--all`, or `--yes`.
- ⚙️ **Unified Settings Store** — One validated `~/.config/ryoiki/settings.json` for charge ceiling, alert ladder, auto poweroff, and UI refresh, written atomically.
- 🔇 **Zero Visual Clutter** — Hides noisy `apt`, `dpkg`, and compiler scroll walls behind clean, single-line spinners. Full execution logs are saved quietly to `~/.local/state/ryoiki/install.log`.
- 🌐 **Tailscale & MagicDNS** — Built-in WireGuard mesh. Connect via `ssh user@hostname` from anywhere without static IPs, port forwarding, or firewall holes.
- ⏱️ **Granular Adaptive Timers** — Section, subsection, and overall total duration timings with dynamic unit formatting (`<1ms`, `420ms`, `12.4s`, `1m 24s`).
- 📦 **Zero-Clone Embedded Dotfiles** — All configuration templates (`.tmux.conf`, `.bash_aliases`, `starship.toml`) are compiled directly into the binary with `include_str!`.
- 🎬 **Jellyfin Media Server** — Turn your server into a personal Netflix with automated Docker Compose deployment and Intel QuickSync (QSV) hardware transcoding.
- 📥 **qBittorrent Web UI** — Remote torrent management out of the box via Docker Compose on port 6881 and UFW firewall configuration.
- 🛡️ **Hardened Server Security** — Automated UFW firewall configuration (SSH, HTTP, HTTPS) and unneeded daemon cleanup.
- 🚀 **Dual Architecture Releases** — Native static binaries built for both `x86_64` and `aarch64` (AWS Graviton, Ampere, Raspberry Pi).
- 🦀 **Strict Rust Standards** — Built under strict quality gates: `<400` LOC per file, `<60` LOC per function, zero unhandled `unwrap()` calls, and zero Clippy warnings.

---

## 🧩 Provisioning Modules

| # | Module | Identifier | Included Components & Configuration |
|:---:|:---|:---|:---|
| `01` | **Git & SSH Key** | `git_ssh` | Ed25519 SSH keypair generation, GitHub CLI association & connection test |
| `02` | **System Essentials** | `essentials` | `git`, `tmux`, `neovim`, `adb`, `curl`, `build-essential`, official GitHub CLI (`gh`), OpenCode AI agent |
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
| `13` | **AI Media Organizer** | `media` | Automated Jellyfin library classifier & mover (Dual-tier DeepSeek Primary + Gemini 3.1 Flash-Lite Secondary + Regex fallback) |
| `14` | **Headless Audio Null Sink** | `headless_audio` | Routes default ALSA PCM/CTL to a null device via `~/.asoundrc` (backs up custom config) |

---

## ⌨️ Control Center Keybindings

| Key | Action |
|:---:|:---|
| <kbd>q</kbd> / <kbd>Ctrl-C</kbd> | Quit and restore the terminal |
| <kbd>?</kbd> | Toggle the help overlay |
| <kbd>Tab</kbd> | Switch focus between the menu and content panes |
| <kbd>1</kbd>–<kbd>8</kbd> | Jump to Dashboard, Provision, Battery, Notifications, Services, Media, Updates, or Settings |
| <kbd>j</kbd> / <kbd>k</kbd> / <kbd>↑</kbd> / <kbd>↓</kbd> | Move the cursor |
| <kbd>h</kbd> / <kbd>l</kbd> / <kbd>←</kbd> / <kbd>→</kbd> | Previous/next tab in the menu, or adjust a stepper in content |
| <kbd>Enter</kbd> / <kbd>Space</kbd> | Activate the focused row or toggle a module/toggle |
| <kbd>g</kbd> / <kbd>G</kbd> | Jump to the first/last selectable row |
| <kbd>r</kbd> / <kbd>s</kbd> | Refresh the snapshot / save settings |
| <kbd>Ctrl-U</kbd> | Clear the active text input |
| <kbd>y</kbd> / <kbd>n</kbd> | Confirm or cancel a privileged action |

> Modals take over the keyboard: <kbd>Esc</kbd> cancels an input or confirmation, <kbd>Enter</kbd> saves an input, and any key closes the help overlay.

---

## 🛠️ CLI Usage & Commands

```bash
# Open the control center dashboard (interactive terminal)
ryoiki

# Open the provisioning checklist tab
ryoiki install

# Explicit dashboard subcommand (same as bare ryoiki)
ryoiki tui

# Preview provisioning without changing the system
ryoiki --dry-run

# Headless provisioning: all modules / defaults for prompts
ryoiki --all
ryoiki --yes

# Piped or redirected output never provisions; it prints a plain dashboard
ryoiki < /dev/null

# Audit the host system and report which tools are installed
ryoiki check

# Deploy or update embedded dotfiles only (~/.tmux.conf, ~/.bash_aliases, starship.toml)
ryoiki dotfiles

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

# Seedr.cc Cloud Downloads & Live Pipeline Status
ryoiki seedr status                      # Inspect active Seedr cloud caching & disk download progress
ryoiki seedr "magnet:?xt=urn:btih:..."   # Offload torrent to Seedr cloud with auto-cleanup of qBittorrent duplicates

# AI Media Organizer (Jellyfin Library Placement)
ryoiki media /path/to/downloads --dry-run # Preview media classification without moving
ryoiki media /path/to/downloads           # Classify and organize media files into Movies/Shows/Anime

# Media auto-prune (uses ~/.config/ryoiki/settings.json when flags are omitted)
ryoiki prune --threshold 90 --target 75 --dry-run  # Preview the 90% → 75% policy
ryoiki prune --schedule                            # Deploy the hourly systemd user timer
```

### 🎛️ Control Center & Battery Policy

Bare `ryoiki` opens an eight-tab control center:

| Tab | What it controls |
|:---|:---|
| **Dashboard** | Host, kernel, CPU temperature, memory, disk, battery gauge, AC state, charge backend, watcher health |
| **Provision** | Module checklist with installed markers, per-module details, and an explicit run action |
| **Battery** | Charge ceiling, auto poweroff threshold, alert ladder, hardware apply, watcher start/stop |
| **Notifications** | Telegram token, chat id, server name, timezone, test card, system hooks |
| **Services** | Managed systemd units and Docker containers with restart actions |
| **Media** | Jellyfin library state, auto-prune trigger/target, timer apply/disable, storage report, prune preview, media manager |
| **Updates** | Current binary path, latest release check, in-place self-update |
| **Settings** | Config paths, UI refresh rate, notification timezone, save/reload, resume reset |

Battery policy lives in `~/.config/ryoiki/settings.json`:

```json
{
  "battery": {
    "max_charge_enabled": true,
    "max_charge_percent": 60,
    "alert_thresholds": [50, 40, 30, 25, 15, 5, 1]
  },
  "power": {
    "poweroff_on_low": false,
    "poweroff_percent": 5
  },
  "storage": {
    "prune_threshold": 90,
    "prune_target": 75
  },
  "ui": {
    "refresh_secs": 5
  }
}
```

- **Charge ceiling** is written to the battery controller with `ryoiki charge-limit` or the Battery tab. The watcher also reads it for HP ACPI firmware regulation.
- **Auto poweroff** is off by default. When enabled, the watcher fires once per discharge cycle at `poweroff_percent` or below, sends a Telegram card, then runs `systemctl poweroff`. Plugging in resets the latch. `RYOIKI_NO_POWEROFF=1` suppresses the shutdown for testing.
- **Media auto-prune** triggers at `storage.prune_threshold` (60–95%, default 90%) and recovers to `storage.prune_target` (default 75%, at least 5 points lower). Adjust both in the Media tab with ←/→, then press **Apply auto-prune policy**. The Telegram prune command and bare `ryoiki prune` use the same settings.
- **Alert ladder** fires each threshold once per discharge cycle. Edit it in the Battery tab or change `alert_thresholds` directly.
- **Power decisions** are appended to `~/.local/state/ryoiki/power.log`.
- **System services** run as root with `Environment=HOME=<your home>` so the watcher reads the same settings and Telegram config as the TUI. Reinstall hooks from the Notifications tab after upgrading a rootless install.

### 🎬 AI Media Organizer & Dual-Tier AI

`ryoiki` features an autonomous dual-tier AI pipeline to classify, organize, and rename incoming torrents and media into Jellyfin-compliant directory hierarchies (`Movies/`, `Shows/`, `Anime/`):
1. **Primary Provider**: **DeepSeek** via OpenAI-compatible endpoints (`http://mochi:4000/v1/chat/completions`, model `v4.1flash`).
2. **Secondary Fallback**: **Google Gemini** (`gemini-3.1-flash-lite`) if DeepSeek is unreachable or rate-limited.
3. **Tertiary Fallback**: Offline deterministic regex heuristics.

#### Configuration Options

**1. Configuration File (`~/.config/ryoiki/telegram.json`)**:
```json
{
  "bot_token": "...",
  "chat_id": "...",
  "deepseek_url": "http://mochi:4000/v1/chat/completions",
  "deepseek_model": "v4.1flash",
  "deepseek_api_key": "dseeker",
  "enable_deepseek": true,
  "gemini_api_key": "AIzaSy...",
  "gemini_model": "gemini-3.1-flash-lite"
}
```

**2. Environment Variable Overrides**:
```bash
export DEEPSEEK_URL="http://mochi:4000/v1/chat/completions"
export DEEPSEEK_MODEL="v4.1flash"
export DEEPSEEK_API_KEY="dseeker"
export ENABLE_DEEPSEEK="true"

export GEMINI_API_KEY="AIzaSy..."
export GEMINI_MODEL="gemini-3.1-flash-lite"
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
│   ├── .tmux.conf            # 256-color, vi-keys, mouse-scrolling, OSC 52 clipboard passthrough
│   ├── .bash_aliases         # Modern tool aliases (eza, bat, toss) & PATH exports
│   └── starship.toml         # Minimalist Catppuccin-styled prompt with Nerd Font glyphs
├── src/
│   ├── main.rs               # CLI coordinator, argument parser & execution banner
│   ├── modules.rs            # Module registry, dependency resolver & dispatcher
│   ├── runner.rs             # Subprocess runner, elapsed timers & quiet output logging
│   ├── state.rs              # Run state persistence & resume-on-interruption
│   ├── tui.rs                # Control center entry, terminal guard & plain dashboard
│   ├── tui/                  # Ratatui control center
│   │   ├── app.rs            # Tabs, focus, actions, modals, state
│   │   ├── app/forms.rs      # Text input and Telegram editing forms
│   │   ├── actions.rs        # Privileged/long task execution and suspension
│   │   ├── input.rs          # Key dispatch and navigation
│   │   ├── render.rs         # Header, menu, footer, help/confirm/input overlays
│   │   ├── run.rs            # Event loop, terminal restoration, plain fallback
│   │   ├── system.rs         # Host snapshot and managed-service probes
│   │   ├── views.rs          # Tab row/action router
│   │   ├── views/            # Eight tab views
│   │   └── widgets/          # Theme, row renderer, gauges, modal layout
│   ├── settings.rs           # Unified settings schema and persistence entry
│   ├── settings/             # Battery/power/UI model, validation & atomic store
│   ├── updater.rs            # In-place binary self-update from GitHub releases
│   ├── configs.rs            # Embedded dotfile deployment routines
│   ├── notify.rs             # Central notification dispatcher & CLI handler
│   ├── notify/               # Notification subsystem
│   │   ├── config.rs         # Telegram config loader with legacy fallback
│   │   ├── client.rs         # Telegram HTTP API client & HTML card generator
│   │   ├── system.rs         # Boot metrics, PAM login hook & custom alerts
│   │   ├── power.rs          # AC events, battery watcher & auto-poweroff policy
│   │   ├── power/            # Battery/CPU sensors and PID lock
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
