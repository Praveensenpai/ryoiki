# 🌸 領域 Ryoiki Codebase Reference & Architectural Specification

> [!IMPORTANT]
> ### 🚨 Mandatory AI Synchronization Protocol
> **This file is the authoritative single source of truth for the entire `ryoiki` codebase.**
> **RULE FOR FUTURE AI ASSISTANTS**:
> Whenever **ANY** file in this repository is created, modified, refactored, renamed, or deleted:
> 1. You **MUST** immediately update `codebase.md` in the same turn/task to reflect all changes (updated signatures, changed behaviors, modified file structures, or new features).
> 2. Never leave `codebase.md` stale or out-of-sync with the physical codebase files.
> 3. Maintain this file's high information density and token-efficient formatting.

---

## 1. High-Level Architecture & Design Philosophy

**Ryoiki** (領域 - *Domain/Territory*) is an aesthetic, idempotent, zero-drift Ubuntu server & workstation provisioning orchestrator written in idiomatic Rust.

```
                     ┌─────────────────────────────────────────────────┐
                     │                 CLI Entrypoint                  │
                     │                 (src/main.rs)                   │
                     └───────────────┬─────────────────┬───────────────┘
                                     │                 │
                      Subcommands    ▼                 ▼  Interactive TUI
                  ┌──────────────────────┐         ┌───────────────────────┐
                  │ • notify  • media    │         │      src/tui.rs       │
                  │ • rclone  • prune    │         │  (Ratatui Module Pick)│
                  │ • backup  • bot      │         └───────────┬───────────┘
                  │ • charge  • dotfiles │                     │
                  └──────────┬───────────┘                     │ Selected IDs
                             │                                 ▼
                             │                     ┌───────────────────────┐
                             │                     │   Dependency Graph    │
                             │                     │ (modules::resolve_deps)
                             │                     └───────────┬───────────┘
                             ▼                                 │
                     ┌─────────────────────────────────────────▼───────────┐
                     │                    src/runner.rs                    │
                     │      Execution Engine, Sudo Elevation, Spinners     │
                     └──────┬──────────────────────┬────────────────┬──────┘
                            │                      │                │
            System Modules  ▼         Notify/Hooks ▼   Charge Limit ▼
            ┌──────────────────┐   ┌──────────────────┐ ┌──────────────────┐
            │ essentials       │   │ Telegram Alerts  │ │ Sysfs / HP-ACPI  │
            │ cli_tools        │   │ Login Debounce   │ │ TLP / Guide      │
            │ dev_runtimes     │   │ Battery Watch    │ └──────────────────┘
            │ docker/jellyfin  │   │ AC Power Hooks   │
            │ torrent/rclone   │   │ Webhook Server   │
            │ media/dubstrip   │   └──────────────────┘
            │ tailscale/trash  │
            └──────────────────┘
```

### Core Architectural Standards
- **Pure Rust Zero-Tolerance Rules**: 0 compiler warnings, 0 clippy warnings (`-D warnings`), zero `unwrap()`/`expect()` in production flows (all bubbled via `anyhow::Result`).
- **File & Function Limits**: Hard ceiling of <400 lines/file (soft target <300) and <60 lines/function (soft target <40).
- **State & Idempotency**: State tracked via `~/.ryoiki_state.json` allowing interrupted runs to seamlessly resume without re-running completed steps.

---

## 2. Directory Tree & File Inventory

```
ryoiki/
├── Cargo.toml                              # Package manifest (v0.1.42, edition 2021, dependencies & release profiles)
├── Cargo.lock                              # Pinned dependency tree
├── install.sh                              # Source bootstrapper (installs rustup if missing, builds, runs)
├── remote-install.sh                       # Binary bootstrapper (downloads prebuilt x86_64/aarch64 tarball from GitHub)
├── codebase.md                             # Authoritative codebase reference (this document)
├── configs/
│   └── starship.toml                       # Preconfigured Starship cross-shell prompt configuration
└── src/
    ├── main.rs                             # CLI entrypoint, argument parsing (clap), command dispatch, banner
    ├── runner.rs                           # Subprocess execution, dry-run simulation, command_exists, sudo prompt
    ├── state.rs                            # Persistent state engine (~/.ryoiki_state.json) for step resumption
    ├── summary.rs                          # Execution reporting card renderer with ANSI box styling & timing metrics
    ├── tui.rs                              # Ratatui/crossterm interactive terminal UI module selection menu
    ├── updater.rs                          # In-place self-updater pulling latest tarball from GitHub releases
    ├── configs.rs                          # Dotfiles deployment (~/.tmux.conf, ~/.bash_aliases, starship.toml)
    ├── charge_limit.rs                     # Battery charge threshold root coordinator & hardware detection
    ├── charge_limit/
    │   ├── guide.rs                        # Fallback guide & vendor manual link renderer for unsupported laptops
    │   ├── hp_acpi.rs                      # HP ACPI charge limit backend detection and configuration
    │   ├── sysfs.rs                        # Standard Linux sysfs battery charge_control_end_threshold backend
    │   └── tlp.rs                          # TLP battery configuration updater (/etc/tlp.conf)
    ├── notify.rs                           # Telegram notification CLI subcommand definitions & dispatcher
    ├── notify/
    │   ├── client.rs                       # Telegram Bot API HTTP client, HTML card formatter, entity escaping
    │   ├── config.rs                       # TelegramConfig schema, loader (~/.config/ryoiki/telegram.json)
    │   ├── hooks.rs                        # Systemd unit generator (boot, battery) and bashrc login hook installer
    │   ├── power.rs                        # Battery level monitor daemon & threshold triggers (20%, 10%, 5%)
    │   ├── server.rs                       # Micro-HTTP webhook listener (default port 9119) for external triggers
    │   ├── session.rs                      # IP-based login notification debouncer (eliminates terminal flood)
    │   └── system.rs                       # Hardware metrics gatherer (CPU, RAM, Disk, Public IP, Uptime)
    ├── modules.rs                          # Module registry, metadata, dependency resolver, sudo requirements
    └── modules/
        ├── cli_tools.rs                    # Installs modern CLI tools: eza, bat, zoxide, fzf, ble.sh
        ├── dev_runtimes.rs                 # Installs dev toolchains: Go, Rust (rustup), Python (uv), Bun
        ├── docker.rs                       # Installs official Docker CE engine, compose plugin, user group setup
        ├── dubstrip.rs                     # Deploys theatrical dubstrip audio stripper binary (Praveensenpai/dubstrip)
        ├── essentials.rs                   # Core apt packages: git, tmux, neovim, adb, gh, build-essential
        ├── git_ssh.rs                      # Ed25519 SSH key generation, Git user config, GitHub CLI auth
        ├── prompt.rs                       # Starship prompt installer & Fastfetch system stats utility
        ├── security.rs                     # UFW firewall configuration, open ports (22, 80, 443), Fail2ban
        ├── tailscale.rs                    # Tailscale mesh VPN installer, MagicDNS, and Tailscale SSH enablement
        ├── trash.rs                        # toss-rs trash manager installation & safe rm shell aliasing
        ├── rclone.rs                       # Rclone Google Drive CLI subcommand root & dispatcher
        ├── rclone/
        │   ├── install.rs                  # Rclone binary installation from official upstream script
        │   ├── oauth.rs                    # Headless interactive OAuth flow for Google Drive client configuration
        │   └── service.rs                  # Generates & manages systemd rclone-mount.service (VFS cache mode)
        ├── jellyfin.rs                     # Jellyfin container deployment, Intel QuickSync/VAAPI hardware transcode
        ├── jellyfin/
        │   ├── api.rs                      # Jellyfin REST API client (/Library/Refresh, user watched status)
        │   ├── backup.rs                   # Database/config tar.gz backup orchestrator to Google Drive
        │   └── timer.rs                    # Systemd timer generator for automated nightly Jellyfin backups
        ├── torrent.rs                      # qBittorrent Docker deployment (port 6881/8080) & autorun hooks
        ├── torrent/
        │   ├── api.rs                      # qBittorrent Web API client (list, add magnet/file, pause, resume, delete)
        │   ├── bot.rs                      # 2-way interactive Telegram Bot daemon (polls commands, magnet downloader)
        │   ├── notify.rs                   # Torrent completed/started Telegram card notification formatter
        │   ├── report.rs                   # Disk and torrent status report builder for Telegram responses
        │   └── telegram.rs                 # Telegram Bot polling types, command handlers (/status, /pause, etc.)
        └── media.rs                        # Media subsystem entry, types (MediaInfo, MediaType), category mappings
        └── media/
            ├── ai.rs                       # Gemini AI classifier, probe context injection, remake disambiguation
            ├── audio.rs                    # Audio inspection & dubstrip stripping runner/dispatcher
            ├── config.rs                   # Media library path resolvers (~/jellyfin/media, categories)
            ├── disk.rs                     # statvfs disk usage calculator, bytes-to-free computer, byte formatter
            ├── heuristic.rs                # Local regex/stem parser (resolutions, season/episodes, year, language)
            ├── interactive.rs              # Interactive media transfer manager entry & cycle filters
            ├── interactive/
            │   ├── events.rs               # Terminal keyboard event handler (navigation, selection, search)
            │   └── ui.rs                   # Ratatui TUI view router
            │   └── ui/
            │       ├── footer.rs           # Keybinding legend & status bar footer
            │       ├── main_view.rs        # Split-pane source directory browser & destination picker
            │       └── sub_view.rs         # Modal dialogs (confirmations, progress popups)
            ├── organizer.rs                # Core ingestion pipeline: scan, probe, classify, rename, move, notify
            ├── probe.rs                    # Deep ffprobe media container inspector (duration, audio languages, res)
            ├── prune_timer.rs              # Generates systemd timer (ryoiki-prune.timer, 6h cadence)
            ├── pruner.rs                   # Smart SSD pruner: sorts by watched status & size, archives to GDrive
            ├── status.rs                   # Console storage dashboard (SSD usage vs Google Drive cloud mounts)
            ├── sync.rs                     # Daily post-boot Google Drive media sync engine & systemd timer
            └── transfer.rs                 # Category transfer coordinator (Movies, TV Shows, Anime, Custom)
            └── transfer/
                ├── execute.rs              # File movement, collision prevention, directory creation
                ├── notify.rs               # Transfer completion Telegram card notifications
                └── scan.rs                 # Recursive directory scanner for video files
```

---

## 3. Core Engine & Runtime Modules

### `src/main.rs`
- **Purpose**: CLI entrypoint, argument parsing via `clap`, subcommand routing, terminal check, and execution flow.
- **Key Types**:
  - `Cli`: Root parser with `--all`, `--yes`, `--dry-run`, `--verbose`, and optional `Commands` enum.
  - `Commands`: Subcommands (`Dotfiles`, `Check`, `Run`, `Update`, `Bot`, `ChargeLimit`, `Notify`, `Organize`, `Rclone`, `Backup`, `Prune`, `Storage`, `Media`, `Audio`).
- **Key Functions**:
  - `main() -> Result<()>`: Initializer; suppresses banner for notify calls, dispatches subcommands or launches TUI.
  - `resolve_selected_modules(cli: &Cli) -> Result<Option<Vec<String>>>`: Handles non-interactive vs TUI selection.
  - `run_modules(...) -> Result<(Duration, Vec<(String, Duration)>)>`: Sequentially executes modules, records timing, updates `RunState`.

### `src/runner.rs`
- **Purpose**: Low-level subprocess management, terminal animation, dry-run interception, and privilege verification.
- **Key Types**:
  - `Runner`: Struct holding `dry_run: bool` and `verbose: bool`.
- **Key Functions**:
  - `Runner::run(&self, msg: &str, cmd: &str) -> Result<()>`: Executes shell command under an animated spinner (`indicatif`). In dry-run, prints command without executing. In verbose, streams stdout/stderr directly.
  - `Runner::run_silent(&self, cmd: &str) -> Result<Output>`: Executes command without terminal spinner, returning output.
  - `Runner::command_exists(cmd: &str) -> bool`: Checks if a binary is in `$PATH` via `which`.
  - `Runner::ensure_sudo(&self) -> Result<()>`: Checks `sudo -v` and prompts if elevation is expired.
  - `format_duration(dur: Duration) -> String`: Formats duration into `Xm Ys` or `Xs`.

### `src/state.rs`
- **Purpose**: Execution persistence stored in `~/.ryoiki_state.json`.
- **Key Types**:
  - `RunState`: Struct storing `completed: HashSet<String>`.
- **Key Functions**:
  - `RunState::load() -> Result<Option<Self>>`: Reads state file from disk.
  - `RunState::mark_done(&mut self, module_id: &str) -> Result<()>`: Appends completed module ID and flushes to disk.
  - `RunState::clear()`: Deletes state file upon complete setup completion.
  - `resolve_resume(selected: Vec<String>, non_interactive: bool) -> Result<(Vec<String>, RunState)>`: Prompts user or automatically resumes from last incomplete module.

### `src/tui.rs`
- **Purpose**: Terminal UI using `ratatui` and `crossterm` for interactive module selection.
- **Key Features**:
  - Checkbox selection (`[X]`), group toggling (`all`, `none`), module details description panel, responsive navigation (`Up`/`Down`, `Space`, `Enter` to confirm, `Esc`/`q` to quit).

### `src/updater.rs`
- **Purpose**: Self-updating mechanism pulling the latest release from GitHub.
- **Key Functions**:
  - `run_self_update(current_version: &str) -> Result<()>`: Queries GitHub Releases API, checks architecture (`x86_64` vs `aarch64`), streams release tarball into temporary directory, verifies binary, and atomizes replacement of current executable.

### `src/summary.rs`
- **Purpose**: Generates high-aesthetic Unicode/ANSI execution summary card with per-module timing table and final status.

### `src/configs.rs`
- **Purpose**: Deploys embedded configuration dotfiles.
- **Embedded Files**:
  - `configs/starship.toml` -> `~/.config/starship.toml`
  - Embedded `.tmux.conf` (prefix key, vi mode, 256color)
  - Embedded `.bash_aliases` (modern CLI aliases for `ls`, `cat`, `cd`, `grep`, `toss`)
  - Appends source directive to `~/.bashrc` idempotently.

---

## 4. Provisioning Modules Subsystem (`src/modules/`)

### `src/modules.rs`
- **Module Struct**: `pub struct Module { id, title, description, default_enabled, deps }`
- **Dependency Resolution**:
  - `resolve_dependencies(selected_ids: &[String]) -> Vec<String>`: Breadth-first dependency resolution ensuring prerequisites precede dependents (e.g. `docker` before `jellyfin` or `torrent`).
- **Privilege Checking**:
  - `requires_sudo(modules: &[String]) -> bool`: Inspects module set to demand sudo elevation upfront before execution starts.

### Available Modules Roster

| Module ID | Title | Description & Actions | Sudo | Dependencies |
|---|---|---|---|---|
| `git_ssh` | Git & SSH Setup | Configures git user/email, generates Ed25519 SSH keys, links with GitHub CLI | No | None |
| `essentials` | System Essentials | Installs `curl`, `git`, `tmux`, `neovim`, `build-essential`, `adb`, `gh` | Yes | None |
| `cli_tools` | Modern CLI Suite | Installs `eza`, `bat`, `zoxide`, `fzf`, and `ble.sh` | Yes | None |
| `dev_runtimes` | Dev Runtimes | Installs Go, Rust (`rustup`), Python (`uv`), and JavaScript (`Bun`) | Yes | None |
| `security` | Server Security | Enables UFW, allows ports 22, 80, 443, enables Fail2ban | Yes | None |
| `docker` | Docker Platform | Installs official Docker CE engine, `containerd`, and `docker-compose-plugin` | Yes | None |
| `jellyfin` | Jellyfin Server | Deploys Dockerized Jellyfin with Intel QuickSync / VAAPI GPU pass-through | Yes | `docker` |
| `torrent` | qBittorrent | Dockerized qBittorrent-nox with Web UI (port 6881), autorun notification hooks | Yes | `docker` |
| `rclone` | Rclone Google Drive | Sets up rclone Google Drive cloud mount and systemd background service | Yes | None |
| `prompt` | Shell Prompt | Deploys Starship cross-shell prompt and Fastfetch system stats tool | Yes | None |
| `trash` | Trash Manager | Installs `toss-rs` (Rust FreeDesktop trash utility) and aliases `rm` | Yes | None |
| `dubstrip` | DubStrip Preserver | Autonomous zero-loss theatrical audio dub stripper (`dubstrip`) | No | None |
| `tailscale` | Tailscale Mesh VPN | Installs Tailscale, connects node, enables MagicDNS & Tailscale SSH | Yes | None |
| `dotfiles` | Aesthetic Dotfiles | Writes `~/.tmux.conf`, `~/.bash_aliases`, `~/.config/starship.toml` | No | None |
| `charge_limit` | Battery Charge Limit | Restricts battery charge threshold (60/80%) for always-plugged servers | Yes | None |
| `media` | AI Media Organizer | Automatic torrent ingestion to Jellyfin via Gemini AI & heuristic engines | No | `jellyfin`, `torrent` |

---

## 5. Media Pipeline & Jellyfin Subsystem (`src/modules/media/`)

The media pipeline handles automatic classification, clean renaming, container stream inspection, audio stripping, disk space management, and Jellyfin library synchronization.

```
Incoming Torrent File (e.g. ~/torrents/completed/Film.2023.1080p.mkv)
                          │
                          ▼
            ┌───────────────────────────┐
            │   src/modules/media/      │
            │        probe.rs           │
            └─────────────┬─────────────┘
                          │ Extracts: duration (~142m), resolution (1080p),
                          │ audio language tracks (Malayalam, English)
                          ▼
            ┌───────────────────────────┐
            │ Has Gemini API Key in     │
            │ telegram.json?            │
            └──────┬─────────────┬──────┘
               Yes │             │ No / Network Fallback
                   ▼             ▼
       ┌──────────────────┐  ┌──────────────────┐
       │   src/media/     │  │   src/media/     │
       │      ai.rs       │  │   heuristic.rs   │
       └──────────┬───────┘  └──────────┬───────┘
                  │                     │
                  └──────────┬──────────┘
                             │ Returns MediaInfo (Title, Year, Resolution, Language)
                             ▼
            ┌───────────────────────────┐
            │   src/modules/media/      │
            │       organizer.rs        │
            └─────────────┬─────────────┘
                          │ • Ensures mandatory (Year) in Movie titles
                          │ • Injects [Language] bracket
                          │ • Resolves collision & unique destination path
                          │ • Moves file: ~/jellyfin/media/Movies/Title (Year)/Title (Year) [Lang] [Res].ext
                          ▼
            ┌───────────────────────────┐
            │  modules/jellyfin/api.rs  │
            │ Trigger Library Refresh   │
            └───────────────────────────┘
```

### Module Breakdown
- **`src/modules/media/probe.rs`**:
  - `probe_media_file(path: &Path) -> Option<MediaProbe>`: Invokes `ffprobe` in JSON mode extracting container duration, video stream height (converted to `2160p`, `1080p`, `720p`, `480p`), and audio streams language tags.
  - `map_language_code(code: &str) -> String`: Maps 3-letter ISO language codes (`mal`, `tam`, `tel`, `hin`, `kan`, `eng`, `jpn`, `kor`, `fra`, `deu`, etc.) to capitalized full language names.
  - `resolve_primary_language(languages: &[String]) -> Option<String>`: Evaluates audio tracks; if single track, returns that language; if multiple tracks with English, prioritizes the native language; if multiple non-English tracks, returns `"Multi"`.
- **`src/modules/media/ai.rs`**:
  - `classify_media_ai(...) -> Result<MediaInfo>`: Uses Google Gemini API (`gemini-2.5-flash` with fallback models) to parse messy filenames into clean schema.
  - Injects `probe` metadata (duration, streams, resolution) into the prompt to resolve cinema remakes (e.g. *Drishyam* 2013 Malayalam vs *Drishyam* 2015 Hindi).
  - `ensure_year_in_clean_name(name, year)`: Guarantees `(YYYY)` format in filenames.
  - `ensure_language_in_clean_name(name, language)`: Guarantees `[Language]` format in filenames.
- **`src/modules/media/heuristic.rs`**:
  - `classify_media_heuristic(raw_name: &str) -> MediaInfo`: Offline regex and string manipulation fallback extracting title, year (`(19|20)\d{2}`), season/episode (`S\d{1,2}E\d{1,2}`), resolution (`2160p`, `1080p`), and regional language tokens.
- **`src/modules/media/organizer.rs`**:
  - `organize_file(...) -> Result<OrganizeResult>`: Drives the ingestion workflow for single files or torrent folders.
  - `run_organize_cli(target: &Path, dry_run: bool) -> Result<()>`: CLI entrypoint scanning directory and notifying Jellyfin upon success.
- **`src/modules/media/disk.rs` & `pruner.rs`**:
  - `get_disk_usage(path: &Path) -> Result<DiskUsage>`: Uses `libc::statvfs` for exact byte counts and disk usage percentages.
  - `run_prune(opts: PruneOptions) -> Result<()>`: Checks if local SSD exceeds threshold percentage (e.g. 80%). Queries Jellyfin API for watch history. Sorts eviction candidates (watched items first, then largest file sizes) and moves them to Google Drive cold storage (`gdrive:ryoiki-archive/media/`) via rclone until disk drops to target percentage (e.g. 70%).
- **`src/modules/media/prune_timer.rs`**:
  - `deploy_prune_timer(home, threshold, target) -> Result<()>`: Installs systemd user timer `ryoiki-prune.timer` running every 6 hours.
- **`src/modules/media/sync.rs`**:
  - `deploy_sync_timer(home) -> Result<()>`: Installs systemd user timer `ryoiki-media-sync.timer` running 30 minutes post-boot and daily to sync local media to Google Drive.
- **`src/modules/media/transfer.rs` & `interactive.rs`**:
  - Interactive Ratatui interface to select, categorize (Movies, TV Shows, Anime, Custom), and move/sync downloads manually.

---

## 6. Notifications & Security Daemon (`src/notify/`)

The notification subsystem provides unified system telemetry, login alerts, power management, and Telegram messaging.

### Configuration Schema (`~/.config/ryoiki/telegram.json`)
```json
{
  "bot_token": "123456789:ABCDEF...",
  "chat_id": "-1001234567890",
  "qbittorrent_url": "http://localhost:6881",
  "server_name": "Ryoiki-Node-01",
  "api_port": 9119,
  "gemini_api_key": "AIzaSy...",
  "jellyfin_url": "http://localhost:8096",
  "jellyfin_api_key": "abc123def...",
  "session_cooldown_mins": 60
}
```

### Components
- **`src/notify/client.rs`**:
  - `send_telegram_card(config, title, lines, level) -> Result<()>`: Constructs styled HTML message cards with header badges (`🌸 INFO`, `✔ SUCCESS`, `⚠ WARN`, `✖ ALERT`) and transmits to Telegram Bot API.
  - `escape_html(raw: &str) -> String`: Sanitizes `<`, `>`, `&` characters.
- **`src/notify/session.rs`**:
  - `is_session_suppressed(ip: &str, cooldown_mins: u64) -> bool`: Prevents spamming Telegram on every new terminal, SSH connection, or IDE remote window. Stores timestamp files in `/tmp/ryoiki_session_{uid}_{sanitized_ip}`. If an IP has connected within `session_cooldown_mins` (default: 60m), the notification is silently suppressed. Distinct or unknown IPs immediately fire alerts.
- **`src/notify/power.rs`**:
  - `run_battery_watch() -> Result<()>`: Long-running battery monitor daemon. Reads `/sys/class/power_supply/*/capacity` and `status`. Fires alerts at descending thresholds (20%, 10%, 5%). Alerts auto-reset once battery charges back above threshold + 5%.
- **`src/notify/hooks.rs`**:
  - `install_hooks(home: &str) -> Result<()>`:
    1. Installs systemd service `ryoiki-notify-boot.service` (`ExecStart=ryoiki notify boot`).
    2. Installs systemd service `ryoiki-battery-watch.service`.
    3. Installs udev rule `/etc/udev/rules.d/99-ryoiki-power.rules` triggering `ryoiki notify power --status plugged/unplugged`.
    4. Appends PAM/bashrc login security trap triggering `ryoiki notify login --user ... --ip ...`.
- **`src/notify/server.rs`**:
  - `spawn_background_server(config, port)`: Micro-HTTP server handling webhook requests (`GET /torrent?event=...&hash=...` and `POST /notify`).
- **`src/notify/system.rs`**:
  - `collect_system_metrics() -> SystemMetrics`: Reads `/proc/stat` for multi-core CPU usage percentage, `/proc/meminfo` for RAM usage, `statvfs` for root disk usage, public IP via `api.ipify.org` (or local fallback), and `/proc/uptime`.

---

## 7. Battery Charge Limiter (`src/charge_limit/`)

Designed for laptops repurposed as 24/7 home servers to prevent lithium-ion battery swelling and degradation.

### Hardware Detection Hierarchy (`src/charge_limit.rs`)
1. **Sysfs Backend (`charge_limit/sysfs.rs`)**:
   - Searches `/sys/class/power_supply/BAT*/charge_control_end_threshold`.
   - Writes limit (e.g. 60 or 80) directly.
   - Persists via udev rule `/etc/udev/rules.d/99-battery-charge-limit.rules` and systemd unit `/etc/systemd/system/ryoiki-charge-limit.service`.
2. **HP ACPI Backend (`charge_limit/hp_acpi.rs`)**:
   - Detects HP laptops with ACPI battery health manager support (`/sys/devices/platform/hp-wmi/`).
   - Configures `/etc/ryoiki/charge_limit`.
3. **TLP Backend (`charge_limit/tlp.rs`)**:
   - Configures ThinkPads and supported hardware via `/etc/tlp.conf` (`START_CHARGE_THRESH_BAT0`, `STOP_CHARGE_THRESH_BAT0`).
4. **Unsupported Hardware Guide (`charge_limit/guide.rs`)**:
   - Automatically detects laptop vendor and model via DMI (`/sys/class/dmi/id/sys_vendor` and `product_name`).
   - Renders tailored BIOS setup instructions and kernel module documentation for Asus, Dell, Apple, Lenovo, etc.

---

## 8. Torrent & Rclone Cloud Ecosystem

### Torrent Automation (`src/modules/torrent/`)
- **Web API Client (`torrent/api.rs`)**: Interacts with qBittorrent Web UI via HTTP cookies (`/api/v2/auth/login`, `/api/v2/torrents/info`, `/api/v2/torrents/add`).
- **2-Way Telegram Bot Daemon (`torrent/bot.rs`)**:
  - Runs in background polling Telegram Bot API `getUpdates`.
  - Accepts raw magnet links (`magnet:?xt=urn:...`) and forwards them instantly to qBittorrent.
  - Accepts `.torrent` document files sent in chat, downloads them, and submits them to qBittorrent.
  - Provides commands: `/status` (live transfer speeds, active downloads, ETA), `/help`, `/pause`, `/resume`.
  - Ingestion Loop: Automatically listens for torrent completion signals and triggers `media::organizer` to classify and ingest the finished downloads into Jellyfin.

### Rclone Google Drive Integration (`src/modules/rclone/`)
- **`rclone/install.rs`**: Installs official Rclone binary.
- **`rclone/oauth.rs`**: Guides user through headless OAuth authorization with Google Drive.
- **`rclone/service.rs`**: Generates and enables systemd service `rclone-mount.service`:
  - Mounts `gdrive:` to `/mnt/gdrive`.
  - Flags: `--vfs-cache-mode full --vfs-cache-max-size 20G --allow-other --dir-cache-time 72h`.

---

## 9. Jellyfin Integration & Backup (`src/modules/jellyfin/`)

- **Container Deployment (`jellyfin.rs`)**: Deploys `jellyfin/jellyfin:latest` with GPU passthrough (`/dev/dri:/dev/dri`), mapping `/media` to host media directory.
- **Library Refresh API (`jellyfin/api.rs`)**:
  - `trigger_library_refresh(config: &TelegramConfig) -> Result<()>`: Sends authenticated POST request to `/Library/Refresh` to instantly index newly organized media.
  - `fetch_watched_media(...) -> Result<HashSet<String>>`: Queries playback reporting API to identify completed media for disk pruner.
- **Cloud Backup Engine (`jellyfin/backup.rs` & `timer.rs`)**:
  - Creates compressed snapshot of `/var/lib/jellyfin/data` (database, user playback states, collections).
  - Syncs backup archive to `gdrive:ryoiki-backups/jellyfin/` via rclone.
  - Manages systemd timer `ryoiki-jellyfin-backup.timer` running nightly at 03:00.

---

## 10. CLI Command Reference

```bash
# Run interactive TUI to pick and install modules
ryoiki

# Provision all modules with default prompts (headless)
ryoiki --all --yes

# Deploy configuration dotfiles only
ryoiki dotfiles

# Audit installed system and developer CLI tools
ryoiki check

# Execute specific modules by ID
ryoiki run essentials cli_tools docker

# Update ryoiki binary to latest GitHub release
ryoiki update

# Start 2-way Telegram bot daemon
ryoiki bot

# Configure laptop battery charge limit
ryoiki charge-limit

# Send custom Telegram alert
ryoiki notify send "Server maintenance scheduled" --title "Ops" --level warn

# Ingest and organize media files into Jellyfin
ryoiki organize /path/to/media/file_or_dir

# Preview media organization without moving files
ryoiki organize --dry-run

# Run smart SSD media pruner
ryoiki prune --threshold 80 --target 70

# Setup 6-hour automated prune timer
ryoiki prune --schedule

# Inspect storage status (SSD vs Google Drive)
ryoiki storage

# Interactive media manager (push, pull, sync)
ryoiki media

# Setup daily post-boot Google Drive media sync
ryoiki media --schedule

# Manage Rclone Google Drive mount
ryoiki rclone setup
ryoiki rclone status

# Manage Jellyfin cloud backups
ryoiki backup create
ryoiki backup schedule
```

---

## 11. Testing & Code Quality Gates

To verify code changes locally, run:
```bash
# Run unit tests (60/60 tests passing)
cargo test --all-targets

# Enforce zero clippy warnings
cargo clippy --all-targets -- -D warnings

# Verify formatting
cargo fmt --check
```

### GitHub Actions CI/CD
- `.github/workflows/ci.yml`: Triggers on push and pull requests to `main`. Executes `cargo fmt`, `cargo clippy`, and `cargo test`.
- `.github/workflows/release.yml`: Triggers on tag push (`v*`). Builds optimized binaries for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (cross-compiled via `aarch64-linux-gnu-gcc`), packages `.tar.gz` archives, and publishes GitHub Release with attached assets.

---

> [!IMPORTANT]
> **REMINDER FOR ALL AI AGENTS**: Any time you modify, add, or delete any files in `src/`, `configs/`, or project manifests, update this `codebase.md` file immediately in the same step. Keep this specification synchronized!
