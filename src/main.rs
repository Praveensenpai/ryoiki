mod charge_limit;
mod configs;
mod modules;
mod notify;
mod runner;
mod state;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut runner = Runner::new(cli.dry_run, cli.verbose)?;

    if !matches!(cli.command, Some(Commands::Notify(..))) {
        print_banner();
    }

    if let Some(cmd) = cli.command {
        return handle_subcommand(cmd, &mut runner, cli.yes);
    }

    let Some(chosen) = resolve_selected_modules(&cli)? else {
        return Ok(());
    };

    let resolved = modules::resolve_dependencies(&chosen);
    print_dependency_notes(&chosen, &resolved);

    if modules::requires_sudo(&resolved) {
        runner.ensure_sudo()?;
    }

    let non_interactive = cli.all || cli.yes || !std::io::stdin().is_terminal();
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
    let (total_dur, timings) = run_modules(&to_run, &mut runner, non_interactive, &mut state_opt)?;
    state::RunState::clear();
    print_summary(&resolved, total_dur, &timings);
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
        Commands::Dotfiles => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let start = std::time::Instant::now();
            configs::deploy_dotfiles(&home)?;
            let dur = runner::format_duration(start.elapsed());
            println!(
                "  {} Dotfiles deployed successfully to {} ({dur})",
                "✔".green().bold(),
                home.cyan()
            );
        }
        Commands::Check => run_system_check(),
        Commands::Run { modules } => {
            let resolved = modules::resolve_dependencies(&modules);
            if modules::requires_sudo(&resolved) {
                runner.ensure_sudo()?;
            }
            let (total_dur, timings) = run_modules(&resolved, runner, yes, &mut None)?;
            print_summary(&resolved, total_dur, &timings);
        }
        Commands::Update => {
            updater::run_self_update(env!("CARGO_PKG_VERSION"))?;
        }
        Commands::Bot => {
            modules::torrent::bot::run_bot()?;
        }
        Commands::ChargeLimit => {
            charge_limit::run(yes)?;
        }
        Commands::Notify(sub) => {
            notify::handle_cli(sub)?;
        }
        Commands::Organize { path, dry_run } => {
            let target = if path.is_empty() {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                std::path::PathBuf::from(home).join("torrents")
            } else {
                std::path::PathBuf::from(path)
            };
            modules::media::organizer::run_organize_cli(&target, dry_run)?;
        }
    }
    Ok(())
}

fn resolve_selected_modules(cli: &Cli) -> Result<Option<Vec<String>>> {
    let non_interactive = cli.all || cli.yes || !std::io::stdin().is_terminal();
    if non_interactive {
        let all_ids = get_available_modules()
            .into_iter()
            .map(|m| m.id.to_string())
            .collect();
        return Ok(Some(all_ids));
    }

    let Some(mods) = tui::select_modules()? else {
        println!("\n  {} Setup cancelled.", "•".dimmed());
        return Ok(None);
    };

    if mods.is_empty() {
        println!("\n  {} No modules selected.", "•".dimmed());
        return Ok(None);
    }

    Ok(Some(mods))
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

fn print_summary(
    module_ids: &[String],
    total_duration: std::time::Duration,
    timings: &[(String, std::time::Duration)],
) {
    let total_str = runner::format_duration(total_duration);
    let title_line = format!("✨ 領域 (Ryoiki) Server Setup Complete in {total_str}!");
    let border_len = (title_line.chars().count() + 6).max(52);
    let border = "─".repeat(border_len);

    println!("  {}", border.dimmed());
    println!("  {}", title_line.green().bold());
    println!("  {}", border.dimmed());

    if !timings.is_empty() {
        println!("  • {}", "Timings:".dimmed());
        for (name, dur) in timings {
            let dur_str = runner::format_duration(*dur);
            println!("    {} {:<26} {}", "•".dimmed(), name, dur_str.cyan());
        }
        println!();
    }

    print_module_highlights(module_ids);

    println!(
        "  • {} Log saved to ~/.local/state/ryoiki/install.log",
        "Debug:   ".dimmed()
    );
    println!("  {}\n", border.dimmed());
}

fn print_module_highlights(module_ids: &[String]) {
    print_cli_highlights(module_ids);
    print_infra_highlights(module_ids);
}

fn print_cli_highlights(module_ids: &[String]) {
    if module_ids
        .iter()
        .any(|m| m == "dotfiles" || m == "cli_tools" || m == "trash")
    {
        println!(
            "  • {} eza (ls) • bat (cat) • zoxide (cd) • toss (rm)",
            "Aliases: ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "dev_runtimes") {
        println!(
            "  • {} Go • Rust (cargo) • Python (uv) • JavaScript (bun)",
            "Runtimes:".dimmed()
        );
    }
}

fn print_infra_highlights(module_ids: &[String]) {
    if module_ids.iter().any(|m| m == "security") {
        println!("  • {} UFW (22, 80, 443) active", "Security:".dimmed());
    }
    if module_ids.iter().any(|m| m == "docker") {
        println!(
            "  • {} Docker Engine & Docker Compose plugin active",
            "Docker:  ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "jellyfin") {
        println!(
            "  • {} Jellyfin live on port 8096 (Intel QSV enabled)",
            "Media:   ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "torrent") {
        println!(
            "  • {} qBittorrent live on port 6881 (Web UI enabled)",
            "Torrent: ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "tailscale") {
        println!(
            "  • {} Tailscale MagicDNS active (connect via hostname)",
            "Mesh VPN:".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "charge_limit") {
        println!(
            "  • {} Battery charge limit active (persists across reboots)",
            "Battery: ".dimmed()
        );
    }
}

fn run_system_check() {
    let tools = [
        ("git", "Git VCS"),
        ("tmux", "Tmux Terminal Multiplexer"),
        ("nvim", "Neovim Text Editor"),
        ("gh", "GitHub CLI"),
        ("eza", "Eza modern ls"),
        ("bat", "Bat syntax-highlighting cat"),
        ("zoxide", "Zoxide smarter cd"),
        ("fzf", "FZF fuzzy finder"),
        ("go", "Go programming language"),
        ("rustc", "Rust compiler"),
        ("cargo", "Cargo package manager"),
        ("uv", "uv Python tool"),
        ("bun", "Bun JS/TS runtime"),
        ("docker", "Docker Engine"),
        ("starship", "Starship shell prompt"),
        ("fastfetch", "Fastfetch system stats"),
        ("toss", "toss-rs trash manager"),
        ("tailscale", "Tailscale Mesh VPN"),
    ];

    println!("  {} System Tool Audit:\n", "🔍".bold());
    for (cmd, desc) in tools {
        let exists = Runner::command_exists(cmd);
        let status = if exists {
            "✔ installed".green().bold()
        } else {
            "✖ missing".red().dimmed()
        };
        println!("    {cmd:<12} {desc:<32} {status}");
    }
    println!();
}
