use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

/// Sets up Caddy reverse proxy configured for Tailscale `MagicDNS` HTTPS.
pub fn setup(runner: &mut Runner) -> Result<()> {
    install_caddy(runner)?;

    let hostname = detect_tailscale_hostname();
    let caddyfile = build_caddyfile(&hostname);

    write_caddyfile(runner, &caddyfile)?;
    enable_caddy(runner)?;
    print_access_info(&hostname);

    Ok(())
}

fn install_caddy(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("caddy") {
        println!("  {} Caddy is already installed.", "•".dimmed());
        return Ok(());
    }

    runner.exec_bash(
        "Adding Caddy official APT repository...",
        r"
        sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
            | sudo gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
            | sudo tee /etc/apt/sources.list.d/caddy-stable.list > /dev/null
        sudo apt-get update -y
        ",
    )?;

    runner.apt_install("Installing Caddy web server...", &["caddy"])?;
    Ok(())
}

fn detect_tailscale_hostname() -> String {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output();

    let Ok(out) = output else {
        return String::new();
    };

    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);

    json["Self"]["DNSName"]
        .as_str()
        .map(|s| s.trim_end_matches('.').to_string())
        .unwrap_or_default()
}

fn build_caddyfile(hostname: &str) -> String {
    if hostname.is_empty() {
        return r"
# Tailscale hostname not detected — edit this file with your hostname.
# Example: http://100.x.x.x

http:// {
    handle /jellyfin* {
        reverse_proxy localhost:8096
    }
    handle {
        reverse_proxy localhost:6881
    }
}
"
        .to_string();
    }

    format!(
        r"
# Caddy reverse proxy — Tailscale HTTPS (auto-cert via ts.net)
{hostname} {{
    handle /jellyfin* {{
        reverse_proxy localhost:8096
    }}
    handle {{
        reverse_proxy localhost:6881
    }}
}}
"
    )
}

fn write_caddyfile(runner: &Runner, content: &str) -> Result<()> {
    if runner.dry_run {
        println!("  {} [dry-run] Write /etc/caddy/Caddyfile", "•".dimmed());
        return Ok(());
    }

    let tmp = "/tmp/ryoiki_caddyfile";
    fs::write(tmp, content).context("Failed to write temp Caddyfile")?;

    std::process::Command::new("sudo")
        .args(["cp", tmp, "/etc/caddy/Caddyfile"])
        .status()
        .context("Failed to copy Caddyfile to /etc/caddy/")?;

    let _ = fs::remove_file(tmp);
    Ok(())
}

fn enable_caddy(runner: &mut Runner) -> Result<()> {
    runner.exec_silent(
        "Enabling and reloading Caddy service...",
        "sudo",
        &["systemctl", "enable", "--now", "caddy"],
    )?;
    runner.exec_silent(
        "Reloading Caddy configuration...",
        "sudo",
        &["systemctl", "reload", "caddy"],
    )
}

fn print_access_info(hostname: &str) {
    if hostname.is_empty() {
        println!(
            "  {} Caddy running — edit /etc/caddy/Caddyfile with your Tailscale hostname.",
            "✔".green().bold()
        );
    } else {
        println!(
            "  {} Caddy reverse proxy active at https://{}",
            "✔".green().bold(),
            hostname.cyan().bold()
        );
        println!(
            "  {} Jellyfin  → https://{}/jellyfin",
            "•".dimmed(),
            hostname
        );
        println!("  {} qBittorrent → https://{}", "•".dimmed(), hostname);
    }
}
