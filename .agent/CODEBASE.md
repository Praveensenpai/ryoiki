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
│   ├── modules.rs           ← Module registry + dependency resolution + dispatcher
│   ├── runner.rs            ← Runner struct: subprocess exec, spinners, logging
│   ├── state.rs             ← RunState persistence & resume-on-interruption logic
│   ├── tui.rs               ← Ratatui interactive checklist TUI ([Space], [a], [n])
│   ├── updater.rs           ← Self-update binary directly from GitHub releases
│   ├── configs.rs           ← Dotfile deployment (include_str! embeds)
│   ├── notify.rs            ← Central notification dispatcher & NotifySubcommand
│   ├── notify/
│   │   ├── config.rs        ← TelegramConfig with dual-path backward compatibility
│   │   ├── client.rs        ← Telegram HTTP API client & HTML card formatter
│   │   ├── system.rs        ← Boot metrics, PAM login hook & custom alert logic
│   │   └── server.rs        ← Embedded std::net loopback HTTP webhook server (:9119)
│   └── modules/
│       ├── git_ssh.rs       ← Ed25519 SSH keygen, git identity, GitHub verify
│       ├── essentials.rs    ← apt: git, tmux, neovim, adb, gh CLI
│       ├── cli_tools.rs     ← eza, bat, zoxide, fzf, ble.sh
│       ├── dev_runtimes.rs  ← Go, rustup, uv (Python), Bun (JS)
│       ├── security.rs      ← UFW firewall (ports 22/80/443), daemon cleanup
│       ├── docker.rs        ← Docker Engine CE + Compose plugin
│       ├── jellyfin.rs      ← Dockerized Jellyfin + Intel QuickSync GPU (dep: docker)
│       ├── torrent.rs       ← qBittorrent Docker setup (main entry, dep: docker)
│       ├── prompt.rs        ← Starship cross-shell prompt + fastfetch
│       ├── trash.rs         ← toss-rs trash manager binary install
│       ├── tailscale.rs     ← WireGuard mesh VPN + MagicDNS SSH
│       ├── media.rs         ← Media organization domain models & module declarations
│       ├── media/           ← Media subsystem
│       │   ├── ai.rs        ← Gemini API client with 6-stage exponential retry backoff
│       │   ├── config.rs    ← Gemini API key resolution & interactive prompt
│       │   ├── heuristic.rs ← Local regex/heuristic fallback parser
│       │   └── organizer.rs ← Jellyfin destination resolver, moving & CLI handler
│       └── torrent/         ← Torrent subsystem submodules
│           ├── api.rs       ← qBittorrent Web API v2 (blocking reqwest)
│           ├── bot.rs       ← 2-way long-polling Telegram bot + embedded webhook server
│           ├── notify.rs    ← Torrent event Telegram alerts & post-download media trigger
│           └── telegram.rs  ← Service install & AutoRun qBittorrent configuration
├── Cargo.toml               ← Rust 2021, strict lints (deny unwrap, dead_code, warnings)
├── install.sh               ← Local install script
└── remote-install.sh        ← One-liner remote bootstrap (curl | bash)
```

---

## Core Components

### `main.rs` — CLI & Orchestration
- **Clap** parser with global flags: `--all`, `--yes`, `--dry-run`, `--verbose`
- **Subcommands:** `dotfiles`, `check`, `run <ids...>`, `update`, `bot`, `notify [send|boot|login|torrent|serve|install-hooks]`, `organize [path] [--dry-run]`
- **Flow:** parse → banner → TUI or auto-select → resolve dependencies → sudo check → resume prompt → loop modules → state persistence → print timed summary
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

### `state.rs` — Run State & Interruption Recovery
- `RunState { completed: Vec<String>, pending: Vec<String> }`
- Saved at `~/.local/state/ryoiki/resume.json`
- `resolve_resume()` prompts user if an interrupted run is detected; skips already finished modules
- Automatically cleans up `resume.json` upon successful execution completion

### `updater.rs` — In-place Self Update
- `run_self_update()` queries GitHub Releases API for `Praveensenpai/ryoiki`
- Detects architecture (`x86_64` vs `aarch64`), streams binary asset
- Atomically replaces `std::env::current_exe()` with `chmod +x`

### `tui.rs` — Ratatui TUI
- 3-panel layout: **header** / **module checklist** / **footer keybindings**
- State: `Vec<bool>` (selected) + `cursor: usize`
- Returns `Option<Vec<String>>` (selected module IDs, or `None` on quit)
- Keys: `↑↓jk` navigate, `Space` toggle, `a` select all, `n` deselect all, `Enter` confirm, `q/Esc` quit

### `modules.rs` — Registry, Dependency Resolution & Dispatch
- `Module { id, title, description, default_enabled, deps }` — static metadata with prerequisites
- `resolve_dependencies(selected_ids)` — transitively resolves prerequisites (e.g. `docker` before `torrent` and `jellyfin`) preserving canonical execution order
- `get_available_modules()` → ordered `Vec<Module>` (core → platform → environment)
- `requires_sudo(modules)` → `bool`
- `execute_module(id, runner, non_interactive)` → dispatches to each module's `setup()`

### `configs.rs` — Dotfile Deployment
- `TMUX_CONF`, `BASH_ALIASES`, `STARSHIP_TOML` embedded at compile time via `include_str!`
- `deploy_dotfiles(home)` — writes files, removes broken symlinks, patches `~/.bashrc`
- Contains unit tests for symlink + regular file overwrite behavior

---

## The 12 Provisioning Modules

| ID | Title | Key Behavior | Prerequisites |
|---|---|---|---|
| `git_ssh` | Git & SSH Key Setup | Interactive git identity prompt, Ed25519 keygen, optional GitHub SSH verify | None |
| `essentials` | System Essentials | apt packages + official GitHub CLI repo | None |
| `cli_tools` | Modern CLI Suite | eza, bat, zoxide, fzf, ble.sh | None |
| `dev_runtimes` | Dev Runtimes | Go binary, rustup, uv, Bun | None |
| `security` | Server Security | UFW allow 22/80/443, disable unused daemons | None |
| `docker` | Docker Platform | Docker CE + containerd + Compose plugin | None |
| `jellyfin` | Jellyfin Media Server | `docker run` with Intel `/dev/dri` passthrough | `docker` |
| `torrent` | qBittorrent Server | Docker run, PBKDF2 creds, Telegram bot, UFW ports | `docker` |
| `prompt` | Shell Prompt | Starship + fastfetch | None |
| `trash` | Trash Manager | toss-rs binary install | None |
| `tailscale` | Tailscale Mesh VPN | curl install, systemd enable, `tailscale up --ssh` | None |
| `dotfiles` | Aesthetic Dotfiles | Deploy embedded `.tmux.conf`, `.bash_aliases`, `starship.toml` | None |

Modules requiring sudo: `essentials`, `cli_tools`, `dev_runtimes`, `security`, `docker`, `jellyfin`, `torrent`, `prompt`, `tailscale`

---

## Torrent Subsystem — Detailed

Lives in `src/modules/torrent.rs` + `src/modules/torrent/`.

### `torrent.rs` (setup entry)
1. Creates `~/.config/qbittorrent/` and `~/torrents/` directories
2. Optionally prompts for custom WebUI credentials → hashes with python3 PBKDF2
3. Optionally configures Telegram bot (`telegram::prompt_telegram_config`)
4. Writes `qBittorrent.conf` with sane defaults (save paths, pre-allocation, subnet whitelist)
5. Starts `lscr.io/linuxserver/qbittorrent:latest` Docker container (ports 6881/6882)
6. Configures UFW if available
7. Prints access URLs (Tailscale IP preferred)

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

### `torrent/telegram.rs` — AutoRun Config & User Service
- Re-exports `TelegramConfig` from centralized `crate::notify`
- `install_bot_service(home)` — writes `~/.config/systemd/user/ryoiki-bot.service`, enables it
- `configure_autorun(lines, enabled)` — patches `[AutoRun]` section in `qBittorrent.conf`

---

## Notification Subsystem — Detailed

Lives in `src/notify.rs` + `src/notify/`.

### `notify/config.rs` — Configuration
- `TelegramConfig { bot_token, chat_id, qbittorrent_url, server_name, api_port }`
- Primary path: `~/.config/ryoiki/telegram.json`
- Fallback path: `~/.config/qbittorrent/telegram.json` for 100% backward compatibility

### `notify/client.rs` — Telegram API Client
- `send_telegram_alert(client, token, chat_id, text)`
- `format_card(category, badge, fields)`: aesthetic Ryoiki Telegram cards
- `escape_html(input)`: sanitizes HTML entities

### `notify/system.rs` — System Events & Metrics
- `send_boot_notification(config)`: gathers uptime, public IP, Tailscale IP, RAM/Disk, and kernel release
- `send_login_notification(config, user, ip, service, tty)`: tracks SSH/PAM sessions (`PAM_USER`, `PAM_RHOST`)
- `send_custom_notification(config, msg, title, level)`: arbitrary user/service alerts
- `install_hooks(bin_path)`: sets up `ryoiki-boot-notify.service`, PAM hook in `/etc/pam.d/sshd`, and `/etc/profile.d/ryoiki-login-notify.sh`

### `notify/server.rs` — Local HTTP Webhook Gateway
- Ultra-lightweight loopback HTTP server (`std::net::TcpListener`) listening on `127.0.0.1:9119`
- Endpoints: `POST /notify` or `POST /send` (JSON or plain text) and `GET /health`
- Embedded into `ryoiki bot` daemon automatically; also runnable standalone via `ryoiki notify serve`

---

## CLI Reference

```bash
ryoiki                           # Interactive TUI (default)
ryoiki --all                     # All modules, non-interactive
ryoiki --dry-run                 # Simulate, no system changes
ryoiki --verbose / -v            # Stream subprocess output
ryoiki check                     # Audit installed tools
ryoiki dotfiles                  # Deploy dotfiles only
ryoiki run <id> [id...]          # Run specific modules (auto-resolves dependencies)
ryoiki update                    # In-place self-update to latest GitHub release
ryoiki bot                       # Start Telegram bot + embedded webhook server (:9119)
ryoiki notify send <msg>         # Dispatch custom Telegram alert (--title, --level)
ryoiki notify boot               # Dispatch system boot metrics notification
ryoiki notify login              # Dispatch SSH / PAM login security alert
ryoiki notify torrent <ev> <h>   # Send torrent Telegram alert (AutoRun hook)
ryoiki notify serve [--port 9119]# Run standalone local HTTP webhook listener
ryoiki notify install-hooks      # Install boot systemd unit and PAM login hooks
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
| `serde` / `serde_json` | TelegramConfig and RunState JSON serialization |
| `reqwest` (blocking) | HTTP client for GitHub updater, Telegram & qBittorrent APIs |
| `libc` | `geteuid()`, `statvfs()` syscalls |
