mod configs;
mod modules;
mod runner;
mod tui;

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
#[command(version = "0.1.7")]
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut runner = Runner::new(cli.dry_run, cli.verbose)?;

    print_banner();

    if let Some(cmd) = cli.command {
        return handle_subcommand(cmd, &mut runner, cli.yes);
    }

    let Some(chosen_modules) = resolve_selected_modules(&cli)? else {
        return Ok(());
    };

    if modules::requires_sudo(&chosen_modules) {
        runner.ensure_sudo()?;
    }

    let non_interactive = cli.all || cli.yes || !std::io::stdin().is_terminal();
    println!(
        "\n  {} Running {} selected modules...\n",
        "▶".cyan().bold(),
        chosen_modules.len()
    );

    let (total_dur, timings) = run_modules(&chosen_modules, &mut runner, non_interactive)?;
    print_summary(&chosen_modules, total_dur, &timings);

    Ok(())
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
            if modules::requires_sudo(&modules) {
                runner.ensure_sudo()?;
            }
            let (total_dur, timings) = run_modules(&modules, runner, yes)?;
            print_summary(&modules, total_dur, &timings);
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
        println!(
            "  • {} UFW (22, 80, 443) • Fail2Ban guard active",
            "Security:".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "docker") {
        println!(
            "  • {} Docker Engine & Docker Compose plugin active",
            "Docker:  ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "tailscale") {
        println!(
            "  • {} Tailscale MagicDNS active (connect via hostname)",
            "Mesh VPN:".dimmed()
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
