pub mod git_ssh;
pub mod essentials;
pub mod cli_tools;
pub mod dev_runtimes;
pub mod security;
pub mod docker;
pub mod prompt;
pub mod trash;

use anyhow::Result;
use colored::*;
use crate::configs;
use crate::runner::Runner;

#[derive(Clone, Debug)]
pub struct Module {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub default_enabled: bool,
}

pub fn get_available_modules() -> Vec<Module> {
    vec![
        Module {
            id: "git_ssh",
            title: "Git & SSH Key Setup",
            description: "Ed25519 key generation & GitHub link",
            default_enabled: true,
        },
        Module {
            id: "essentials",
            title: "System Essentials",
            description: "git, tmux, neovim, and GitHub CLI (gh)",
            default_enabled: true,
        },
        Module {
            id: "cli_tools",
            title: "Modern CLI Suite",
            description: "eza, bat, zoxide, fzf, and ble.sh",
            default_enabled: true,
        },
        Module {
            id: "dev_runtimes",
            title: "Dev Runtimes",
            description: "Go, Rust (rustup), Python (uv), JavaScript (Bun)",
            default_enabled: true,
        },
        Module {
            id: "security",
            title: "Server Security",
            description: "UFW Firewall (22, 80, 443) & Fail2Ban brute-force guard",
            default_enabled: true,
        },
        Module {
            id: "docker",
            title: "Docker Platform",
            description: "Docker Engine CE & Docker Compose plugin",
            default_enabled: true,
        },
        Module {
            id: "prompt",
            title: "Prompt & Banner",
            description: "Starship prompt & Fastfetch system stats banner",
            default_enabled: true,
        },
        Module {
            id: "trash",
            title: "Trash Manager",
            description: "toss-rs (FreeDesktop trash TUI & rm alias)",
            default_enabled: true,
        },
        Module {
            id: "dotfiles",
            title: "Aesthetic Dotfiles",
            description: "Deploy embedded ~/.tmux.conf, ~/.bash_aliases, starship.toml",
            default_enabled: true,
        },
    ]
}

pub fn execute_module(module_id: &str, runner: &mut Runner, non_interactive: bool) -> Result<()> {
    match module_id {
        "git_ssh" => git_ssh::setup(runner, non_interactive),
        "essentials" => essentials::setup(runner),
        "cli_tools" => cli_tools::setup(runner),
        "dev_runtimes" => dev_runtimes::setup(runner),
        "security" => security::setup(runner),
        "docker" => docker::setup(runner),
        "prompt" => prompt::setup(runner),
        "trash" => trash::setup(runner),
        "dotfiles" => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            if runner.dry_run {
                println!("  {} [dry-run] Deploying dotfiles to {}", "•".dimmed(), home);
            } else {
                configs::deploy_dotfiles(&home)?;
                println!("  {} Dotfiles deployed (~/.tmux.conf, ~/.bash_aliases, starship.toml)", "✔".green().bold());
            }
            Ok(())
        }
        _ => anyhow::bail!("Unknown module: {module_id}"),
    }
}
