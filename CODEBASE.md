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

#### `src/modules/media/audio.rs` (Role: Dubstrip Audio Dispatcher, Lines: ~185)
- **Responsibility**: CLI dispatch for `dubstrip` binary; auto-strip on organize with enqueue-on-failure.
- **Sub-modules**: `strip_queue`, `retry_timer`.
- **Public Functions**: `pub fn handle_cli(sub: AudioSubcommand) -> Result<()>`, `pub fn strip_audio_auto(path: &Path)`, `pub fn find_dubstrip_bin() -> Option<PathBuf>`.
- **Subcommand enum**: `AudioSubcommand` — `Inspect`, `Strip`, `Sweep`, `Retry`.

#### `src/modules/media/audio/strip_queue.rs` (Role: Persistent Retry Queue, Lines: ~119)
- **Responsibility**: JSON queue at `~/.local/share/ryoiki/pending_strips.json`; stores `{ path, attempts, enqueued_secs }`; max 24 attempts per entry.
- **Public Functions**: `pub fn enqueue(path: &Path)`, `pub fn process_queue(dubstrip_bin: &Path) -> Result<()>`.

#### `src/modules/media/audio/retry_timer.rs` (Role: Systemd Hourly Timer, Lines: ~88)
- **Responsibility**: Deploys `ryoiki-strip-retry.{service,timer}` under `~/.config/systemd/user/`; fires `ryoiki audio retry` every hour; status exposed in `ryoiki storage`.
- **Public Functions**: `pub fn deploy_retry_timer(home: &str) -> Result<()>`, `pub fn is_timer_active() -> bool`.

#### `src/modules/media/transfer.rs` & `transfer/*.rs` (Role: Library Sync & Archive Transfer, Lines: ~1100 total)
- **Responsibility**: Manages bi-directional migration (push to cold archive, pull from archive) with sync status cross-referencing, delta calculation, and persistent mtime scan caching.
- **Sub-modules**: `scan.rs` (dual-threaded file inventory & size collector), `cache.rs` (hierarchical mtime cache at `~/.cache/ryoiki/media_cache.json`), `sync_status.rs` (cross-references local SSD and remote archive items/seasons at the file level), `execute.rs` (rclone transfer orchestration with missing file delta summaries).
- **Types**:
  - `MediaFile`: `{ name: String, rel_path: String, size_bytes: u64, exists_in_other: bool }`
  - `SyncStatus`: `total_files`, `other_files`, `total_bytes`, `missing_bytes`.
  - `MediaScanCache`: `{ version: u32, local_items: HashMap<String, CachedItem>, remote_items: HashMap<String, CachedItem> }`
- **Public Functions**:
  - `pub fn scan_libraries(home: &Path) -> Result<(Vec<MediaItem>, Vec<MediaItem>)>`
  - `pub fn rescan_libraries(home: &Path) -> Result<(Vec<MediaItem>, Vec<MediaItem>)>`
  - `pub fn cross_reference_libraries(local: &mut [MediaItem], remote: &mut [MediaItem])`

#### `src/modules/media/pruner.rs`, `pruner/scan.rs`, `pruner/execute.rs` & `prune_timer.rs` (Role: Retention Janitor, Lines: ~195 / ~87 / ~198 / ~95)
- **Responsibility**: Automated disk cleanup rules evicting cold/watched media to Google Drive cold storage (`gdrive:ryoiki-archive/media/`) on 1h timer or 80% watermark; failure detection and Telegram alert reporting.
- **Public Functions**: `pub fn run_prune(opts: PruneOptions) -> Result<()>`, `pub fn handle_bot_prune() -> Result<String>`, `pub fn is_timer_active() -> bool`.

#### `src/modules/media/interactive/` (Role: Media TUI Reviewer & Drilldown, Lines: ~1500 total)
- **Responsibility**: Interactive terminal review and transfer selector (`events.rs`, `selection.rs`, `ui/main_view.rs`, `ui/sub_view.rs`, `ui/files_view.rs`). Shows bidirectional sync badges, calculates exact delta download bytes, supports drill-down file inspections, and interactive `[r]` rescan.
- **View Modes**: `MainView`, `SubView { item_idx, cursor }`, `FilesView { item_idx, season_idx, cursor }`.
- **Public Functions**: `pub fn run_interactive_transfer(direction: TransferDirection, target_root: &Path, remote_root: &Path, local_items: Vec<MediaItem>, remote_items: Vec<MediaItem>) -> Result<()>`.

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
- **2026-09-24**: Resolved `0 B` file size display bug for standalone anime movies and flat series lacking `Season XX` subdirectories (e.g. *Demon Slayer Kimetsu no Yaiba Infinity Castle*, *Paprika*, *Non Non Biyori Vacation*). Updated `src/modules/media/transfer/scan.rs` (`scan_local_category` & `scan_remote_mounted`) to fall back to direct file scanning when no season subfolders exist; hardened `src/modules/media/transfer/cache.rs` to prevent caching empty 0-byte items when subdirectories are absent. Bumped version to `v0.1.63`.
- **2026-09-24**: Added aligned column header rows across interactive media TUI views (`src/modules/media/interactive/ui/main_view.rs`, `sub_view.rs`, and `files_view.rs`) clearly demarcating `Type`, `Title`, `Seasons`, `Size`, `Sync Status`, and `Status` columns; condensed badge layouts to preserve compact single-line rendering under 120-column viewports while staying strictly under `<400 lines/file` limits. Bumped version to `v0.1.62`.
- **2026-09-23**: Added hierarchical mtime-based media scan caching (`src/modules/media/transfer/cache.rs`) persisted at `~/.cache/ryoiki/media_cache.json` for both local SSD and Google Drive archive. Bypasses recursive filesystem and FUSE directory traversals by comparing item/season folder modification timestamps; extracted `src/modules/media/interactive/selection.rs` to maintain strict `<400 lines/file` modular limits; added interactive `[r] Refresh` key in TUI to force cache invalidation. Bumped version to `v0.1.61`.
- **2026-09-23**: Added bidirectional library presence cross-referencing and partial folder sync indicators between local library (`~/jellyfin/media`) and remote cloud archive (`~/gdrive/media`). Populated file-level inventory during library scanning; introduced `SyncStatus` with file-count and missing-byte tracking; added interactive drill-down file viewer (`FilesView`) in TUI (`Enter` / `v`); upgraded pull mode storage calculation to only count delta missing bytes. Bumped version to `v0.1.60`.
- **2026-09-23**: Synchronized dubstrip origin confidence handling and Telegram alerts. Added `sync_filename_after_strip` in `src/modules/media/audio.rs` to dynamically update `[Multi]` filename tags to `[<Language>]` (e.g. `[Tamil]`) when dubstrip successfully isolates the native track, while preserving `[Multi]` if dubstrip keeps multi-audio due to uncertain/low origin confidence (<80%). Updated `src/notify/system.rs` (`send_audio_strip_notification`) to format cards as `AUDIO PRESERVED AS MULTI` with confidence ratings when multi-audio is kept, and `AUDIO DUB TRACKS STRIPPED` when dubs are removed. Bumped version to `v0.1.59`.
- **2026-09-23**: Added persistent dubstrip retry queue (`audio/strip_queue.rs`: JSON queue at `~/.local/share/ryoiki/pending_strips.json`, max 24 attempts) and hourly systemd user timer (`audio/retry_timer.rs`: `ryoiki-strip-retry.{service,timer}`). `strip_audio_auto` enqueues on failure instead of silently logging. `AudioSubcommand::Retry` processes queue non-interactively (invoked by timer). Timer deployed in `dubstrip.rs::setup`. `retry_timer::is_timer_active()` wired into `status.rs::print_automation_status()` — visible as `Strip Retry` row under `ryoiki storage`.
- **2026-09-20**: Resolved multi-season anime directory fragmentation (e.g. Non Non Biyori sequels). Implemented franchise prefix directory resolution and destination filename canonicalization in `src/modules/media/organizer/pathing.rs`. Enhanced Gemini AI prompt in `src/modules/media/ai/prompt.rs` to enforce canonical base franchise titles across multi-season batches and sequels. Bumped version to `v0.1.57`.
- **2026-09-18**: Added system and container timezone auto-detection and custom configuration module (`src/modules/timezone.rs`). Supports querying IP geolocation providers (`ip-api.com`, `ipapi.co`, `ipinfo.io`) with automatic fallback, interactive prompt selection, custom IANA override, and dynamic `TZ` environment injection into the qBittorrent container. Added `ryoiki timezone` CLI command (`--auto`, `--status`, `--provider <PROVIDER>`). Bumped version to `v0.1.53`.
- **2026-09-18**: Fixed qBittorrent torrents JSON deserialization error on `metaDL` downloads by updating `TorrentInfo::total_size` from `u64` to signed `i64` and safely formatting negative sizes as `"Unknown"` in status and notification reports. Added pathing safeguards in `organizer.rs` for empty `content_path`. Bumped version to `v0.1.52`.
- **2026-09-18**: Integrated `tayori` standalone notification engine (`v0.1.0`) into `ryoiki` (`v0.1.51`). Refactored `src/notify/client.rs` to delegate HTML escaping and raw Telegram dispatch to `tayori::infra::telegram`. Added automatic `tayori` standalone binary installation to `src/modules/cli_tools.rs`.
- **2026-09-17**: Introduced chunked batch media classification (`classify_media_batch` in `ai/batch.rs`) processing up to 35 files per Gemini request to prevent 15 RPM rate exhaustion; decoupled download completion Telegram alerts from media organization in `torrent/notify.rs`; added audio stream count check in `probe.rs` to bypass single-track dubstrip overhead; extracted `organizer/cli.rs` and modularized `ai/*.rs`.
- **2026-09-16**: Added qBittorrent pre-completion check before organizing media to prevent premature moving of active/incomplete downloads; extracted `organizer/pathing.rs`; added `TorrentInfo::is_completed`; enhanced recursive scan to skip `incomplete/` directories.
- **2026-09-13**: Generated AI-first `CODEBASE.md` following the `codebase-digest` standard; synced streamlined Linux x86_64 release rules and autonomous self-healing protocol.
