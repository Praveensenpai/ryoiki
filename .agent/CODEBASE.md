# 🌸 領域 (Ryoiki) — Codebase Map

> **AI Agent Notice:** This file is the authoritative codebase map.
> **You MUST update this file whenever you add, remove, rename, or significantly change any module, file, or architectural component.**
> Keep it in sync — a stale map is worse than no map.

---

## What Is Ryoiki?

A single static Rust binary that provisions Ubuntu/Debian servers. It presents an interactive **Ratatui TUI checklist** of 12 provisioning modules. All subprocess noise (`apt`, `dpkg`, compiler output) is hidden behind single-line spinners. Full logs go to `~/.local/state/ryoiki/install.log`.

---

## File Tree

```
ryoiki/
├── .agent/
│   ├── CODEBASE.md          ← this file (keep updated!)
│   └── rules/
│       └── rust.md          ← Rust code quality rules (enforced in CI)
├── configs/                 ← compiled into binary via include_str!
│   ├── .tmux.conf
│   ├── .bash_aliases
│   └── starship.toml
├── src/
│   ├── main.rs              ← CLI entry, arg parsing, orchestration, summary
│   ├── modules.rs           ← Module registry + dispatcher
│   ├── runner.rs            ← Runner struct: subprocess exec, spinners, logging
│   ├── tui.rs               ← Ratatui interactive checklist TUI
│   ├── configs.rs           ← Dotfile deployment (include_str! embeds)
│   └── modules/
│       ├── git_ssh.rs       ← Ed25519 SSH keygen, git identity, GitHub verify
│       ├── essentials.rs    ← apt: git, tmux, neovim, adb, gh CLI
│       ├── cli_tools.rs     ← eza, bat, zoxide, fzf, ble.sh
│       ├── dev_runtimes.rs  ← Go, rustup, uv (Python), Bun (JS)
│       ├── security.rs      ← UFW firewall (ports 22/80/443), daemon cleanup
│       ├── docker.rs        ← Docker Engine CE + Compose plugin
│       ├── jellyfin.rs      ← Dockerized Jellyfin + Intel QuickSync GPU
│       ├── prompt.rs        ← Starship cross-shell prompt + fastfetch
│       ├── trash.rs         ← toss-rs trash manager binary install
│       ├── tailscale.rs     ← WireGuard mesh VPN + MagicDNS SSH
│       ├── torrent.rs       ← qBittorrent Docker setup (main entry)
│       └── torrent/         ← Torrent subsystem submodules
│           ├── api.rs       ← qBittorrent Web API v2 (blocking reqwest)
│           ├── bot.rs       ← 2-way long-polling Telegram bot daemon
│           ├── notify.rs    ← Torrent event Telegram alerts (AutoRun hook)
│           └── telegram.rs  ← TelegramConfig JSON + systemd service install
├── Cargo.toml               ← Rust 2021, strict lints (deny unwrap, dead_code, warnings)
├── install.sh               ← Local install script
└── remote-install.sh        ← One-liner remote bootstrap (curl | bash)
```

---

## Core Components

### `main.rs` — CLI & Orchestration
- **Clap** parser with global flags: `--all`, `--yes`, `--dry-run`, `--verbose`
- **Subcommands:** `dotfiles`, `check`, `run <ids...>`, `bot`, `notify <event> <hash>`
- **Flow:** parse → banner → TUI or auto-select → sudo check → loop modules → print timed summary
- `print_summary` / `print_module_highlights` / `run_system_check` live here

### `runner.rs` — `Runner` struct
| Method | Purpose |
|---|---|
| `Runner::new(dry_run, verbose)` | Init, create log dir + file at `~/.local/state/ryoiki/install.log` |
| `exec_silent(desc, prog, args)` | Run subprocess, show braille spinner, log stdout+stderr |
| `exec_bash(desc, script)` | Shorthand: `bash -c script` |
| `apt_install(desc, pkgs)` | `sudo apt-get install -y --no-install-recommends ...` |
| `apt_update()` | `sudo apt-get update -y` |
| `ensure_sudo()` | One-time `sudo -v` prompt + background keepalive thread (every 60s) |
| `command_exists(cmd)` | PATH + absolute path check |
| `format_duration(d)` | `<1ms` / `420ms` / `12.4s` / `1m 24s` |

### `tui.rs` — Ratatui TUI
- 3-panel layout: **header** / **module checklist** / **footer keybindings**
- State: `Vec<bool>` (selected) + `cursor: usize`
- Returns `Option<Vec<String>>` (selected module IDs, or `None` on quit)
- Keys: `↑↓jk` navigate, `Space` toggle, `a` all/none toggle, `Enter` confirm, `q/Esc` quit

### `modules.rs` — Registry & Dispatch
- `Module { id, title, description, default_enabled }` — static metadata
- `get_available_modules()` → ordered `Vec<Module>` (core → platform → environment)
- `requires_sudo(modules)` → `bool`
- `execute_module(id, runner, non_interactive)` → dispatches to each module's `setup()`

### `configs.rs` — Dotfile Deployment
- `TMUX_CONF`, `BASH_ALIASES`, `STARSHIP_TOML` embedded at compile time via `include_str!`
- `deploy_dotfiles(home)` — writes files, removes broken symlinks, patches `~/.bashrc`
- Contains unit tests for symlink + regular file overwrite behavior

---

## The 12 Provisioning Modules

| ID | Title | Key Behavior |
|---|---|---|
| `git_ssh` | Git & SSH Key Setup | Interactive git identity prompt, Ed25519 keygen, optional GitHub SSH verify |
| `essentials` | System Essentials | apt packages + official GitHub CLI repo |
| `cli_tools` | Modern CLI Suite | eza, bat, zoxide, fzf, ble.sh |
| `dev_runtimes` | Dev Runtimes | Go binary, rustup, uv, Bun |
| `security` | Server Security | UFW allow 22/80/443, disable unused daemons |
| `docker` | Docker Platform | Docker CE + containerd + Compose plugin |
| `jellyfin` | Jellyfin Media Server | `docker run` with Intel `/dev/dri` passthrough |
| `torrent` | qBittorrent Server | Docker run, PBKDF2 creds, Telegram bot, UFW ports |
| `prompt` | Shell Prompt | Starship + fastfetch |
| `trash` | Trash Manager | toss-rs binary install |
| `tailscale` | Tailscale Mesh VPN | curl install, systemd enable, `tailscale up --ssh` |
| `dotfiles` | Aesthetic Dotfiles | Deploy embedded `.tmux.conf`, `.bash_aliases`, `starship.toml` |

Modules requiring sudo: `essentials`, `cli_tools`, `dev_runtimes`, `security`, `docker`, `jellyfin`, `prompt`, `tailscale`

---

## Torrent Subsystem — Detailed

The most complex module. Lives in `src/modules/torrent.rs` + `src/modules/torrent/`.

### `torrent.rs` (setup entry)
1. Ensures Docker is installed (calls `docker::setup` if missing)
2. Creates `~/.config/qbittorrent/` and `~/torrents/` directories
3. Optionally prompts for custom WebUI credentials → hashes with python3 PBKDF2
4. Optionally configures Telegram bot (`telegram::prompt_telegram_config`)
5. Writes `qBittorrent.conf` with sane defaults (save paths, pre-allocation, subnet whitelist)
6. Starts `lscr.io/linuxserver/qbittorrent:latest` Docker container (ports 6881/6882)
7. Configures UFW if available
8. Prints access URLs (Tailscale IP preferred)

### `torrent/api.rs` — qBittorrent Web API v2
- `get_torrents(client, base_url, hash?)` → `Vec<TorrentInfo>`
- `add_magnet(client, base_url, magnet)` → `Result<()>`
- `add_torrent_file(client, base_url, filename, bytes)` → multipart upload
- `pause_all` / `resume_all`

### `torrent/bot.rs` — Telegram Bot Daemon (`ryoiki bot`)
- Long-poll `getUpdates` loop with 25s timeout
- Auth: only responds to messages from `config.chat_id`
- Handles: magnet links → `api::add_magnet`, `.torrent` files → download + `api::add_torrent_file`
- Commands: `/status`, `/disk`, `/pause`, `/resume`, `/help`
- Background thread monitors torrents every 4s, sends completion/start alerts

### `torrent/notify.rs` — AutoRun Hook (`ryoiki notify <event> <hash>`)
- Called by qBittorrent's AutoRun on torrent start/complete
- Retries up to 30× (2s sleep) for "started" event to wait for metadata
- Renders HTML Telegram message with name, size, category, host, Tailscale URL

### `torrent/telegram.rs` — Config & Service
- `TelegramConfig { bot_token, chat_id, qbittorrent_url }` — serde JSON
- Stored at `~/.config/qbittorrent/telegram.json`
- `install_bot_service(home)` — writes `~/.config/systemd/user/ryoiki-bot.service`, enables it
- `configure_autorun(lines, enabled)` — patches `[AutoRun]` section in `qBittorrent.conf`

---

## CLI Reference

```bash
ryoiki                           # Interactive TUI (default)
ryoiki --all                     # All modules, non-interactive
ryoiki --dry-run                 # Simulate, no system changes
ryoiki --verbose / -v            # Stream subprocess output
ryoiki check                     # Audit installed tools
ryoiki dotfiles                  # Deploy dotfiles only
ryoiki run <id> [id...]          # Run specific modules
ryoiki bot                       # Start Telegram bot daemon (blocking)
ryoiki notify <event> <hash>     # Send torrent Telegram alert (called by qBittorrent)
```

---

## Code Quality Gates (CI enforced)

See `.agent/rules/rust.md` for full rules. Summary:

| Gate | Limit |
|---|---|
| Lines per file | 300 soft / 400 hard |
| Lines per function | 40 soft / 60 hard |
| `unwrap()` / `expect()` | Zero in production |
| Compiler warnings | Zero (`-D warnings`) |
| Dead code / unused | `deny` in `Cargo.toml` |
| Module style | `modules.rs` + `modules/*.rs` (no `mod.rs`) |

---

## Dependencies

| Crate | Purpose |
|---|---|
| `clap` | CLI argument parsing (derive) |
| `ratatui` | TUI framework |
| `crossterm` | Terminal raw mode, events |
| `indicatif` | Progress spinners |
| `colored` | Terminal color output |
| `anyhow` | Error handling |
| `serde` / `serde_json` | TelegramConfig JSON serialization |
| `reqwest` (blocking) | HTTP client for Telegram & qBittorrent APIs |
| `libc` | `geteuid()`, `statvfs()` syscalls |
