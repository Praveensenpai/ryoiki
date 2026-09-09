pub mod cli_tools;
pub mod dev_runtimes;
pub mod docker;
pub mod essentials;
pub mod git_ssh;
pub mod jellyfin;
pub mod prompt;
pub mod security;
pub mod tailscale;
pub mod torrent;
pub mod trash;

use crate::configs;
use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;

/// Metadata describing an installable setup module.
#[derive(Clone, Debug)]
pub struct Module {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub default_enabled: bool,
    pub deps: &'static [&'static str],
}

/// Checks if any of the selected module IDs require root (sudo) privileges.
pub fn requires_sudo(modules: &[String]) -> bool {
    modules.iter().any(|m| {
        matches!(
            m.as_str(),
            "essentials"
                | "cli_tools"
                | "dev_runtimes"
                | "security"
                | "docker"
                | "jellyfin"
                | "torrent"
                | "prompt"
                | "tailscale"
        )
    })
}

/// Returns the registry of all available provisioning modules in execution order.
pub fn get_available_modules() -> Vec<Module> {
    let mut mods = core_modules();
    mods.extend(platform_modules());
    mods.extend(environment_modules());
    mods
}

fn core_modules() -> Vec<Module> {
    vec![
        Module {
            id: "git_ssh",
            title: "Git & SSH Key Setup",
            description: "Ed25519 key generation & GitHub link",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "essentials",
            title: "System Essentials",
            description: "git, tmux, neovim, adb, and GitHub CLI (gh)",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "cli_tools",
            title: "Modern CLI Suite",
            description: "eza, bat, zoxide, fzf, and ble.sh",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "dev_runtimes",
            title: "Dev Runtimes",
            description: "Go, Rust (rustup), Python (uv), JavaScript (Bun)",
            default_enabled: true,
            deps: &[],
        },
    ]
}

fn platform_modules() -> Vec<Module> {
    vec![
        Module {
            id: "security",
            title: "Server Security",
            description: "UFW Firewall (22, 80, 443)",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "docker",
            title: "Docker Platform",
            description: "Docker Engine CE & Docker Compose plugin",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "jellyfin",
            title: "Jellyfin Media Server",
            description: "Dockerized media streaming with Intel QuickSync / VAAPI GPU",
            default_enabled: false,
            deps: &["docker"],
        },
        Module {
            id: "torrent",
            title: "qBittorrent Server",
            description: "Dockerized BitTorrent client with Web UI (port 6881)",
            default_enabled: false,
            deps: &["docker"],
        },
    ]
}

fn environment_modules() -> Vec<Module> {
    vec![
        Module {
            id: "prompt",
            title: "Shell Prompt",
            description: "Starship cross-shell prompt & Fastfetch CLI utility",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "trash",
            title: "Trash Manager",
            description: "toss-rs (FreeDesktop trash TUI & rm alias)",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "tailscale",
            title: "Tailscale Mesh VPN",
            description: "WireGuard mesh, MagicDNS (hostname SSH) & Tailscale SSH",
            default_enabled: true,
            deps: &[],
        },
        Module {
            id: "dotfiles",
            title: "Aesthetic Dotfiles",
            description: "Deploy embedded ~/.tmux.conf, ~/.bash_aliases, starship.toml",
            default_enabled: true,
            deps: &[],
        },
    ]
}

/// Resolves and appends any missing dependencies for the selected modules,
/// preserving canonical module execution order.
pub fn resolve_dependencies(selected_ids: &[String]) -> Vec<String> {
    let all_modules = get_available_modules();
    let mut needed = HashSet::new();

    for id in selected_ids {
        needed.insert(id.as_str());
    }

    let mut changed = true;
    while changed {
        changed = false;
        for module in &all_modules {
            if needed.contains(module.id) {
                for &dep in module.deps {
                    if needed.insert(dep) {
                        changed = true;
                    }
                }
            }
        }
    }

    all_modules
        .into_iter()
        .filter(|m| needed.contains(m.id))
        .map(|m| m.id.to_string())
        .collect()
}

/// Dispatches and executes a selected module by its unique identifier.
pub fn execute_module(module_id: &str, runner: &mut Runner, non_interactive: bool) -> Result<()> {
    match module_id {
        "git_ssh" => git_ssh::setup(runner, non_interactive),
        "essentials" => essentials::setup(runner),
        "cli_tools" => cli_tools::setup(runner),
        "dev_runtimes" => dev_runtimes::setup(runner),
        "security" => security::setup(runner),
        "docker" => docker::setup(runner),
        "jellyfin" => jellyfin::setup(runner),
        "torrent" => torrent::setup(runner, non_interactive),
        "prompt" => prompt::setup(runner),
        "trash" => trash::setup(runner),
        "tailscale" => tailscale::setup(runner, non_interactive),
        "dotfiles" => deploy_dotfiles_module(runner),
        _ => anyhow::bail!("Unknown module: {module_id}"),
    }
}

fn deploy_dotfiles_module(runner: &Runner) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    if runner.dry_run {
        println!("  {} [dry-run] Deploying dotfiles to {home}", "•".dimmed());
    } else {
        let start = std::time::Instant::now();
        configs::deploy_dotfiles(&home)?;
        let elapsed = crate::runner::format_duration(start.elapsed());
        println!(
            "  {} Dotfiles deployed (~/.tmux.conf, ~/.bash_aliases, starship.toml) {}",
            "✔".green().bold(),
            format!("({elapsed})").dimmed()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_dependencies_adds_prerequisites() {
        let chosen = vec!["torrent".to_string()];
        let resolved = resolve_dependencies(&chosen);
        assert!(resolved.contains(&"docker".to_string()));
        assert!(resolved.contains(&"torrent".to_string()));

        let docker_pos = resolved.iter().position(|x| x == "docker");
        let torrent_pos = resolved.iter().position(|x| x == "torrent");
        assert!(docker_pos < torrent_pos);
    }

    #[test]
    fn test_resolve_dependencies_deduplicates() {
        let chosen = vec!["jellyfin".to_string(), "torrent".to_string()];
        let resolved = resolve_dependencies(&chosen);
        let docker_count = resolved.iter().filter(|x| *x == "docker").count();
        assert_eq!(docker_count, 1);
    }

    #[test]
    fn test_resolve_dependencies_noop_when_no_deps() {
        let chosen = vec!["git_ssh".to_string(), "essentials".to_string()];
        let resolved = resolve_dependencies(&chosen);
        assert_eq!(resolved, chosen);
    }
}
