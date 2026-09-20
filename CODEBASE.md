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

#### `src/configs.rs` (Role: Dotfiles & User Config Management, Lines: ~170)
- **Responsibility**: Loads, parses, and persists `~/.config/ryoiki/config.toml`; embeds and deploys dotfiles (`.tmux.conf` with TPM persistence, `.bash_aliases`, `starship.toml`) and provisions TPM plugins.
- **Public Functions**: `pub fn deploy_dotfiles(home: &str) -> Result<()>`.
- **Embedded Assets**: `TMUX_CONF`, `BASH_ALIASES`, `STARSHIP_TOML`.

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

#### `src/modules/media/organizer.rs`, `organizer/cli.rs` & `organizer/pathing.rs` (Role: Media Organizer & Torrent Ingest, Lines: ~250 / ~180 / ~290)
- **Responsibility**: Manages media ingestion workflow. Pre-checks qBittorrent for completed torrents before scanning, protects in-progress downloads, batches media files for AI classification, routes completed files to Jellyfin library, and cleans history.
- **Sub-modules**: `cli.rs` (CLI command processing, dry-run simulation, summary tables, qBittorrent history cleanup), `pathing.rs` (file pathing, franchise prefix directory resolution, filename canonicalization, unique destination resolution with Season 00 flat placement for TV/Anime specials, and atomic moves).
- **Public Functions**:
  - `pub fn run_organize_cli(target: &Path, dry_run: bool) -> Result<()>`
  - `pub fn organize_path(target: &Path, client: &Client, api_key: Option<&str>, dry_run: bool) -> Result<Vec<OrganizeResult>>`
  - `pub fn organize_torrent(torrent: &TorrentInfo, client: &Client, api_key: Option<&str>, dry_run: bool) -> Result<Vec<OrganizeResult>>`
  - `pub fn organize_completed_torrent(client: &Client, torrent: &TorrentInfo, api_key: Option<&str>) -> Result<Option<OrganizeResult>>`
  - `pub fn cleanup_matching_torrents(client: &Client, base_url: &str, organized_files: &[OrganizeResult]) -> usize`

#### `src/modules/media/ai.rs` & `src/modules/media/ai/*.rs` (Role: AI Schema Classifier, Lines: ~30 / ~500 total)
- **Responsibility**: Gemini API caller (`gemini-3.5-flash-lite`) supporting single-file and chunked batch classification (up to 35 files per request) to respect rate limits.
- **Sub-modules**: `batch.rs` (chunked batch classification orchestrator), `client.rs` (HTTP request & exponential backoff retry loop), `prompt.rs` (single/batch prompts & probe context builder), `schema.rs` (deserialization schemas & clean name resolvers).
- **Public Functions**:
  - `pub fn classify_media_ai(client: &Client, api_key: &str, raw_name: &str, probe: Option<&MediaProbe>) -> Result<MediaInfo>`
  - `pub fn classify_media_batch(client: &Client, api_key: &str, items: &[(&str, Option<&MediaProbe>)]) -> Result<HashMap<String, MediaInfo>>`

#### `src/modules/media/heuristic.rs` (Role: Offline Regex Classifier, Lines: ~240)
- **Responsibility**: High-performance regex fallback parser for anime and TV show filenames when offline or unconfigured.

#### `src/modules/media/probe.rs` (Role: FFprobe Inspector, Lines: ~250)
- **Responsibility**: Inspects video files for container codecs, audio stream count (bypassing single-audio streams), audio tracks, and subtitle tracks via `ffprobe`.

#### `src/modules/media/audio.rs` (Role: Dubstrip Audio Transcoder, Lines: ~210)
- **Responsibility**: Strips unwanted foreign audio tracks or retains specified languages via `ffmpeg`.

#### `src/modules/media/transfer.rs` & `transfer/*.rs` (Role: File Migration, Lines: ~340 total)
- **Responsibility**: Safely moves/hardlinks processed media into organized destination library paths with atomic renaming.

#### `src/modules/media/pruner.rs`, `pruner/scan.rs`, `pruner/execute.rs` & `prune_timer.rs` (Role: Retention Janitor, Lines: ~195 / ~87 / ~198 / ~95)
- **Responsibility**: Automated disk cleanup rules evicting cold/watched media to Google Drive cold storage (`gdrive:ryoiki-archive/media/`) on 1h timer or 80% watermark; failure detection and Telegram alert reporting.
- **Public Functions**: `pub fn run_prune(opts: PruneOptions) -> Result<()>`, `pub fn handle_bot_prune() -> Result<String>`, `pub fn is_timer_active() -> bool`.

#### `src/modules/media/interactive/` (Role: Media TUI Reviewer, Lines: ~400 total)
- **Responsibility**: Interactive review UI (`events.rs`, `ui/*.rs`) allowing manual approval of filenames before moving.

### Torrent Management Subsystem (`src/modules/torrent/`)

#### `src/modules/torrent.rs` (Role: Torrent Orchestrator, Lines: ~380)
- **Responsibility**: qBittorrent container lifecycle manager (384MB RAM cap, 64MB disk cache, 256MB working set limit), client API integration, and watcher.
- **Sub-modules**: `api`, `bot`, `notify`, `report`, `telegram`.

#### `src/modules/torrent/api.rs` (Role: qBittorrent WebAPI Client, Lines: ~275)
- **Responsibility**: Authenticates and interfaces with qBittorrent API (torrents list, pause/stop, resume/start, delete) with v5.x endpoint support and v4.x fallback.
- **Types**: `TorrentInfo` (`is_completed(&self) -> bool` checks `progress >= 1.0` or seeding/uploading states; `total_size: i64` handles `-1` on `metaDL`).

#### `src/modules/torrent/telegram.rs` & `bot.rs` (Role: Telegram Bot Integration, Lines: ~500 total)
- **Responsibility**: Dispatches completion alerts, auto-organizes completed downloads with error logging, and handles remote commands (`/status`, `/pause`, `/resume`, `/storage`, `/prune`, `/organize`, `/sync`) with bot mention stripping (`@...`), HTML escaping, and interactive error feedback.

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
- `timezone.rs`: Auto-detects local timezone via IP geolocation (ip-api, ipapi, ipinfo) or configures custom IANA timezone via timedatectl; dynamically injects host timezone into qBittorrent container.
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
- **2026-09-20**: Resolved multi-season anime directory fragmentation (e.g. Non Non Biyori sequels). Implemented franchise prefix directory resolution and destination filename canonicalization in `src/modules/media/organizer/pathing.rs`. Enhanced Gemini AI prompt in `src/modules/media/ai/prompt.rs` to enforce canonical base franchise titles across multi-season batches and sequels. Bumped version to `v0.1.57`.
- **2026-09-18**: Added system and container timezone auto-detection and custom configuration module (`src/modules/timezone.rs`). Supports querying IP geolocation providers (`ip-api.com`, `ipapi.co`, `ipinfo.io`) with automatic fallback, interactive prompt selection, custom IANA override, and dynamic `TZ` environment injection into the qBittorrent container. Added `ryoiki timezone` CLI command (`--auto`, `--status`, `--provider <PROVIDER>`). Bumped version to `v0.1.53`.
- **2026-09-18**: Fixed qBittorrent torrents JSON deserialization error on `metaDL` downloads by updating `TorrentInfo::total_size` from `u64` to signed `i64` and safely formatting negative sizes as `"Unknown"` in status and notification reports. Added pathing safeguards in `organizer.rs` for empty `content_path`. Bumped version to `v0.1.52`.
- **2026-09-18**: Integrated `tayori` standalone notification engine (`v0.1.0`) into `ryoiki` (`v0.1.51`). Refactored `src/notify/client.rs` to delegate HTML escaping and raw Telegram dispatch to `tayori::infra::telegram`. Added automatic `tayori` standalone binary installation to `src/modules/cli_tools.rs`.
- **2026-09-17**: Introduced chunked batch media classification (`classify_media_batch` in `ai/batch.rs`) processing up to 35 files per Gemini request to prevent 15 RPM rate exhaustion; decoupled download completion Telegram alerts from media organization in `torrent/notify.rs`; added audio stream count check in `probe.rs` to bypass single-track dubstrip overhead; extracted `organizer/cli.rs` and modularized `ai/*.rs`.
- **2026-09-16**: Added qBittorrent pre-completion check before organizing media to prevent premature moving of active/incomplete downloads; extracted `organizer/pathing.rs`; added `TorrentInfo::is_completed`; enhanced recursive scan to skip `incomplete/` directories.
- **2026-09-13**: Generated AI-first `CODEBASE.md` following the `codebase-digest` standard; synced streamlined Linux x86_64 release rules and autonomous self-healing protocol.
