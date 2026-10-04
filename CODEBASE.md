# CODEBASE.md: ryoiki Semantic Digest

> **Notice**: AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
Entrypoint ──> CLI/Parser ──> Domain Logic ──> Infra/IO
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language**: Rust 2021 edition
- **Architectural Paradigm**: Role-based (domain/, infra/, api/cli/, tui/)
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production unwrap(), 0 warnings.
- **Target Distribution**: Linux x86_64 standalone binary

## 3. Module & Interface Skeleton

### `src/bot/actions.rs` (Role: general, Lines: 112)
- **Responsibility**: Core general logic in src/bot/actions.rs
- **Imports**: use anyhow :: { bail , Result } , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn handle_charge_limit (arg : & str) -> String
  fn execute_reboot () -> Result < String >
  fn execute_poweroff () -> Result < String >
  fn handle_self_update () -> String
  ```

### `src/bot/callbacks.rs` (Role: general, Lines: 177)
- **Responsibility**: Core general logic in src/bot/callbacks.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: Client , use super :: actions :: { execute_poweroff , execute_reboot } , use super :: client :: edit_message , use super :: maintenance :: { handle_bot_audio , handle_bot_check , handle_bot_organize , handle_bot_prune , handle_bot_sync , } , use super :: services :: { get_docker_containers , get_managed_services } , use super :: system :: collect_system_metrics , use super :: torrents :: render_torrent_report , use super :: ui :: { render_docker_view , render_maintenance_view , render_services_view , render_storage_view , render_system_view , render_unified_status , } , use crate :: notify :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn handle_callback_query (client : & Client , config : & TelegramConfig , prompts : & crate :: bot :: prompts :: Prompts , data : & str , msg_id : i64 ,) -> Result < () >
  ```

### `src/bot/client.rs` (Role: cli, Lines: 133)
- **Responsibility**: Core cli logic in src/bot/client.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use serde_json :: json , use super :: types :: { FileResult , InlineKeyboardMarkup , TelegramResponse , Update } 
- **Public Functions & Signatures**:
  ```rust
  fn fetch_updates (client : & Client , token : & str , offset : i64) -> Result < Vec < Update > >
  fn reply (client : & Client , token : & str , chat_id : & str , text : & str) -> Result < () >
  fn reply_with_keyboard (client : & Client , token : & str , chat_id : & str , text : & str , keyboard : & InlineKeyboardMarkup ,) -> Result < () >
  fn reply_with_keyboard_id (client : & Client , token : & str , chat_id : & str , text : & str , keyboard : & InlineKeyboardMarkup ,) -> Result < i64 >
  fn edit_message (client : & Client , token : & str , chat_id : & str , message_id : i64 , text : & str , keyboard : Option < & InlineKeyboardMarkup > ,) -> Result < () >
  fn answer_callback (client : & Client , token : & str , callback_id : & str , toast : Option < & str > ,) -> Result < () >
  fn download_telegram_file (client : & Client , token : & str , file_id : & str ,) -> Result < (String , Vec < u8 >) >
  ```

### `src/bot/keyboards.rs` (Role: general, Lines: 121)
- **Responsibility**: Core general logic in src/bot/keyboards.rs
- **Imports**: use super :: types :: { InlineKeyboardButton , InlineKeyboardMarkup } 
- **Public Functions & Signatures**:
  ```rust
  fn status_keyboard () -> InlineKeyboardMarkup
  fn system_keyboard () -> InlineKeyboardMarkup
  fn storage_keyboard () -> InlineKeyboardMarkup
  fn docker_keyboard () -> InlineKeyboardMarkup
  fn services_keyboard () -> InlineKeyboardMarkup
  fn maintenance_keyboard () -> InlineKeyboardMarkup
  fn seedr_queue_keyboard (hash : & str) -> InlineKeyboardMarkup
  fn reboot_keyboard () -> InlineKeyboardMarkup
  fn poweroff_keyboard () -> InlineKeyboardMarkup
  ```

### `src/bot/maintenance.rs` (Role: general, Lines: 227)
- **Responsibility**: Core general logic in src/bot/maintenance.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: Client , use std :: time :: Duration , use crate :: modules :: torrent :: api :: { self , TorrentInfo } , use crate :: notify :: client :: escape_html , use crate :: notify :: config :: TelegramConfig , use crate :: runner :: Runner 
- **Public Functions & Signatures**:
  ```rust
  fn handle_bot_organize (config : & TelegramConfig) -> Result < String >
  fn handle_bot_prune () -> Result < String >
  fn handle_bot_sync () -> String
  fn handle_bot_audio (action : & str) -> Result < String >
  fn handle_bot_check () -> String
  ```

### `src/bot/prompts.rs` (Role: general, Lines: 110)
- **Responsibility**: Core general logic in src/bot/prompts.rs
- **Imports**: use std :: collections :: HashMap , use std :: sync :: { Arc , Mutex } , use std :: time :: Duration 
- **Types & Enums**:
  ```rust
  pub struct PendingPrompt
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new_registry () -> Prompts
  fn register (prompts : & Prompts , hash : & str , msg_id : i64)
  fn take (prompts : & Prompts , hash : & str) -> Option < PendingPrompt >
  fn spawn_timeout < F > (prompts : & Prompts , hash : & str , timeout_secs : u64 , on_timeout : F) where F : FnOnce (i64) + Send + 'static ,
  ```

### `src/bot/router/commands.rs` (Role: general, Lines: 228)
- **Responsibility**: Core general logic in src/bot/router/commands.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: Client , use super :: super :: actions :: { handle_charge_limit , handle_self_update } , use super :: super :: client :: { reply , reply_with_keyboard } , use super :: super :: maintenance :: { handle_bot_audio , handle_bot_check , handle_bot_organize , handle_bot_prune , handle_bot_sync , } , use super :: super :: services :: { get_docker_containers , get_docker_logs , get_managed_services , restart_docker_container , restart_managed_service , } , use super :: super :: system :: collect_system_metrics , use super :: super :: torrents :: { handle_seedr_cmd , pause_all , render_torrent_report , resume_all } , use super :: super :: ui :: { render_docker_view , render_poweroff_confirm , render_reboot_confirm , render_services_view , render_storage_view , render_system_view , render_unified_status , } , use crate :: notify :: client :: escape_html , use crate :: notify :: config :: TelegramConfig 

### `src/bot/router.rs` (Role: general, Lines: 181)
- **Responsibility**: Core general logic in src/bot/router.rs
- **Imports**: use reqwest :: blocking :: Client , use super :: callbacks :: handle_callback_query , use super :: client :: { download_telegram_file , reply } , use super :: torrents :: { handle_magnet , handle_torrent_file , MagnetOutcome } , use super :: types :: { CallbackQuery , Message } , use crate :: notify :: client :: escape_html , use crate :: notify :: config :: TelegramConfig , use commands :: dispatch_command 
- **Public Functions & Signatures**:
  ```rust
  fn route_message (client : & Client , config : & TelegramConfig , prompts : & crate :: bot :: prompts :: Prompts , msg : Message ,)
  fn route_callback (client : & Client , config : & TelegramConfig , prompts : & crate :: bot :: prompts :: Prompts , cb : CallbackQuery ,)
  fn extract_command (text : & str) -> Option < (& str , & str) >
  ```

### `src/bot/services.rs` (Role: general, Lines: 276)
- **Responsibility**: Core general logic in src/bot/services.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use std :: process :: Command 
- **Types & Enums**:
  ```rust
  pub struct ContainerInfo
  pub struct ServiceInfo
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_docker_containers () -> Result < Vec < ContainerInfo > >
  fn restart_docker_container (name : & str) -> Result < String >
  fn get_docker_logs (name : & str , tail : usize) -> Result < String >
  fn get_managed_services () -> Vec < ServiceInfo >
  fn resolve_managed_unit (name : & str) -> Option < (& 'static str , bool) >
  fn restart_managed_service (name : & str) -> Result < String >
  ```

### `src/bot/smoke_tests.rs` (Role: general, Lines: 179)
- **Responsibility**: Core general logic in src/bot/smoke_tests.rs
- **Imports**: use super :: actions :: handle_charge_limit , use super :: maintenance :: { handle_bot_audio , handle_bot_check } , use super :: router :: extract_command , use super :: services :: { get_docker_containers , get_docker_logs , get_managed_services , resolve_managed_unit , } , use super :: system :: collect_system_metrics , use super :: torrents :: handle_seedr_cmd , use super :: ui :: { render_docker_view , render_maintenance_view , render_poweroff_confirm , render_reboot_confirm , render_services_view , render_storage_view , render_system_view , } , use crate :: notify :: config :: TelegramConfig 

### `src/bot/system.rs` (Role: general, Lines: 265)
- **Responsibility**: Core general logic in src/bot/system.rs
- **Imports**: use std :: ffi :: CString , use std :: fs , use std :: mem :: MaybeUninit , use std :: path :: Path , use std :: process :: Command , use crate :: notify :: config :: TelegramConfig , use crate :: notify :: power :: { is_ac_online , read_battery_percent , read_battery_status , read_cpu_temp , read_cpu_usage , } , use crate :: notify :: system :: { get_hostname , get_kernel , get_memory_stats , get_public_ip , get_tailscale_ip , get_uptime , } 
- **Types & Enums**:
  ```rust
  pub struct DiskMount
  pub struct SystemMetrics
  ```
- **Public Functions & Signatures**:
  ```rust
  fn collect_system_metrics (config : & TelegramConfig) -> SystemMetrics
  fn get_load_averages () -> (f32 , f32 , f32)
  fn get_swap_stats () -> (u64 , u64)
  fn get_local_ip () -> String
  fn get_disk_info (path : & str) -> Option < (u64 , u64 , u64) >
  ```

### `src/bot/torrents.rs` (Role: general, Lines: 268)
- **Responsibility**: Core general logic in src/bot/torrents.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: Client , use std :: collections :: HashMap , use std :: time :: Duration , use crate :: modules :: torrent :: api :: { self , TorrentInfo } , use crate :: modules :: torrent :: notify , use crate :: modules :: torrent :: report :: format_status_report , use crate :: modules :: torrent :: seedr , use crate :: notify :: config :: TelegramConfig , use crate :: modules :: torrent :: dedup :: { self , Availability } , use crate :: modules :: torrent :: scheduler :: { self , SubmitOutcome } 
- **Types & Enums**:
  ```rust
  pub enum MagnetOutcome
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_torrents (client : & Client , url : & str) -> Result < Vec < TorrentInfo > >
  fn render_torrent_report (client : & Client , url : & str) -> Result < String >
  fn pause_all (client : & Client , url : & str) -> Result < () >
  fn resume_all (client : & Client , url : & str) -> Result < () >
  fn handle_magnet (client : & Client , config : & TelegramConfig , magnet : & str) -> MagnetOutcome
  fn handle_seedr_cmd (target : & str , api_port : u16) -> String
  fn handle_torrent_file (client : & Client , config : & TelegramConfig , fname : & str , bytes : Vec < u8 > ,) -> Result < String >
  fn start_torrent_monitor (config : TelegramConfig)
  ```

### `src/bot/types.rs` (Role: general, Lines: 121)
- **Responsibility**: Core general logic in src/bot/types.rs
- **Imports**: use serde :: { Deserialize , Serialize } 
- **Types & Enums**:
  ```rust
  pub struct TelegramResponse
  pub struct Update
  pub struct Message
  pub struct CallbackQuery
  pub struct User
  pub struct Document
  pub struct FileResult
  pub struct InlineKeyboardMarkup
  pub struct InlineKeyboardButton
  ```
- **Public Functions & Signatures**:
  ```rust
  fn callback (text : & str , data : & str) -> Self
  ```

### `src/bot/ui.rs` (Role: tui, Lines: 371)
- **Responsibility**: Core tui logic in src/bot/ui.rs
- **Imports**: use super :: keyboards :: { docker_keyboard , maintenance_keyboard , poweroff_keyboard , reboot_keyboard , services_keyboard , status_keyboard , storage_keyboard , system_keyboard , } , use super :: services :: { ContainerInfo , ServiceInfo } , use super :: system :: { DiskMount , SystemMetrics } , use super :: types :: InlineKeyboardMarkup , use crate :: modules :: media :: disk :: format_bytes 
- **Public Functions & Signatures**:
  ```rust
  fn render_unified_status (sys : & SystemMetrics , torrent_count : usize , active_containers : usize ,) -> (String , InlineKeyboardMarkup)
  fn render_system_view (sys : & SystemMetrics) -> (String , InlineKeyboardMarkup)
  fn render_storage_view (disks : & [DiskMount] , gdrive : Option < & str > ,) -> (String , InlineKeyboardMarkup)
  fn render_docker_view (containers : & [ContainerInfo]) -> (String , InlineKeyboardMarkup)
  fn render_services_view (services : & [ServiceInfo]) -> (String , InlineKeyboardMarkup)
  fn render_maintenance_view () -> (String , InlineKeyboardMarkup)
  fn render_reboot_confirm () -> (String , InlineKeyboardMarkup)
  fn render_poweroff_confirm () -> (String , InlineKeyboardMarkup)
  fn make_progress_bar (pct : u8 , width : usize) -> String
  ```

### `src/bot.rs` (Role: general, Lines: 53)
- **Responsibility**: Core general logic in src/bot.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: time :: Duration , use crate :: notify :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn run_bot () -> Result < () >
  ```

### `src/charge_limit/guide.rs` (Role: tui, Lines: 117)
- **Responsibility**: Core tui logic in src/charge_limit/guide.rs
- **Imports**: use anyhow :: Result , use colored :: Colorize , use std :: fs , use std :: path :: { Path , PathBuf } 
- **Public Functions & Signatures**:
  ```rust
  fn detect_vendor () -> String
  fn detect_model () -> String
  fn config_path () -> PathBuf
  fn save_charge_limit_config (limit : u8) -> Result < PathBuf >
  fn load_configured_limit () -> Option < u8 >
  fn print_unsupported_hardware_guide (vendor : & str , model : & str , target_limit : u8)
  ```

### `src/charge_limit/hp_acpi.rs` (Role: general, Lines: 137)
- **Responsibility**: Core general logic in src/charge_limit/hp_acpi.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use std :: path :: Path , use std :: process :: Command , use super :: sysfs :: { read_privileged_file , write_privileged_file } 
- **Types & Enums**:
  ```rust
  pub enum HpChargeMode
  ```
- **Public Functions & Signatures**:
  ```rust
  fn hex_arg (self) -> & 'static str
  fn label (self) -> & 'static str
  fn is_hp_acpi_supported () -> bool
  fn call_acpi (call_str : & str) -> Result < String >
  fn get_hp_charge_mode () -> Result < HpChargeMode >
  fn set_hp_charge_mode (mode : HpChargeMode) -> Result < () >
  fn regulate_hp_battery (target_limit : u8 , current_pct : u8 , ac_online : bool ,) -> Result < HpChargeMode >
  ```

### `src/charge_limit/sysfs.rs` (Role: general, Lines: 157)
- **Responsibility**: Core general logic in src/charge_limit/sysfs.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use std :: fs , use std :: io :: Write , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn apply_sysfs_limit (battery : & Path , limit : u8) -> Result < () >
  fn battery_name (battery : & Path) -> String
  fn persist_udev_rule (battery : & Path , limit : u8) -> Result < () >
  fn persist_systemd_service (battery : & Path , limit : u8) -> Result < () >
  fn write_privileged_file (path : & Path , content : & str) -> Result < () >
  fn read_privileged_file (path : & Path) -> Result < String >
  ```

### `src/charge_limit/tlp.rs` (Role: general, Lines: 168)
- **Responsibility**: Core general logic in src/charge_limit/tlp.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command , use super :: guide :: detect_vendor , use super :: sysfs :: write_privileged_file , use super :: MIN_LIMIT 
- **Public Functions & Signatures**:
  ```rust
  fn is_tlp_supported_for_hardware () -> bool
  fn apply_tlp_limit (limit : u8) -> Result < () >
  fn ensure_tlp_installed () -> Result < () >
  fn configure_tlp_thresholds (stop : u8) -> Result < () >
  fn update_tlp_conf (conf : & str , start : u8 , stop : u8) -> String
  fn reload_tlp () -> Result < () >
  ```

### `src/charge_limit.rs` (Role: general, Lines: 262)
- **Responsibility**: Core general logic in src/charge_limit.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use colored :: Colorize , use std :: fs , use std :: io :: { self , BufRead , Write } , use std :: path :: { Path , PathBuf } , use std :: process :: Command , pub use guide :: load_configured_limit 
- **Types & Enums**:
  ```rust
  pub enum Backend
  ```
- **Public Functions & Signatures**:
  ```rust
  fn detect_backend () -> Result < Backend >
  fn run (yes : bool) -> Result < () >
  fn apply_limit_programmatic (limit : u8) -> Result < String >
  ```

### `src/configs.rs` (Role: general, Lines: 160)
- **Responsibility**: Core general logic in src/configs.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs , use std :: io :: Write , use std :: path :: Path , use std :: process :: { Command , Stdio } 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_dotfiles (home : & str) -> Result < () >
  ```

### `src/main.rs` (Role: general, Lines: 350)
- **Responsibility**: Core general logic in src/main.rs
- **Imports**: use anyhow :: Result , use clap :: { Parser , Subcommand } , use colored :: Colorize , use modules :: { execute_module , get_available_modules } , use runner :: Runner , use std :: io :: IsTerminal 

### `src/modules/cli_tools.rs` (Role: cli, Lines: 52)
- **Responsibility**: Core cli logic in src/modules/cli_tools.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use std :: fs , use std :: io :: Write , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/dev_runtimes.rs` (Role: general, Lines: 54)
- **Responsibility**: Core general logic in src/modules/dev_runtimes.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use std :: fs , use std :: io :: Write , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/docker.rs` (Role: general, Lines: 63)
- **Responsibility**: Core general logic in src/modules/docker.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/dubstrip.rs` (Role: general, Lines: 66)
- **Responsibility**: Core general logic in src/modules/dubstrip.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: io :: { self , BufRead , Write } , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/essentials.rs` (Role: general, Lines: 60)
- **Responsibility**: Core general logic in src/modules/essentials.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/git_ssh.rs` (Role: general, Lines: 250)
- **Responsibility**: Core general logic in src/modules/git_ssh.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: io :: { self , BufRead , Write } , use std :: path :: { Path , PathBuf } 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/headless_audio.rs` (Role: general, Lines: 69)
- **Responsibility**: Core general logic in src/modules/headless_audio.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: { Path , PathBuf } 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/jellyfin/api.rs` (Role: api, Lines: 131)
- **Responsibility**: Core api logic in src/modules/jellyfin/api.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: path :: Path , use std :: time :: Duration , use crate :: notify :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn trigger_library_refresh (client : & Client , base_url : & str , api_key : Option < & str > ,) -> Result < () >
  fn refresh_library_auto () -> Result < () >
  fn refresh_library_async ()
  ```

### `src/modules/jellyfin/backup.rs` (Role: general, Lines: 253)
- **Responsibility**: Core general logic in src/modules/jellyfin/backup.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use clap :: Subcommand , use colored :: Colorize , use std :: fs , use std :: path :: { Path , PathBuf } , use std :: process :: Command , use std :: time :: Instant , use super :: timer , use crate :: modules :: rclone , use crate :: notify :: client :: format_card , use crate :: notify :: TelegramConfig 
- **Types & Enums**:
  ```rust
  pub enum BackupSubcommand
  ```
- **Public Functions & Signatures**:
  ```rust
  fn handle_cli (sub : BackupSubcommand) -> Result < () >
  fn run_backup_now () -> Result < () >
  fn list_backups () -> Result < () >
  fn setup_schedule () -> Result < () >
  ```

### `src/modules/jellyfin/encoding.rs` (Role: general, Lines: 163)
- **Responsibility**: Core general logic in src/modules/jellyfin/encoding.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn provision_hardware_encoding (base_dir : & Path , dry_run : bool) -> Result < () >
  fn patch_encoding_xml (content : & str) -> String
  ```

### `src/modules/jellyfin/kodi.rs` (Role: general, Lines: 129)
- **Responsibility**: Core general logic in src/modules/jellyfin/kodi.rs
- **Imports**: use anyhow :: { Context , Result } , use clap :: Subcommand , use colored :: Colorize , use std :: fs , use std :: path :: PathBuf 
- **Types & Enums**:
  ```rust
  pub enum KodiSubcommand
  ```
- **Public Functions & Signatures**:
  ```rust
  fn handle_cli (sub : & KodiSubcommand) -> Result < () >
  ```

### `src/modules/jellyfin/timer.rs` (Role: general, Lines: 70)
- **Responsibility**: Core general logic in src/modules/jellyfin/timer.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_backup_timer (home : & str) -> Result < () >
  fn is_timer_active () -> bool
  ```

### `src/modules/jellyfin.rs` (Role: general, Lines: 121)
- **Responsibility**: Core general logic in src/modules/jellyfin.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/media/ai/batch.rs` (Role: general, Lines: 48)
- **Responsibility**: Core general logic in src/modules/media/ai/batch.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: Client , use std :: collections :: HashMap , use super :: super :: probe :: MediaProbe , use super :: super :: MediaInfo , use super :: client :: send_ai_prompt , use super :: prompt :: build_batch_prompt , use super :: schema :: parse_batch_ai_json 
- **Public Functions & Signatures**:
  ```rust
  fn classify_media_batch (client : & Client , api_key : Option < & str > , items : & [(& str , Option < & MediaProbe >)] ,) -> Result < HashMap < String , MediaInfo > >
  fn apply_probe_fallback (info : & mut MediaInfo , probe : Option < & MediaProbe >)
  ```

### `src/modules/media/ai/client.rs` (Role: cli, Lines: 227)
- **Responsibility**: Core cli logic in src/modules/media/ai/client.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use reqwest :: blocking :: Client , use serde :: Deserialize , use serde_json :: json , use std :: thread :: sleep , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn clean_json_text (text : & str) -> String
  fn send_ai_prompt (client : & Client , gemini_api_key : Option < & str > , prompt : & str ,) -> Result < String >
  fn send_deepseek_prompt (client : & Client , prompt : & str) -> Result < String >
  fn send_gemini_prompt (client : & Client , api_key : & str , prompt : & str) -> Result < String >
  ```

### `src/modules/media/ai/prompt.rs` (Role: general, Lines: 102)
- **Responsibility**: Core general logic in src/modules/media/ai/prompt.rs
- **Imports**: use super :: super :: probe :: MediaProbe , use std :: fmt :: Write 
- **Public Functions & Signatures**:
  ```rust
  fn build_single_prompt (raw_name : & str , probe : Option < & MediaProbe >) -> String
  fn build_batch_prompt (raw_names : & [& str] , probe : Option < & MediaProbe >) -> String
  fn format_probe_context (probe : Option < & MediaProbe >) -> String
  ```

### `src/modules/media/ai/schema.rs` (Role: general, Lines: 258)
- **Responsibility**: Core general logic in src/modules/media/ai/schema.rs
- **Imports**: use anyhow :: { Context , Result } , use serde :: Deserialize , use std :: path :: Path , use super :: super :: { ClassificationEngine , MediaInfo , MediaType } 
- **Types & Enums**:
  ```rust
  pub struct AiOutputSchema
  pub struct AiBatchItemSchema
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_ai_json (json_text : & str , raw_name : & str) -> Result < MediaInfo >
  fn parse_batch_ai_json (json_text : & str) -> Result < Vec < (String , MediaInfo) > >
  fn ensure_year_in_clean_name (name : & str , year : u32) -> String
  fn ensure_language_in_clean_name (name : & str , language : & str) -> String
  ```

### `src/modules/media/ai.rs` (Role: general, Lines: 30)
- **Responsibility**: Core general logic in src/modules/media/ai.rs
- **Imports**: pub use batch :: classify_media_batch , # [allow (unused_imports)] pub use client :: { clean_json_text , send_ai_prompt , send_deepseek_prompt , send_gemini_prompt } , pub use schema :: ensure_language_in_clean_name , use anyhow :: Result , use reqwest :: blocking :: Client , use super :: probe :: MediaProbe , use super :: MediaInfo , use prompt :: build_single_prompt , use schema :: parse_ai_json 
- **Public Functions & Signatures**:
  ```rust
  fn classify_media_ai (client : & Client , api_key : Option < & str > , raw_name : & str , probe : Option < & MediaProbe > ,) -> Result < MediaInfo >
  ```

### `src/modules/media/audio/cli.rs` (Role: cli, Lines: 121)
- **Responsibility**: Core cli logic in src/modules/media/audio/cli.rs
- **Imports**: use anyhow :: { Context , Result } , use clap :: Subcommand , use colored :: Colorize , use std :: process :: Command , use super :: { find_dubstrip_bin , strip_queue } 
- **Types & Enums**:
  ```rust
  pub enum AudioSubcommand
  ```
- **Public Functions & Signatures**:
  ```rust
  fn handle_cli (sub : AudioSubcommand) -> Result < () >
  ```

### `src/modules/media/audio/retry_timer.rs` (Role: general, Lines: 87)
- **Responsibility**: Core general logic in src/modules/media/audio/retry_timer.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_retry_timer (home : & str) -> Result < () >
  fn is_timer_active () -> bool
  ```

### `src/modules/media/audio/strip_queue.rs` (Role: general, Lines: 162)
- **Responsibility**: Core general logic in src/modules/media/audio/strip_queue.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: path :: { Path , PathBuf } , use std :: { fs , time } 
- **Types & Enums**:
  ```rust
  pub struct QueueEntry
  ```
- **Public Functions & Signatures**:
  ```rust
  fn enqueue (path : & Path , preserve_multi : bool)
  fn process_queue (dubstrip_bin : & Path) -> Result < super :: AudioStripSummary >
  ```

### `src/modules/media/audio/tests.rs` (Role: general, Lines: 100)
- **Responsibility**: Core general logic in src/modules/media/audio/tests.rs
- **Imports**: use super :: * , use anyhow :: { Context , Result } , use std :: fs 

### `src/modules/media/audio.rs` (Role: general, Lines: 350)
- **Responsibility**: Core general logic in src/modules/media/audio.rs
- **Imports**: pub use cli :: { handle_cli , AudioSubcommand } , use colored :: Colorize , use std :: path :: { Path , PathBuf } , use std :: process :: Command 
- **Types & Enums**:
  ```rust
  pub enum StripOutcome
  pub struct AudioStripSummary
  ```
- **Public Functions & Signatures**:
  ```rust
  fn find_dubstrip_bin () -> Option < PathBuf >
  fn is_empty (& self) -> bool
  fn record (& mut self , outcome : & StripOutcome)
  fn merge (& mut self , other : & Self)
  fn render_html_line (& self) -> Option < String >
  fn reclaimed_display (& self) -> String
  fn strip_audio_auto (path : & Path , preserve_multi : bool) -> StripOutcome
  fn strip_and_preserve (bin : & Path , path : & Path , preserve_multi : bool) -> StripOutcome
  fn sync_filename_after_strip (path : & Path)
  ```

### `src/modules/media/config.rs` (Role: general, Lines: 141)
- **Responsibility**: Core general logic in src/modules/media/config.rs
- **Imports**: use anyhow :: Result , use colored :: Colorize , use std :: io :: { self , BufRead , Write } , use crate :: notify :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn get_or_prompt_gemini_key (interactive : bool) -> Option < String >
  fn is_deepseek_enabled () -> bool
  fn get_deepseek_url () -> String
  fn get_deepseek_model () -> String
  fn get_deepseek_api_key () -> String
  fn get_gemini_model () -> String
  fn is_ai_enabled () -> bool
  ```

### `src/modules/media/disk.rs` (Role: general, Lines: 95)
- **Responsibility**: Core general logic in src/modules/media/disk.rs
- **Imports**: use anyhow :: { bail , Result } , use std :: ffi :: CString , use std :: path :: Path 
- **Types & Enums**:
  ```rust
  pub struct DiskUsage
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_disk_usage (path : & Path) -> Result < DiskUsage >
  fn compute_bytes_to_free (usage : DiskUsage , target_pct : u8) -> u64
  fn format_bytes (bytes : u64) -> String
  ```

### `src/modules/media/heuristic/tests.rs` (Role: general, Lines: 115)
- **Responsibility**: Core general logic in src/modules/media/heuristic/tests.rs
- **Imports**: use super :: * 

### `src/modules/media/heuristic.rs` (Role: general, Lines: 366)
- **Responsibility**: Core general logic in src/modules/media/heuristic.rs
- **Imports**: use super :: { ClassificationEngine , MediaInfo , MediaType } , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn classify_media_heuristic (raw_name : & str) -> MediaInfo
  ```

### `src/modules/media/interactive/events.rs` (Role: general, Lines: 360)
- **Responsibility**: Core general logic in src/modules/media/interactive/events.rs
- **Imports**: use crossterm :: event :: KeyCode , use super :: ui :: files_view :: handle_files_key , use super :: { AppState , MediaItem , TransferDirection , ViewMode } , use crate :: modules :: media :: disk 
- **Public Functions & Signatures**:
  ```rust
  fn handle_key (code : KeyCode , state : & mut AppState) -> Option < bool >
  ```

### `src/modules/media/interactive/selection.rs` (Role: general, Lines: 138)
- **Responsibility**: Core general logic in src/modules/media/interactive/selection.rs
- **Imports**: use super :: { AppState , MediaItem , TransferDirection } , use crate :: modules :: media :: disk 
- **Public Functions & Signatures**:
  ```rust
  fn toggle_main_item (state : & mut AppState , real_idx : usize)
  fn collect_transfer_items (state : & AppState) -> Vec < MediaItem >
  ```

### `src/modules/media/interactive/ui/files_view.rs` (Role: tui, Lines: 297)
- **Responsibility**: Core tui logic in src/modules/media/interactive/ui/files_view.rs
- **Imports**: use crossterm :: event :: KeyCode , use ratatui :: { layout :: { Alignment , Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , BorderType , Borders , Paragraph } , Frame , } , use super :: truncate_str , use crate :: modules :: media :: disk , use crate :: modules :: media :: interactive :: { AppState , ViewMode } , use crate :: modules :: media :: transfer :: { MediaFile , TransferDirection } 
- **Public Functions & Signatures**:
  ```rust
  fn render_files_ui (f : & mut Frame , state : & AppState , item_idx : usize , season_idx : Option < usize > , cursor : usize ,)
  fn handle_files_key (code : KeyCode , state : & mut AppState , item_idx : usize , season_idx : Option < usize > , mut cursor : usize ,) -> Option < bool >
  ```

### `src/modules/media/interactive/ui/footer.rs` (Role: tui, Lines: 30)
- **Responsibility**: Core tui logic in src/modules/media/interactive/ui/footer.rs
- **Imports**: use ratatui :: { layout :: { Alignment , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , BorderType , Borders , Paragraph } , Frame , } 
- **Public Functions & Signatures**:
  ```rust
  fn render_footer (f : & mut Frame , area : Rect , shortcuts : & [(& str , & str)])
  ```

### `src/modules/media/interactive/ui/main_view.rs` (Role: tui, Lines: 382)
- **Responsibility**: Core tui logic in src/modules/media/interactive/ui/main_view.rs
- **Imports**: use ratatui :: { layout :: { Alignment , Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , BorderType , Borders , Paragraph } , Frame , } , use super :: truncate_str , use crate :: modules :: media :: disk , use crate :: modules :: media :: interactive :: AppState , use crate :: modules :: media :: transfer :: { MediaCategory , MediaItem , TransferDirection } 
- **Public Functions & Signatures**:
  ```rust
  fn render_main_ui (f : & mut Frame , state : & AppState)
  ```

### `src/modules/media/interactive/ui/sub_view.rs` (Role: tui, Lines: 316)
- **Responsibility**: Core tui logic in src/modules/media/interactive/ui/sub_view.rs
- **Imports**: use ratatui :: { layout :: { Alignment , Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , BorderType , Borders , Paragraph } , Frame , } , use super :: truncate_str , use crate :: modules :: media :: disk , use crate :: modules :: media :: interactive :: AppState , use crate :: modules :: media :: transfer :: { MediaItem , MediaSeason , TransferDirection } 
- **Public Functions & Signatures**:
  ```rust
  fn render_subview_ui (f : & mut Frame , state : & AppState , item_idx : usize , cursor : usize)
  ```

### `src/modules/media/interactive/ui.rs` (Role: tui, Lines: 33)
- **Responsibility**: Core tui logic in src/modules/media/interactive/ui.rs
- **Imports**: use ratatui :: Frame , use super :: { AppState , ViewMode } , pub use files_view :: render_files_ui , pub use main_view :: render_main_ui , pub use sub_view :: render_subview_ui 
- **Public Functions & Signatures**:
  ```rust
  fn render_ui (f : & mut Frame , state : & AppState)
  fn truncate_str (s : & str , max_chars : usize) -> String
  ```

### `src/modules/media/interactive.rs` (Role: general, Lines: 280)
- **Responsibility**: Core general logic in src/modules/media/interactive.rs
- **Imports**: use anyhow :: Result , use crossterm :: { event :: { self , Event , KeyEventKind } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , Terminal } , use std :: io , use std :: path :: { Path , PathBuf } , use super :: disk :: DiskUsage , use super :: transfer :: { MediaCategory , MediaItem , TransferDirection } 
- **Types & Enums**:
  ```rust
  pub enum ItemFilter
  pub enum ViewMode
  pub struct AppState
  ```
- **Public Functions & Signatures**:
  ```rust
  fn next (self) -> Self
  fn label (self) -> & 'static str
  fn matches (self , cat : MediaCategory) -> bool
  fn new (local : Vec < MediaItem > , remote : Vec < MediaItem > , dir : TransferDirection , local_disk : Option < DiskUsage > , home_path : PathBuf ,) -> Self
  fn current_items (& self) -> & [MediaItem]
  fn current_items_mut (& mut self) -> & mut [MediaItem]
  fn is_selected (& self , idx : usize) -> bool
  fn current_selected (& mut self) -> & mut [bool]
  fn filtered_indices (& self) -> Vec < usize >
  fn total_selected_bytes (& self) -> u64
  fn total_needed_pull_bytes (& self) -> u64
  fn can_add_bytes (& self , additional_bytes : u64) -> bool
  fn remaining_free_bytes (& self) -> Option < u64 >
  fn reload_libraries (& mut self)
  fn toggle_main_item (& mut self , real_idx : usize)
  fn collect_transfer_items (& self) -> Vec < MediaItem >
  fn run_interactive_tui (local : Vec < MediaItem > , remote : Vec < MediaItem > , dir : TransferDirection , local_disk : Option < DiskUsage > , home_path : & Path ,) -> Result < Option < (TransferDirection , Vec < MediaItem >) > >
  ```

### `src/modules/media/organizer/cli.rs` (Role: cli, Lines: 194)
- **Responsibility**: Core cli logic in src/modules/media/organizer/cli.rs
- **Imports**: use anyhow :: Result , use colored :: Colorize , use reqwest :: blocking :: Client , use std :: path :: Path , use super :: { organize_path , organize_torrent } , use crate :: modules :: media :: OrganizeResult , use crate :: modules :: torrent :: api :: { self , TorrentInfo } , use crate :: runner :: Runner 
- **Public Functions & Signatures**:
  ```rust
  fn run_organize_cli (target : & Path , dry_run : bool) -> Result < () >
  fn cleanup_matching_torrents (client : & Client , base_url : & str , organized_files : & [OrganizeResult] ,) -> usize
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/media/organizer/pathing.rs` (Role: general, Lines: 295)
- **Responsibility**: Core general logic in src/modules/media/organizer/pathing.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs , use std :: path :: { Path , PathBuf } , use crate :: modules :: media :: { MediaInfo , MediaType } 
- **Public Functions & Signatures**:
  ```rust
  fn is_video_file (path : & Path) -> bool
  fn get_jellyfin_media_dir () -> PathBuf
  fn get_jellyfin_backup_multi_dir () -> PathBuf
  fn calculate_dest_dir (info : & MediaInfo) -> PathBuf
  fn resolve_series_dir (parent_category : & Path , title : & str) -> PathBuf
  fn resolve_unique_dest_path (src : & Path , dest_dir : & Path , info : & MediaInfo , dry_run : bool ,) -> PathBuf
  fn perform_move (src : & Path , dst : & Path) -> Result < () >
  ```

### `src/modules/media/organizer.rs` (Role: general, Lines: 393)
- **Responsibility**: Core general logic in src/modules/media/organizer.rs
- **Imports**: pub use cli :: { run_organize_cli , setup } , pub use pathing :: { calculate_dest_dir , is_video_file , perform_move , resolve_unique_dest_path } , use anyhow :: { Context , Result } , use colored :: Colorize , use reqwest :: blocking :: Client , use std :: fs , use std :: path :: { Path , PathBuf } , use super :: ai :: { classify_media_ai , classify_media_batch } , use super :: heuristic :: classify_media_heuristic , use super :: probe :: MediaProbe , use super :: { MediaType , OrganizeResult } , use crate :: modules :: torrent :: api :: TorrentInfo , use std :: collections :: HashMap 
- **Public Functions & Signatures**:
  ```rust
  fn organize_file (file_path : & Path , client : & Client , api_key : Option < & str > , dry_run : bool ,) -> Result < OrganizeResult >
  fn execute_file_organize (file_path : & Path , media_info : super :: MediaInfo , probe : Option < & MediaProbe > , dry_run : bool ,) -> Result < OrganizeResult >
  fn organize_path (target : & Path , client : & Client , api_key : Option < & str > , dry_run : bool ,) -> Result < Vec < OrganizeResult > >
  fn find_videos_recursive (dir : & Path , list : & mut Vec < PathBuf >) -> Result < () >
  fn resolve_torrent_source (torrent : & TorrentInfo , default_dl : & Path) -> PathBuf
  fn organize_torrent (torrent : & TorrentInfo , client : & Client , api_key : Option < & str > , dry_run : bool ,) -> Result < Vec < OrganizeResult > >
  fn organize_completed_torrent (client : & Client , torrent : & TorrentInfo , api_key : Option < & str > ,) -> Result < Option < OrganizeResult > >
  ```

### `src/modules/media/probe.rs` (Role: general, Lines: 267)
- **Responsibility**: Core general logic in src/modules/media/probe.rs
- **Imports**: use serde :: Deserialize , use std :: path :: Path , use std :: process :: Command 
- **Types & Enums**:
  ```rust
  pub struct MediaProbe
  ```
- **Public Functions & Signatures**:
  ```rust
  fn probe_media_file (path : & Path) -> Option < MediaProbe >
  fn parse_ffprobe_json (raw_json : & [u8]) -> Option < MediaProbe >
  fn map_language_code (code : & str) -> String
  ```

### `src/modules/media/prune_timer.rs` (Role: general, Lines: 94)
- **Responsibility**: Core general logic in src/modules/media/prune_timer.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_prune_timer (home : & str , threshold : u8 , target : u8) -> Result < () >
  fn is_timer_active () -> bool
  ```

### `src/modules/media/pruner/execute.rs` (Role: general, Lines: 295)
- **Responsibility**: Core general logic in src/modules/media/pruner/execute.rs
- **Imports**: use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command , use super :: { PruneCandidate , PruneOptions , COLD_ARCHIVE_DEST } , use crate :: modules :: media :: disk , use crate :: notify :: client :: format_card , use crate :: notify :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn process_candidates (candidates : & [PruneCandidate] , target_bytes : u64 , opts : PruneOptions , base : & Path ,)
  fn cleanup_expired_backups (dry_run : bool)
  fn send_prune_alert (names : & [String] , freed_bytes : u64)
  fn send_prune_failure_alert (item_name : & str , error_detail : & str)
  fn extract_error_detail (stderr : & [u8]) -> (String , String)
  ```

### `src/modules/media/pruner/scan.rs` (Role: general, Lines: 103)
- **Responsibility**: Core general logic in src/modules/media/pruner/scan.rs
- **Imports**: use anyhow :: Result , use std :: fs , use std :: path :: Path , use std :: time :: { Duration , SystemTime } , use super :: PruneCandidate 
- **Public Functions & Signatures**:
  ```rust
  fn scan_media_candidates (base : & Path) -> Result < Vec < PruneCandidate > >
  fn tag_watched_status (candidates : & mut [PruneCandidate])
  ```

### `src/modules/media/pruner.rs` (Role: general, Lines: 228)
- **Responsibility**: Core general logic in src/modules/media/pruner.rs
- **Imports**: use anyhow :: Result , use colored :: Colorize , use std :: path :: { Path , PathBuf } , use std :: time :: SystemTime , use super :: disk :: { self , DiskUsage } 
- **Types & Enums**:
  ```rust
  pub struct PruneOptions
  pub struct PruneCandidate
  ```
- **Public Functions & Signatures**:
  ```rust
  fn run_prune (opts : PruneOptions) -> Result < () >
  fn handle_bot_prune () -> Result < String >
  fn sort_candidates (candidates : & mut [PruneCandidate])
  ```

### `src/modules/media/status.rs` (Role: general, Lines: 78)
- **Responsibility**: Core general logic in src/modules/media/status.rs
- **Imports**: use colored :: Colorize , use std :: path :: Path , use std :: process :: Command , use super :: disk :: { self , DiskUsage } 
- **Public Functions & Signatures**:
  ```rust
  fn show_storage_status ()
  ```

### `src/modules/media/sync.rs` (Role: general, Lines: 230)
- **Responsibility**: Core general logic in src/modules/media/sync.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command , use std :: time :: Instant , use crate :: modules :: rclone , use crate :: notify :: client :: format_card , use crate :: notify :: TelegramConfig , use crate :: runner :: format_duration 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_sync_timer (home : & str) -> Result < () >
  fn is_timer_active () -> bool
  fn resolve_remote_category (folder_name : & str) -> String
  fn run_media_sync () -> Result < () >
  ```

### `src/modules/media/transfer/cache.rs` (Role: general, Lines: 317)
- **Responsibility**: Core general logic in src/modules/media/transfer/cache.rs
- **Imports**: use anyhow :: Result , use serde :: { Deserialize , Serialize } , use std :: collections :: HashMap , use std :: fs , use std :: path :: { Path , PathBuf } , use std :: time :: UNIX_EPOCH , use super :: scan :: collect_dir_files , use super :: { MediaCategory , MediaFile , MediaSeason , SyncStatus } 
- **Types & Enums**:
  ```rust
  pub struct CachedSeason
  pub struct CachedItem
  pub struct MediaScanCache
  ```
- **Public Functions & Signatures**:
  ```rust
  fn path_mtime_secs (path : & Path) -> u64
  fn cache_file_path (home : & Path) -> PathBuf
  fn load_cache (home : & Path) -> MediaScanCache
  fn save (& self , home : & Path) -> Result < () >
  fn invalidate (home : & Path) -> Result < () >
  fn get_or_scan_movie (& mut self , path : & Path , title : & str , cat : MediaCategory , is_local : bool ,) -> (Vec < MediaFile > , u64)
  fn get_or_scan_seasons (& mut self , dir_path : & Path , remote_base : & str , is_local : bool , cat : MediaCategory , show_title : & str ,) -> Vec < MediaSeason >
  ```

### `src/modules/media/transfer/execute.rs` (Role: general, Lines: 255)
- **Responsibility**: Core general logic in src/modules/media/transfer/execute.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: io :: { self , Write } , use std :: path :: Path , use std :: process :: Command , use std :: time :: Duration , use super :: { notify , MediaItem , TransferDirection } , use crate :: modules :: media :: disk , use crate :: runner :: format_duration 
- **Public Functions & Signatures**:
  ```rust
  fn confirm_transfer (dir : TransferDirection , items : & [MediaItem]) -> bool
  fn execute_transfer (dir : TransferDirection , items : & [MediaItem] , home : & Path) -> Result < () >
  ```

### `src/modules/media/transfer/notify.rs` (Role: general, Lines: 204)
- **Responsibility**: Core general logic in src/modules/media/transfer/notify.rs
- **Imports**: use std :: time :: Duration , use super :: { MediaCategory , MediaItem , TransferDirection } , use crate :: modules :: media :: disk , use crate :: notify :: client :: { escape_html , send_alert } , use crate :: notify :: TelegramConfig , use crate :: runner :: format_duration 
- **Public Functions & Signatures**:
  ```rust
  fn format_transfer_rate (bytes : u64 , duration : Duration) -> String
  fn send_item_notification (dir : TransferDirection , item : & MediaItem , index : usize , total_count : usize , duration : Duration ,)
  fn send_batch_initiated_notification (dir : TransferDirection , items : & [MediaItem] , total_bytes : u64 ,)
  fn send_batch_completed_notification (dir : TransferDirection , timings : & [(& MediaItem , Duration)] , total_bytes : u64 , total_duration : Duration ,)
  ```

### `src/modules/media/transfer/scan.rs` (Role: general, Lines: 397)
- **Responsibility**: Core general logic in src/modules/media/transfer/scan.rs
- **Imports**: use anyhow :: Result , use std :: collections :: HashSet , use std :: fs , use std :: path :: Path , use std :: process :: Command , use super :: cache :: MediaScanCache , use super :: { MediaCategory , MediaFile , MediaItem , SyncStatus } 
- **Public Functions & Signatures**:
  ```rust
  fn collect_dir_files (path : & Path , rel_prefix : & str) -> Vec < MediaFile >
  fn scan_libraries (home : & Path) -> Result < (Vec < MediaItem > , Vec < MediaItem >) >
  fn rescan_libraries (home : & Path) -> Result < (Vec < MediaItem > , Vec < MediaItem >) >
  fn scan_local_media (home : & Path , cache : & mut MediaScanCache) -> Vec < MediaItem >
  fn scan_remote_media (home : & Path , cache : & mut MediaScanCache) -> Result < Vec < MediaItem > >
  ```

### `src/modules/media/transfer/sync_status.rs` (Role: general, Lines: 347)
- **Responsibility**: Core general logic in src/modules/media/transfer/sync_status.rs
- **Imports**: use serde :: { Deserialize , Serialize } , use std :: collections :: HashMap , use super :: { MediaCategory , MediaItem , MediaSeason } 
- **Types & Enums**:
  ```rust
  pub struct MediaFile
  pub struct SyncStatus
  ```
- **Public Functions & Signatures**:
  ```rust
  fn is_all_in_other (& self) -> bool
  fn is_partial (& self) -> bool
  fn from_files (files : & [MediaFile]) -> Self
  fn combine (statuses : impl IntoIterator < Item = SyncStatus >) -> Self
  fn cross_reference_libraries (local : & mut [MediaItem] , remote : & mut [MediaItem])
  ```

### `src/modules/media/transfer.rs` (Role: general, Lines: 160)
- **Responsibility**: Core general logic in src/modules/media/transfer.rs
- **Imports**: pub use execute :: { confirm_transfer , execute_transfer } , pub use scan :: { rescan_libraries , scan_libraries } , pub use sync_status :: { cross_reference_libraries , MediaFile , SyncStatus } , use serde :: { Deserialize , Serialize } , use std :: path :: PathBuf 
- **Types & Enums**:
  ```rust
  pub enum TransferDirection
  pub enum MediaCategory
  pub struct MediaSeason
  pub struct MediaItem
  ```
- **Public Functions & Signatures**:
  ```rust
  fn as_str (self) -> & 'static str
  fn has_seasons (& self) -> bool
  fn selected_seasons_count (& self) -> usize
  fn selected_bytes (& self , is_item_selected : bool) -> u64
  fn selected_needed_bytes (& self , is_item_selected : bool) -> u64
  fn are_all_seasons_selected (& self) -> bool
  fn are_some_seasons_selected (& self) -> bool
  ```

### `src/modules/media.rs` (Role: general, Lines: 125)
- **Responsibility**: Core general logic in src/modules/media.rs
- **Imports**: use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: io :: IsTerminal , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub enum MediaType
  pub enum ClassificationEngine
  pub struct MediaInfo
  pub struct OrganizeResult
  ```
- **Public Functions & Signatures**:
  ```rust
  fn handle_media_cli (action : Option < & str >) -> anyhow :: Result < () >
  ```

### `src/modules/prompt.rs` (Role: general, Lines: 38)
- **Responsibility**: Core general logic in src/modules/prompt.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use std :: fs , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/rclone/install.rs` (Role: general, Lines: 50)
- **Responsibility**: Core general logic in src/modules/rclone/install.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: fs , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn install_rclone_binary (runner : & mut Runner) -> Result < () >
  fn install_fuse_support (runner : & mut Runner) -> Result < () >
  fn configure_fuse_allow_other (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/rclone/oauth.rs` (Role: general, Lines: 143)
- **Responsibility**: Core general logic in src/modules/rclone/oauth.rs
- **Imports**: use anyhow :: Result , use colored :: Colorize , use std :: io :: { self , BufRead , Write } , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn is_remote_configured () -> bool
  fn ensure_remote_initialized ()
  fn prompt_and_configure_remote () -> Result < bool >
  ```

### `src/modules/rclone/service.rs` (Role: general, Lines: 134)
- **Responsibility**: Core general logic in src/modules/rclone/service.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { Context , Result } , use colored :: Colorize , use std :: fs , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn deploy_systemd_service (runner : & Runner , home : & str) -> Result < () >
  fn enable_and_start_service (runner : & mut Runner) -> Result < () >
  fn is_service_active () -> bool
  fn is_path_mounted (path : & Path) -> bool
  fn print_quota_stats ()
  fn start_mount () -> Result < () >
  fn stop_mount () -> Result < () >
  ```

### `src/modules/rclone.rs` (Role: general, Lines: 180)
- **Responsibility**: Core general logic in src/modules/rclone.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { Context , Result } , use clap :: Subcommand , use colored :: Colorize , use std :: fs , use std :: path :: { Path , PathBuf } , pub use oauth :: is_remote_configured 
- **Types & Enums**:
  ```rust
  pub enum RcloneSubcommand
  ```
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  fn handle_cli (sub : RcloneSubcommand) -> Result < () >
  ```

### `src/modules/security.rs` (Role: general, Lines: 54)
- **Responsibility**: Core general logic in src/modules/security.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules/seedr.rs` (Role: general, Lines: 42)
- **Responsibility**: Core general logic in src/modules/seedr.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/tailscale.rs` (Role: general, Lines: 148)
- **Responsibility**: Core general logic in src/modules/tailscale.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: io :: { self , BufRead , Write } 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/timezone.rs` (Role: general, Lines: 314)
- **Responsibility**: Core general logic in src/modules/timezone.rs
- **Imports**: use crate :: notify :: TelegramConfig , use crate :: runner :: Runner , use anyhow :: { bail , Context , Result } , use clap :: { Args , ValueEnum } , use colored :: Colorize , use serde :: Deserialize , use std :: fs , use std :: io :: { self , BufRead , Write } , use std :: path :: Path , use std :: process :: Command , use std :: time :: Duration 
- **Types & Enums**:
  ```rust
  pub enum IpProvider
  pub struct TimezoneArgs
  ```
- **Public Functions & Signatures**:
  ```rust
  const fn name (self) -> & 'static str
  const fn endpoint (self) -> & 'static str
  fn detect_timezone (provider : Option < IpProvider >) -> Result < (String , & 'static str) >
  fn get_current_timezone () -> String
  fn is_valid_timezone (tz : & str) -> bool
  fn set_system_timezone (tz : & str , runner : & mut Runner) -> Result < () >
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  fn handle_cli (args : TimezoneArgs , runner : & mut Runner) -> Result < () >
  ```

### `src/modules/torrent/api.rs` (Role: api, Lines: 286)
- **Responsibility**: Core api logic in src/modules/torrent/api.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use reqwest :: blocking :: Client , use serde :: Deserialize 
- **Types & Enums**:
  ```rust
  pub struct TorrentInfo
  ```
- **Public Functions & Signatures**:
  ```rust
  fn is_completed (& self) -> bool
  fn get_torrents (client : & Client , base_url : & str , hash : Option < & str > ,) -> Result < Vec < TorrentInfo > >
  fn add_magnet (client : & Client , base_url : & str , magnet : & str) -> Result < () >
  fn add_torrent_file (client : & Client , base_url : & str , filename : & str , bytes : Vec < u8 > ,) -> Result < () >
  fn pause_all (client : & Client , base_url : & str) -> Result < () >
  fn resume_all (client : & Client , base_url : & str) -> Result < () >
  fn delete_torrent (client : & Client , base_url : & str , hash : & str , delete_files : bool ,) -> Result < () >
  fn get_torrent_size (hash : & str , base_url : & str) -> Option < u64 >
  ```

### `src/modules/torrent/cli.rs` (Role: cli, Lines: 74)
- **Responsibility**: Core cli logic in src/modules/torrent/cli.rs
- **Imports**: use crate :: modules :: torrent :: { api , dedup , queue , scheduler , seedr } , use crate :: notify :: TelegramConfig , use anyhow :: Result , use colored :: Colorize , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn handle_seedr_cli (target : & str) -> Result < () >
  ```

### `src/modules/torrent/dedup.rs` (Role: general, Lines: 352)
- **Responsibility**: Core general logic in src/modules/torrent/dedup.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub enum Availability
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_magnet (target : & str) -> (Option < String > , Option < String >)
  fn check_already_available (magnet_or_url : & str) -> Availability
  fn find_in_google_drive (local_path : & Path) -> Option < PathBuf >
  fn extract_rel_media_subpath (path : & Path) -> Option < PathBuf >
  fn restore_from_cloud (pairs : & [(PathBuf , PathBuf)]) -> Result < Vec < PathBuf > >
  ```

### `src/modules/torrent/history.rs` (Role: general, Lines: 153)
- **Responsibility**: Core general logic in src/modules/torrent/history.rs
- **Imports**: use anyhow :: { Context , Result } , use serde :: { Deserialize , Serialize } , use std :: collections :: HashMap , use std :: fs , use std :: hash :: { Hash , Hasher } , use std :: io :: { Read , Seek , SeekFrom } , use std :: path :: { Path , PathBuf } , use crate :: modules :: media :: MediaInfo 
- **Types & Enums**:
  ```rust
  pub struct TrackedFile
  pub struct HistoryRecord
  pub struct DownloadHistory
  ```
- **Public Functions & Signatures**:
  ```rust
  fn load_history () -> DownloadHistory
  fn compute_file_signature (path : & Path) -> Result < String >
  fn create_tracked_file (path : PathBuf , variant : & str) -> TrackedFile
  fn record_download_history (hash : Option < & str > , info : & MediaInfo , files : Vec < TrackedFile > ,) -> Result < () >
  ```

### `src/modules/torrent/notify.rs` (Role: general, Lines: 222)
- **Responsibility**: Core general logic in src/modules/torrent/notify.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: thread :: sleep , use std :: time :: Duration , use super :: api :: { self , TorrentInfo } , use super :: telegram :: TelegramConfig , use crate :: modules :: media :: OrganizeResult , use crate :: notify :: client :: escape_html 
- **Public Functions & Signatures**:
  ```rust
  fn execute (event : & str , hash : & str) -> Result < () >
  fn format_size (bytes : u64) -> String
  ```

### `src/modules/torrent/queue/tests.rs` (Role: general, Lines: 247)
- **Responsibility**: Core general logic in src/modules/torrent/queue/tests.rs
- **Imports**: use super :: * , use std :: sync :: Arc , use std :: thread 

### `src/modules/torrent/queue.rs` (Role: general, Lines: 295)
- **Responsibility**: Core general logic in src/modules/torrent/queue.rs
- **Imports**: use anyhow :: { Context , Result } , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: os :: unix :: io :: AsRawFd , use std :: path :: { Path , PathBuf } , use std :: sync :: Mutex 
- **Types & Enums**:
  ```rust
  pub enum QueueState
  pub enum QueuePolicy
  pub struct QueueEntry
  pub struct SeedrQueue
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse (raw : & str) -> Self
  fn now_secs () -> u64
  fn enqueue (& mut self , hash : & str , magnet : & str , name : & str)
  fn has_active (& self) -> bool
  fn activate (& mut self , hash : & str) -> bool
  fn active_entry (& self) -> Option < & QueueEntry >
  fn demote_to_queued (& mut self , hash : & str) -> bool
  fn set_slow_since (& mut self , hash : & str , secs : u64) -> bool
  fn clear_slow_since (& mut self , hash : & str) -> bool
  fn promote_front (& mut self , hash : & str) -> bool
  fn finish (& mut self , hash : & str , state : QueueState) -> bool
  fn remove (& mut self , hash : & str) -> bool
  fn position (& self , hash : & str) -> usize
  fn pending_count (& self) -> usize
  fn with_lock < T > (f : impl FnOnce () -> T) -> T
  fn load () -> SeedrQueue
  fn save (queue : & SeedrQueue) -> Result < () >
  ```

### `src/modules/torrent/report.rs` (Role: general, Lines: 121)
- **Responsibility**: Core general logic in src/modules/torrent/report.rs
- **Imports**: use super :: api :: TorrentInfo , use super :: notify :: format_size 
- **Public Functions & Signatures**:
  ```rust
  fn format_status_report (torrents : & [TorrentInfo]) -> String
  ```

### `src/modules/torrent/scheduler.rs` (Role: general, Lines: 372)
- **Responsibility**: Core general logic in src/modules/torrent/scheduler.rs
- **Imports**: use std :: path :: { Path , PathBuf } , use super :: api , use super :: queue :: { self , QueueEntry , QueuePolicy , QueueState } , use super :: seedr , use crate :: notify :: config :: TelegramConfig 
- **Types & Enums**:
  ```rust
  pub enum SubmitOutcome
  pub enum Promotion
  ```
- **Public Functions & Signatures**:
  ```rust
  fn pick_promotion (entries : & [QueueEntry] , policy : QueuePolicy , free_bytes : Option < u64 > , needed_bytes : Option < u64 > ,) -> Promotion
  fn submit (hash : & str , magnet : & str , name : & str , config : & TelegramConfig) -> SubmitOutcome
  fn handle_qb_completion (hash : & str , config : & TelegramConfig)
  fn promote_front (hash : & str) -> bool
  fn handle_seedr_done (hash : & str , config : & TelegramConfig)
  fn handle_seedr_failure (hash : & str , config : & TelegramConfig)
  ```

### `src/modules/torrent/seedr.rs` (Role: general, Lines: 326)
- **Responsibility**: Core general logic in src/modules/torrent/seedr.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: path :: { Path , PathBuf } , use std :: process :: { Command , Stdio } , use std :: time :: Duration , use super :: api , use super :: telegram :: TelegramConfig , pub use super :: seedr_tasks :: { format_seedr_tasks_section , get_active_seedr_tasks } 
- **Public Functions & Signatures**:
  ```rust
  fn extract_btih_hash (magnet : & str) -> Option < String >
  fn save_pending_magnet (hash : & str , magnet : & str)
  fn load_pending_magnet (hash : & str) -> Option < String >
  fn remove_pending_magnet (hash : & str)
  fn spawn_seedr_download (magnet : & str , api_port : u16) -> Result < () >
  fn handle_seedr_completion (hash : Option < & str > , file_name : & str , total_bytes : u64 , dest_path : Option < & str > , config : & TelegramConfig ,) -> Result < () >
  fn handle_seedr_failure (hash : Option < & str > , error : & str , config : & TelegramConfig ,) -> Result < () >
  ```

### `src/modules/torrent/seedr_health.rs` (Role: general, Lines: 98)
- **Responsibility**: Core general logic in src/modules/torrent/seedr_health.rs
- **Imports**: use anyhow :: Context , use reqwest :: blocking :: Client , use std :: time :: Duration , use super :: api , use super :: queue :: { self , QueueState } , use super :: scheduler , use super :: seedr :: { load_pending_magnet , remove_pending_magnet } , use super :: seedr_tasks , use crate :: notify :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn record_active_health (config : & TelegramConfig)
  ```

### `src/modules/torrent/seedr_tasks.rs` (Role: general, Lines: 383)
- **Responsibility**: Core general logic in src/modules/torrent/seedr_tasks.rs
- **Imports**: use serde :: Deserialize , use std :: collections :: HashSet , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct SeedrTaskState
  ```
- **Public Functions & Signatures**:
  ```rust
  fn seedr_available_bytes () -> Option < u64 >
  fn get_active_seedr_tasks () -> Vec < SeedrTaskState >
  fn format_seedr_tasks_section (tasks : & [SeedrTaskState]) -> String
  ```

### `src/modules/torrent/telegram.rs` (Role: general, Lines: 140)
- **Responsibility**: Core general logic in src/modules/torrent/telegram.rs
- **Imports**: use anyhow :: Result , use std :: fs , use std :: io :: { self , Write } , use std :: path :: Path , use std :: process :: Command , pub use crate :: notify :: TelegramConfig , use crate :: runner :: Runner 
- **Public Functions & Signatures**:
  ```rust
  fn prompt_telegram_config (_runner : & mut Runner , non_interactive : bool ,) -> Result < Option < TelegramConfig > >
  fn install_bot_service (home : & str) -> Result < () >
  fn configure_autorun (lines : & mut Vec < String > , enabled : bool)
  ```

### `src/modules/torrent.rs` (Role: general, Lines: 394)
- **Responsibility**: Core general logic in src/modules/torrent.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: { bail , Context , Result } , use colored :: Colorize , use std :: fs , use std :: io :: { self , BufRead , Write } , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner , non_interactive : bool) -> Result < () >
  ```

### `src/modules/trash.rs` (Role: general, Lines: 16)
- **Responsibility**: Core general logic in src/modules/trash.rs
- **Imports**: use crate :: runner :: Runner , use anyhow :: Result 
- **Public Functions & Signatures**:
  ```rust
  fn setup (runner : & mut Runner) -> Result < () >
  ```

### `src/modules.rs` (Role: general, Lines: 364)
- **Responsibility**: Core general logic in src/modules.rs
- **Imports**: use crate :: configs , use crate :: runner :: Runner , use anyhow :: Result , use colored :: Colorize , use std :: collections :: HashSet 
- **Types & Enums**:
  ```rust
  pub struct Module
  ```
- **Public Functions & Signatures**:
  ```rust
  fn requires_sudo (modules : & [String]) -> bool
  fn get_available_modules () -> Vec < Module >
  fn resolve_dependencies (selected_ids : & [String]) -> Vec < String >
  fn execute_module (module_id : & str , runner : & mut Runner , non_interactive : bool) -> Result < () >
  fn run_system_check ()
  ```

### `src/notify/client.rs` (Role: cli, Lines: 60)
- **Responsibility**: Core cli logic in src/notify/client.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: fmt :: Write as _ , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn escape_html (input : & str) -> String
  fn send_telegram_alert (client : & Client , token : & str , chat_id : & str , text : & str) -> Result < () >
  fn send_alert (token : & str , chat_id : & str , text : & str) -> Result < () >
  fn format_card (category : & str , badge : & str , fields : & [(& str , & str)]) -> String
  ```

### `src/notify/config.rs` (Role: general, Lines: 169)
- **Responsibility**: Core general logic in src/notify/config.rs
- **Imports**: use anyhow :: Result , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct TelegramConfig
  ```
- **Public Functions & Signatures**:
  ```rust
  fn primary_config_path () -> PathBuf
  fn legacy_config_path () -> PathBuf
  fn candidate_paths () -> Vec < PathBuf >
  fn load () -> Result < Self >
  fn save (& self) -> Result < () >
  fn save_to (& self , config_dir : & Path) -> Result < () >
  ```

### `src/notify/hooks.rs` (Role: general, Lines: 328)
- **Responsibility**: Core general logic in src/notify/hooks.rs
- **Imports**: use std :: fmt :: Write as _ , use std :: fs , use std :: path :: Path , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn install_hooks (bin_path : & Path)
  ```

### `src/notify/power.rs` (Role: general, Lines: 389)
- **Responsibility**: Core general logic in src/notify/power.rs
- **Imports**: use anyhow :: { bail , Result } , use std :: fs , use std :: io :: Write , use std :: path :: Path , use std :: thread , use std :: time :: Duration , use super :: client :: { format_card , send_alert } , use super :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn send_power_event (config : & TelegramConfig , status : & str) -> Result < () >
  fn run_battery_watch (config : & TelegramConfig) -> Result < () >
  fn is_ac_online () -> bool
  fn read_battery_percent () -> Option < u8 >
  fn read_battery_status () -> Option < String >
  fn read_cpu_temp () -> Option < f64 >
  fn read_cpu_usage () -> Option < f64 >
  ```

### `src/notify/server.rs` (Role: general, Lines: 237)
- **Responsibility**: Core general logic in src/notify/server.rs
- **Imports**: use anyhow :: { Context , Result } , use serde :: Deserialize , use std :: io :: { BufRead , BufReader , Read , Write } , use std :: net :: { TcpListener , TcpStream } , use std :: sync :: Arc , use std :: time :: Duration , use super :: config :: TelegramConfig , use super :: system :: send_custom_notification 
- **Public Functions & Signatures**:
  ```rust
  fn spawn_background_server (config : TelegramConfig , port : u16)
  fn run_server (config : TelegramConfig , port : u16) -> Result < () >
  ```

### `src/notify/session.rs` (Role: general, Lines: 85)
- **Responsibility**: Core general logic in src/notify/session.rs
- **Imports**: use std :: fs , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn is_session_suppressed (ip : & str , cooldown_mins : u64) -> bool
  fn record_session_login (ip : & str)
  ```

### `src/notify/shutdown.rs` (Role: general, Lines: 78)
- **Responsibility**: Core general logic in src/notify/shutdown.rs
- **Imports**: use std :: process :: Command , use std :: time :: Duration , use anyhow :: Result , use super :: client :: { format_card , send_alert } , use super :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn send_shutdown_notification (config : & TelegramConfig) -> Result < () >
  ```

### `src/notify/system.rs` (Role: general, Lines: 364)
- **Responsibility**: Core general logic in src/notify/system.rs
- **Imports**: use anyhow :: Result , use std :: fs , use std :: mem :: MaybeUninit , use std :: process :: Command , use std :: time :: Duration , use super :: client :: { escape_html , format_card , send_alert } , use super :: config :: TelegramConfig 
- **Public Functions & Signatures**:
  ```rust
  fn send_boot_notification (config : & TelegramConfig) -> Result < () >
  fn send_login_notification (config : & TelegramConfig , user : Option < & str > , ip : Option < & str > , service : Option < & str > , tty : Option < & str > ,) -> Result < () >
  fn send_custom_notification (config : & TelegramConfig , message : & str , title : Option < & str > , level : & str ,) -> Result < () >
  fn get_hostname (config : & TelegramConfig) -> String
  fn get_public_ip () -> String
  fn get_tailscale_ip () -> String
  fn get_kernel () -> String
  fn get_uptime () -> String
  fn get_memory_stats () -> Option < (u64 , u64) >
  fn get_disk_stats () -> Option < (u64 , u64) >
  fn format_usage (used : u64 , total : u64) -> String
  fn send_audio_strip_notification (config : & TelegramConfig , args : & crate :: notify :: AudioStripArgs ,) -> Result < () >
  ```

### `src/notify.rs` (Role: general, Lines: 186)
- **Responsibility**: Core general logic in src/notify.rs
- **Imports**: use anyhow :: Result , use clap :: Subcommand , use colored :: Colorize , pub use config :: TelegramConfig 
- **Types & Enums**:
  ```rust
  pub enum NotifySubcommand
  pub struct AudioStripArgs
  ```
- **Public Functions & Signatures**:
  ```rust
  fn handle_cli (cmd : NotifySubcommand) -> Result < () >
  ```

### `src/runner.rs` (Role: general, Lines: 262)
- **Responsibility**: Core general logic in src/runner.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use colored :: Colorize , use indicatif :: { ProgressBar , ProgressStyle } , use std :: fs :: { self , File , OpenOptions } , use std :: io :: { Read , Write } , use std :: path :: { Path , PathBuf } , use std :: process :: { Command , Stdio } , use std :: time :: Duration 
- **Types & Enums**:
  ```rust
  pub struct Runner
  ```
- **Public Functions & Signatures**:
  ```rust
  fn format_duration (d : Duration) -> String
  fn new (dry_run : bool , verbose : bool) -> Result < Self >
  fn command_exists (cmd : & str) -> bool
  fn ensure_sudo (& self) -> Result < () >
  fn create_spinner (message : & str) -> ProgressBar
  fn exec_silent (& mut self , desc : & str , program : & str , args : & [& str]) -> Result < () >
  fn exec_bash (& mut self , desc : & str , script : & str) -> Result < () >
  fn apt_install (& mut self , desc : & str , packages : & [& str]) -> Result < () >
  fn apt_update (& mut self) -> Result < () >
  ```

### `src/state.rs` (Role: general, Lines: 133)
- **Responsibility**: Core general logic in src/state.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: io :: { self , BufRead , Write } , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct RunState
  ```
- **Public Functions & Signatures**:
  ```rust
  fn load () -> Option < Self >
  fn save (& self) -> Result < () >
  fn clear ()
  fn mark_done (& mut self , module_id : & str) -> Result < () >
  fn resolve_resume (module_ids : Vec < String > , non_interactive : bool ,) -> Result < (Vec < String > , RunState) >
  ```

### `src/summary.rs` (Role: general, Lines: 100)
- **Responsibility**: Core general logic in src/summary.rs
- **Imports**: use crate :: runner , use colored :: Colorize 
- **Public Functions & Signatures**:
  ```rust
  fn print_summary (module_ids : & [String] , total_duration : std :: time :: Duration , timings : & [(String , std :: time :: Duration)] ,)
  ```

### `src/tui.rs` (Role: tui, Lines: 262)
- **Responsibility**: Core tui logic in src/tui.rs
- **Imports**: use anyhow :: Result , use crossterm :: { event :: { self , Event , KeyCode , KeyEventKind } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , layout :: { Alignment , Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , BorderType , Borders , Paragraph } , Frame , Terminal , } , use std :: io , use crate :: modules :: { get_available_modules , Module } 
- **Public Functions & Signatures**:
  ```rust
  fn select_modules () -> Result < Option < Vec < String > > >
  ```

### `src/updater.rs` (Role: general, Lines: 119)
- **Responsibility**: Core general logic in src/updater.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use reqwest :: blocking :: Client , use serde :: Deserialize , use std :: fs , use std :: os :: unix :: fs :: PermissionsExt , use std :: path :: PathBuf 
- **Public Functions & Signatures**:
  ```rust
  fn run_self_update (current_version : & str) -> Result < () >
  ```

## 4. Execution Lifecycle Trace
1. **Startup**: Entrypoint parses CLI flags & dispatches command.
2. **Execution**: Core domain logic processes inputs and evaluates rules.
3. **Persistence / I/O**: Domain logic calls infra for disk/terminal I/O.
4. **Exit**: Graceful termination with standard exit codes.

## 5. Verification Commands
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo test --all-targets
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```
