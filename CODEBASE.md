# CODEBASE.md: Ryoiki (領域) Semantic Digest

> **Notice**: AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
CLI / TUI (main.rs, tui.rs) ──> State & Config (state.rs, configs.rs)
   ├──> Modules Engine (modules.rs: docker, tailscale, rclone, git_ssh, dev_runtimes, security)
   ├──> Media Pipeline (modules/media.rs: scan -> ai/heuristic -> probe -> audio -> transfer -> sync -> pruner)
   ├──> Torrent Engine (modules/torrent.rs: api -> bot -> report -> telegram)
   ├──> Jellyfin Hub (modules/jellyfin.rs: api -> backup -> timer)
   ├──> Notification Server/Client (notify.rs: hooks, power, session, system)
   └──> Battery Subsystem (charge_limit.rs: sysfs, tlp, hp_acpi)
```

## 2. Global Constraints & Architecture Patterns
- **Language**: Rust 2021 edition.
- **Architectural Rules**:
  - Hard Limits: <400 lines/file (soft 300), <60 lines/fn (soft 40), max 4 parameters, max 3 nesting depth.
  - Zero Tolerance: 0 warnings (`-D warnings`), no `#[allow(...)]`, zero production `unwrap()`/`expect()`.
  - Modern module structure: `name.rs` alongside `name/` directory (no `mod.rs`).
- **Target Distribution**: Linux `x86_64` standalone binary (`x86_64-unknown-linux-gnu`).

## 3. Module & Interface Skeleton

### Core & Runtime

#### `src/main.rs` (Role: Entrypoint / CLI Router, Lines: ~350)
- **Responsibility**: Parses CLI subcommands and orchestrates interactive TUI or headless execution.
- **Subcommands**: `setup`, `media`, `jellyfin`, `torrent`, `notify`, `battery`, `update`, `status`.
- **Consumers**: OS process launch.

#### `src/tui.rs` (Role: Terminal UI / Curses, Lines: ~280)
- **Responsibility**: Interactive terminal selection menus, category pickers, and live progress display via Crossterm.
- **Public Functions**: `pub fn select_menu(...)`, `pub fn show_banner()`, `pub fn run_interactive()`.

#### `src/state.rs` (Role: State Persistence, Lines: ~110)
- **Responsibility**: Manages persistent installation state in `~/.config/ryoiki/state.json`.
- **Types**: `pub struct AppState { installed_modules: HashSet<String>, last_run: DateTime }`.

#### `src/configs.rs` (Role: User Config Management, Lines: ~150)
- **Responsibility**: Loads, parses, and persists `~/.config/ryoiki/config.toml`.
- **Types**: `pub struct Config { pub telegram: Option<TelegramConfig>, pub media: Option<MediaConfig>, ... }`.

#### `src/runner.rs` (Role: Command Execution Engine, Lines: ~140)
- **Responsibility**: Safe sub-process execution wrapper with stdout capture, error formatting, and root privilege checks.
- **Public Functions**: `pub fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, RyoikiError>`.

#### `src/summary.rs` (Role: Reporting, Lines: ~90)
- **Responsibility**: Renders ASCII summary tables of executed operations and module health.

#### `src/updater.rs` (Role: Self-Update, Lines: ~130)
- **Responsibility**: Checks GitHub releases for newer versions of `ryoiki` and performs self-binary replacement.

### Media Automation Subsystem (`src/modules/media/`)

#### `src/modules/media.rs` (Role: Media Pipeline Orchestrator, Lines: ~220)
- **Responsibility**: Coordinates multi-stage media ingestion: scanning, dual-engine categorization, audio stripping, transfer, and cleanup.
- **Sub-modules**: `scan`, `probe`, `ai`, `heuristic`, `audio`, `transfer`, `sync`, `pruner`, `interactive`.

#### `src/modules/media/ai.rs` (Role: AI Schema Classifier, Lines: ~210)
- **Responsibility**: Gemini API structured prompt caller for media classification (series title, season, episode, specials).
- **Public Functions**: `pub async fn classify_media_ai(filename: &str, api_key: &str) -> Result<MediaMetadata>`.

#### `src/modules/media/heuristic.rs` (Role: Offline Regex Classifier, Lines: ~240)
- **Responsibility**: High-performance regex fallback parser for anime and TV show filenames when offline or unconfigured.

#### `src/modules/media/probe.rs` (Role: FFprobe Inspector, Lines: ~180)
- **Responsibility**: Inspects video files for container codecs, audio streams, and subtitle tracks via `ffprobe`.

#### `src/modules/media/audio.rs` (Role: Dubstrip Audio Transcoder, Lines: ~210)
- **Responsibility**: Strips unwanted foreign audio tracks or retains specified languages via `ffmpeg`.

#### `src/modules/media/transfer.rs` & `transfer/*.rs` (Role: File Migration, Lines: ~340 total)
- **Responsibility**: Safely moves/hardlinks processed media into organized destination library paths with atomic renaming.

#### `src/modules/media/pruner.rs` & `prune_timer.rs` (Role: Retention Janitor, Lines: ~250 total)
- **Responsibility**: Automated disk cleanup rules deleting old watch queues or expired downloads.

#### `src/modules/media/interactive/` (Role: Media TUI Reviewer, Lines: ~400 total)
- **Responsibility**: Interactive review UI (`events.rs`, `ui/*.rs`) allowing manual approval of filenames before moving.

### Torrent Management Subsystem (`src/modules/torrent/`)

#### `src/modules/torrent.rs` (Role: Torrent Orchestrator, Lines: ~180)
- **Responsibility**: qBittorrent client API integration and watcher.
- **Sub-modules**: `api`, `bot`, `notify`, `report`, `telegram`.

#### `src/modules/torrent/api.rs` (Role: qBittorrent WebAPI Client, Lines: ~260)
- **Responsibility**: Authenticates and interfaces with qBittorrent API (torrents list, pause, resume, delete).

#### `src/modules/torrent/telegram.rs` & `bot.rs` (Role: Telegram Bot Integration, Lines: ~320 total)
- **Responsibility**: Dispatches completion alerts and handles remote commands (`/status`, `/pause`) via Telegram Bot API.

### System Notification Daemon (`src/notify/`)

#### `src/notify.rs` (Role: Notification Root, Lines: ~120)
- **Sub-modules**: `server`, `client`, `power`, `session`, `system`, `hooks`, `config`.

#### `src/notify/server.rs` & `client.rs` (Role: Unix Socket IPC, Lines: ~300 total)
- **Responsibility**: Local background daemon listening on Unix domain socket for system events and hooks.

#### `src/notify/power.rs` & `system.rs` (Role: Hardware Monitors, Lines: ~230 total)
- **Responsibility**: Detects AC connect/disconnect, battery thresholds, high temperatures, and high memory usage.

### Hardware & Battery Management (`src/charge_limit/`)

#### `src/charge_limit.rs` (Role: Battery Charging Limiter, Lines: ~180)
- **Responsibility**: Configures hardware charge thresholds (e.g. 80% stop) to protect battery health.
- **Drivers**: `sysfs.rs` (standard Linux kernel ACPI), `tlp.rs` (TLP subsystem), `hp_acpi.rs` (HP vendor module).

### Server & Tool Modules (`src/modules/`)

- `cli_tools.rs`: Installs curated developer CLI tools (ripgrep, fd, bat, eza, fzf, zoxide).
- `dev_runtimes.rs`: Manages Rust, Node, Python (uv), Go environments.
- `docker.rs`: Docker engine installation, daemon config, and non-root group management.
- `rclone/`: Rclone setup, OAuth flow, systemd mount services.
- `tailscale.rs`: Tailscale VPN installation, subnet routing, and exit node configuration.
- `security.rs`: UFW firewall rules, SSH hardening, Fail2ban setup.
- `jellyfin/`: Media server deployment, systemd timers, automated metadata backups.

## 4. Execution Lifecycle Trace
1. CLI entrypoint invokes `main()`.
2. Evaluates flags or launches interactive `tui::run_interactive()`.
3. Loads configuration from `~/.config/ryoiki/config.toml` or creates defaults.
4. Executes requested module runner:
   - Module operations run command sequences via `runner::run_cmd`.
   - Media operations invoke pipeline (Scan -> AI/Heuristic -> Probe -> Audio -> Transfer).
   - Torrent operations connect to qBittorrent API and monitor events.
5. Emits events to `notify` server or dispatches Telegram message.
6. Writes successful step records to `state.rs` (`~/.config/ryoiki/state.json`).
7. Prints aesthetic completion card; exits with code 0.

## 5. Verification Commands
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## 6. Recent Iteration Changes
- **2026-09-13**: Generated AI-first `CODEBASE.md` following the `codebase-digest` standard; synced streamlined Linux x86_64 release rules and autonomous self-healing protocol.
