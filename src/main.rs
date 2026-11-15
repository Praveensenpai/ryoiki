mod bot;
mod charge_limit;
mod configs;
mod modules;
mod notify;
mod runner;
mod settings;
mod state;
mod summary;
mod tui;
mod updater;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use modules::{execute_module, get_available_modules};
use runner::Runner;
use std::io::IsTerminal;

// reason: CLI flag structure defined by clap command line interface
#[allow(clippy::struct_excessive_bools)]
#[derive(Parser)]
#[command(name = "ryoiki")]
#[command(author = "Praveensenpai <pvnt20@gmail.com>")]
#[command(version)]
#[command(about = "Aesthetic, zero-clutter Ubuntu server provisioning orchestrator", long_about = None)]
struct Cli {
    /// Install all modules without interactive prompt
    #[arg(short = 'a', long = "all", global = true)]
    all: bool,

    /// Non-interactive mode (use defaults for all prompts)
    #[arg(short = 'y', long = "yes", global = true)]
    yes: bool,

    /// Simulate execution without running commands
    #[arg(long = "dry-run", global = true)]
    dry_run: bool,

    /// Stream verbose command outputs directly to terminal
    #[arg(short = 'v', long = "verbose", global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open the control center provisioning checklist
    Install,
    /// Open the control center dashboard (same as bare ryoiki)
    Tui,
    /// Deploy only embedded dotfiles (`~/.tmux.conf`, `~/.bash_aliases`, `starship.toml`)
    Dotfiles,
    /// Inspect current system status (check installed tools)
    Check,
    /// Run specific modules by ID
    Run {
        #[arg(required = true)]
        modules: Vec<String>,
    },
    /// Update ryoiki to the latest release from GitHub
    Update,
    /// Run the interactive 2-way Telegram bot daemon
    Bot,
    /// Set battery max charge limit (60% default — extends lifespan on always-plugged servers)
    ChargeLimit,
    /// Send automated or custom Telegram notifications
    #[command(subcommand)]
    Notify(notify::NotifySubcommand),
    /// Classify and organize media files into Jellyfin
    Organize {
        /// Path to media file or folder to organize (default: ~/torrents)
        #[arg(default_value = "")]
        path: String,

        /// Only preview changes without moving files
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage zero-hassle Rclone Google Drive cloud mount
    #[command(subcommand)]
    Rclone(modules::rclone::RcloneSubcommand),
    /// Manage automated Jellyfin cloud backups to Google Drive
    #[command(subcommand)]
    Backup(modules::jellyfin::backup::BackupSubcommand),
    /// Generate optimal Kodi streaming settings and troubleshooting guide
    #[command(subcommand)]
    Kodi(modules::jellyfin::kodi::KodiSubcommand),
    /// Prune watched/large media to Google Drive when SSD is full
    Prune {
        /// High-watermark trigger (default: settings, 90)
        #[arg(long)]
        threshold: Option<u8>,

        /// Target percentage to recover down to (default: settings, 75)
        #[arg(long)]
        target: Option<u8>,

        /// Preview files that would be archived without deleting
        #[arg(long)]
        dry_run: bool,

        /// Enable and activate the hourly automated prune timer
        #[arg(long)]
        schedule: bool,
    },
    /// Inspect local SSD and Google Drive storage overview
    Storage,
    /// Interactive media manager (push to / pull from Google Drive with checklist)
    Media {
        /// Mode to preselect: "push", "pull", "sync", or "schedule"
        #[arg(value_name = "ACTION")]
        action: Option<String>,

        /// Enable and activate the daily post-boot media copy timer
        #[arg(long)]
        schedule: bool,
    },
    /// Inspect or strip redundant dubbed audio tracks via dubstrip engine
    #[command(subcommand)]
    Audio(modules::media::audio::AudioSubcommand),
    /// Configure system timezone (auto-detect via IP provider or set custom)
    Timezone(modules::timezone::TimezoneArgs),
    /// Queue magnet to dual-pipeline (Seedr.cc cloud + qBittorrent) or inspect status
    Seedr {
        /// Magnet link, cloud folder ID, or "status"
        #[arg(default_value = "status")]
        target: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();

    match cli.command.as_ref() {
        None => {
            if interactive && !cli.all && !cli.yes {
                return tui::run(tui::RunOptions {
                    dry_run: cli.dry_run,
                    verbose: cli.verbose,
                    start: None,
                });
            }
            if !cli.all && !cli.yes {
                tui::print_plain();
                return Ok(());
            }
            let mut runner = Runner::new(cli.dry_run, cli.verbose)?;
            print_banner();
            return run_all_modules(&mut runner, cli.all, cli.yes);
        }
        Some(Commands::Install) => {
            if interactive && !cli.all && !cli.yes {
                return tui::run(tui::RunOptions {
                    dry_run: cli.dry_run,
                    verbose: cli.verbose,
                    start: Some(tui::Tab::Provision),
                });
            }
            let mut runner = Runner::new(cli.dry_run, cli.verbose)?;
            print_banner();
            return run_all_modules(&mut runner, cli.all, cli.yes);
        }
        Some(Commands::Tui) => {
            if interactive && !cli.all && !cli.yes {
                return tui::run(tui::RunOptions {
                    dry_run: cli.dry_run,
                    verbose: cli.verbose,
                    start: None,
                });
            }
            tui::print_plain();
            return Ok(());
        }
        _ => {}
    }

    let mut runner = Runner::new(cli.dry_run, cli.verbose)?;
    if !matches!(cli.command.as_ref(), Some(Commands::Notify(..))) {
        print_banner();
    }

    if let Some(cmd) = cli.command {
        return handle_subcommand(cmd, &mut runner, cli.yes);
    }
    Ok(())
}

fn print_dependency_notes(chosen: &[String], resolved: &[String]) {
    if resolved.len() > chosen.len() {
        let added: Vec<&str> = resolved
            .iter()
            .filter(|id| !chosen.contains(id))
            .map(String::as_str)
            .collect();
        println!(
            "  {} Auto-included dependencies: {}\n",
            "ℹ".cyan().bold(),
            added.join(", ").bold()
        );
    }
}

fn handle_subcommand(cmd: Commands, runner: &mut Runner, yes: bool) -> Result<()> {
    match cmd {
        Commands::Install | Commands::Tui => tui::print_plain(),
        Commands::Dotfiles => deploy_dotfiles()?,
        Commands::Check => modules::run_system_check(),
        Commands::Run { modules } => run_selected(&modules, runner, yes)?,
        Commands::Update => updater::run_self_update(env!("CARGO_PKG_VERSION"))?,
        Commands::Bot => bot::run_bot()?,
        Commands::ChargeLimit => charge_limit::run(yes)?,
        Commands::Notify(sub) => notify::handle_cli(sub)?,
        Commands::Organize { path, dry_run } => organize(&path, dry_run)?,
        Commands::Rclone(sub) => modules::rclone::handle_cli(sub)?,
        Commands::Backup(sub) => modules::jellyfin::backup::handle_cli(sub)?,
        Commands::Kodi(sub) => modules::jellyfin::kodi::handle_cli(&sub)?,
        Commands::Prune {
            threshold,
            target,
            dry_run,
            schedule,
        } => handle_prune(threshold, target, dry_run, schedule)?,
        Commands::Storage => modules::media::status::show_storage_status(),
        Commands::Media { action, schedule } => handle_media(action.as_deref(), schedule)?,
        Commands::Audio(sub) => modules::media::audio::handle_cli(sub)?,
        Commands::Timezone(args) => handle_timezone(args, runner)?,
        Commands::Seedr { target } => modules::torrent::cli::handle_seedr_cli(&target)?,
    }
    Ok(())
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
}

fn deploy_dotfiles() -> Result<()> {
    let home = home_dir();
    let start = std::time::Instant::now();
    configs::deploy_dotfiles(&home)?;
    let dur = runner::format_duration(start.elapsed());
    println!(
        "  {} Dotfiles deployed successfully to {} ({dur})",
        "✔".green().bold(),
        home.cyan()
    );
    Ok(())
}

fn run_selected(selected: &[String], runner: &mut Runner, yes: bool) -> Result<()> {
    let resolved = modules::resolve_dependencies(selected);
    if modules::requires_sudo(&resolved) {
        runner.ensure_sudo()?;
    }
    let (total_dur, timings) = run_modules(&resolved, runner, yes, &mut None)?;
    summary::print_summary(&resolved, total_dur, &timings);
    Ok(())
}

fn organize(path: &str, dry_run: bool) -> Result<()> {
    let target = if path.is_empty() {
        std::path::PathBuf::from(home_dir()).join("torrents")
    } else {
        std::path::PathBuf::from(path)
    };
    modules::media::organizer::run_organize_cli(&target, dry_run)
}

fn handle_prune(
    threshold: Option<u8>,
    target: Option<u8>,
    dry_run: bool,
    schedule: bool,
) -> Result<()> {
    let storage = settings::load_or_default().storage;
    let threshold = threshold.unwrap_or(storage.prune_threshold);
    let target = target.unwrap_or_else(|| {
        storage
            .prune_target
            .min(threshold.saturating_sub(settings::MIN_PRUNE_GAP))
            .max(settings::MIN_PRUNE_TARGET)
    });
    settings::validate_prune(threshold, target)?;

    if schedule {
        modules::media::prune_timer::deploy_prune_timer(&home_dir(), threshold, target)?;
        return Ok(());
    }
    let opts = modules::media::pruner::PruneOptions {
        threshold_pct: threshold,
        target_pct: target,
        dry_run,
    };
    modules::media::pruner::run_prune(opts)
}

fn handle_media(action: Option<&str>, schedule: bool) -> Result<()> {
    if schedule || action == Some("schedule") {
        modules::media::sync::deploy_sync_timer(&home_dir())?;
    } else if action == Some("sync") {
        modules::media::sync::run_media_sync()?;
    } else {
        modules::media::handle_media_cli(action)?;
    }
    Ok(())
}

fn handle_timezone(args: modules::timezone::TimezoneArgs, runner: &mut Runner) -> Result<()> {
    if !args.status {
        runner.ensure_sudo()?;
    }
    modules::timezone::handle_cli(args, runner)
}

fn run_all_modules(runner: &mut Runner, all: bool, yes: bool) -> Result<()> {
    let chosen: Vec<String> = get_available_modules()
        .into_iter()
        .map(|module| module.id.to_string())
        .collect();
    let resolved = modules::resolve_dependencies(&chosen);
    print_dependency_notes(&chosen, &resolved);

    if modules::requires_sudo(&resolved) {
        runner.ensure_sudo()?;
    }

    let non_interactive = all || yes || !std::io::stdin().is_terminal();
    let (to_run, run_state) = state::resolve_resume(resolved.clone(), non_interactive)?;

    if to_run.is_empty() {
        println!(
            "  {} All selected modules are completed.",
            "✔".green().bold()
        );
        state::RunState::clear();
        return Ok(());
    }

    println!(
        "\n  {} Running {} selected modules...\n",
        "▶".cyan().bold(),
        to_run.len()
    );
    let mut state_opt = Some(run_state);
    let (total_dur, timings) = run_modules(&to_run, runner, non_interactive, &mut state_opt)?;
    state::RunState::clear();
    summary::print_summary(&resolved, total_dur, &timings);
    Ok(())
}

fn print_banner() {
    println!(
        "\n  {} {}",
        "領域".cyan().bold(),
        "Ryoiki Server Setup".bold()
    );
    println!("  {}\n", "─".repeat(42).dimmed());
}

fn run_modules(
    module_ids: &[String],
    runner: &mut Runner,
    non_interactive: bool,
    run_state: &mut Option<state::RunState>,
) -> Result<(std::time::Duration, Vec<(String, std::time::Duration)>)> {
    let all_mods = get_available_modules();
    let total = module_ids.len();
    let total_start = std::time::Instant::now();
    let mut timings = Vec::new();

    for (i, id) in module_ids.iter().enumerate() {
        if let Some(meta) = all_mods.iter().find(|m| m.id == id) {
            println!("  [{}/{}] {}", i + 1, total, meta.title.bold());
            let mod_start = std::time::Instant::now();
            execute_module(id, runner, non_interactive)?;
            let mod_dur = mod_start.elapsed();
            let mod_dur_str = runner::format_duration(mod_dur);
            timings.push((meta.title.to_string(), mod_dur));
            if let Some(state) = run_state {
                let _ = state.mark_done(id);
            }
            println!("  {}", format!("── completed in {mod_dur_str} ──").dimmed());
            println!();
        }
    }

    Ok((total_start.elapsed(), timings))
}
