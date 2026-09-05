mod configs;
mod modules;
mod runner;
mod tui;

use std::io::IsTerminal;
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use modules::{execute_module, get_available_modules};
use runner::Runner;

#[derive(Parser)]
#[command(name = "ryoiki")]
#[command(author = "Praveensenpai <pvnt20@gmail.com>")]
#[command(version = "0.1.0")]
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
    /// Deploy only embedded dotfiles (~/.tmux.conf, ~/.bash_aliases, starship.toml)
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
        match cmd {
            Commands::Dotfiles => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                configs::deploy_dotfiles(&home)?;
                println!("  {} Dotfiles deployed successfully to {}", "✔".green().bold(), home.cyan());
                return Ok(());
            }
            Commands::Check => {
                run_system_check(&runner);
                return Ok(());
            }
            Commands::Run { modules } => {
                run_modules(&modules, &mut runner, cli.yes)?;
                print_summary(&modules);
                return Ok(());
            }
        }
    }

    let non_interactive = cli.all || cli.yes || !std::io::stdin().is_terminal();

    let chosen_modules = if non_interactive {
        get_available_modules().into_iter().map(|m| m.id.to_string()).collect()
    } else {
        match tui::select_modules()? {
            Some(mods) => mods,
            None => {
                println!("\n  {} Setup cancelled.", "•".dimmed());
                return Ok(());
            }
        }
    };

    if chosen_modules.is_empty() {
        println!("\n  {} No modules selected.", "•".dimmed());
        return Ok(());
    }

    println!("\n  {} Running {} selected modules...\n", "▶".cyan().bold(), chosen_modules.len());

    run_modules(&chosen_modules, &mut runner, non_interactive)?;
    print_summary(&chosen_modules);

    Ok(())
}

fn print_banner() {
    println!("\n  {} {}", "領域".cyan().bold(), "Ryoiki Server Setup".bold());
    println!("  {}\n", "─".repeat(42).dimmed());
}

fn run_modules(module_ids: &[String], runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let all_mods = get_available_modules();
    let total = module_ids.len();

    for (i, id) in module_ids.iter().enumerate() {
        if let Some(meta) = all_mods.iter().find(|m| m.id == id) {
            println!("  [{}/{}] {}", i + 1, total, meta.title.bold());
            execute_module(id, runner, non_interactive)?;
            println!();
        }
    }

    Ok(())
}

fn print_summary(module_ids: &[String]) {
    println!("  {}", "─".repeat(48).dimmed());
    println!("  {} {}", "✨".bold(), "領域 (Ryoiki) Server Setup Complete!".green().bold());
    println!("  {}", "─".repeat(48).dimmed());

    if module_ids.iter().any(|m| m == "dotfiles" || m == "cli_tools" || m == "trash") {
        println!("  • {} eza (ls) • bat (cat) • zoxide (cd) • toss (rm)", "Aliases:".dimmed());
    }
    if module_ids.iter().any(|m| m == "dev_runtimes") {
        println!("  • {} Go • Rust (cargo) • Python (uv) • JavaScript (bun)", "Runtimes:".dimmed());
    }
    if module_ids.iter().any(|m| m == "security") {
        println!("  • {} UFW (22, 80, 443) • Fail2Ban guard active", "Security:".dimmed());
    }
    if module_ids.iter().any(|m| m == "docker") {
        println!("  • {} Docker Engine & Docker Compose plugin active", "Docker:  ".dimmed());
    }

    println!("  • {} Log saved to ~/.local/state/ryoiki/install.log", "Debug:   ".dimmed());
    println!("  {}\n", "─".repeat(48).dimmed());
}

fn run_system_check(runner: &Runner) {
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
    ];

    println!("  {} System Tool Audit:\n", "🔍".bold());
    for (cmd, desc) in tools {
        let exists = runner.command_exists(cmd);
        let status = if exists {
            "✔ installed".green().bold()
        } else {
            "✖ missing".red().dimmed()
        };
        println!("    {:<12} {:<32} {}", cmd.bold(), desc.dimmed(), status);
    }
    println!();
}
