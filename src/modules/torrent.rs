use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Sets up qBittorrent server with Docker Compose and Web UI.
pub fn setup(runner: &mut Runner) -> Result<()> {
    ensure_docker(runner)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let torrent_dir = Path::new(&home).join("torrents");

    create_directories(runner, &torrent_dir)?;
    deploy_compose_file(runner, &torrent_dir)?;
    start_container(runner, &torrent_dir)?;
    configure_firewall(runner)?;
    print_access_info(&home);

    Ok(())
}

fn ensure_docker(runner: &mut Runner) -> Result<()> {
    if !Runner::command_exists("docker") {
        super::docker::setup(runner)?;
    }
    Ok(())
}

fn create_directories(runner: &Runner, base: &Path) -> Result<()> {
    let dirs = [base.join("config"), base.join("downloads")];

    for dir in &dirs {
        if !runner.dry_run && !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }
    }
    Ok(())
}

fn deploy_compose_file(runner: &Runner, base: &Path) -> Result<()> {
    let compose_file = base.join("docker-compose.yml");
    if compose_file.exists() {
        return Ok(());
    }

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    let compose_content = generate_compose_yaml(uid, gid);

    if runner.dry_run {
        println!("  • [dry-run] Create {}", compose_file.display());
    } else {
        fs::write(&compose_file, compose_content)
            .with_context(|| format!("Failed to write: {}", compose_file.display()))?;
    }
    Ok(())
}

fn generate_compose_yaml(uid: u32, gid: u32) -> String {
    format!(
        r#"services:
  qbittorrent:
    image: lscr.io/linuxserver/qbittorrent:latest
    container_name: qbittorrent
    environment:
      - PUID={uid}
      - PGID={gid}
      - TZ=Etc/UTC
      - WEBUI_PORT=6881
      - TORRENTING_PORT=6882
    volumes:
      - ./config:/config
      - ./downloads:/downloads
    ports:
      - "6881:6881"
      - "6882:6882"
      - "6882:6882/udp"
    restart: unless-stopped
"#
    )
}

fn start_container(runner: &mut Runner, base: &Path) -> Result<()> {
    let dir_str = base.to_string_lossy();
    runner.exec_bash(
        "Starting qBittorrent server...",
        &format!("cd '{dir_str}' && docker compose up -d"),
    )
}

fn configure_firewall(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("ufw") {
        runner.exec_silent(
            "Allowing qBittorrent Web UI port (6881/tcp) in UFW...",
            "sudo",
            &["ufw", "allow", "6881/tcp"],
        )?;
        runner.exec_silent(
            "Allowing BitTorrent peer ports (6882) in UFW...",
            "sudo",
            &["ufw", "allow", "6882"],
        )?;
    }
    Ok(())
}

fn get_access_urls() -> (String, String) {
    let ts_ip = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_or_else(
            |_| String::new(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        );

    let hostname = std::process::Command::new("hostname").output().map_or_else(
        |_| String::new(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
    );

    (ts_ip, hostname)
}

fn get_initial_password() -> Option<String> {
    let output = std::process::Command::new("docker")
        .args(["logs", "qbittorrent"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for line in stdout.lines().chain(stderr.lines()) {
        if line.contains("temporary password is provided for this session:") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                return Some(parts[parts.len() - 1].trim().to_string());
            }
        }
    }
    None
}

fn print_access_info(home: &str) {
    let (ts_ip, host) = get_access_urls();
    let url = if ts_ip.is_empty() {
        if host.is_empty() {
            "http://<server-ip>:6881".to_string()
        } else {
            format!("http://{host}:6881")
        }
    } else {
        format!("http://{ts_ip}:6881")
    };

    println!("  {} qBittorrent web UI: {url}", "✔".green().bold());
    if !host.is_empty() && !ts_ip.is_empty() {
        println!("  {} MagicDNS URL: http://{host}:6881", "•".dimmed());
    }
    println!("  {} Default username: admin", "•".dimmed());

    if let Some(pwd) = get_initial_password() {
        println!(
            "  {} Temporary password: {}",
            "✔".green().bold(),
            pwd.cyan().bold()
        );
    } else {
        println!(
            "  {} Check password: docker logs qbittorrent | grep -i password",
            "•".dimmed()
        );
    }
    println!(
        "  {} Downloads directory: {home}/torrents/downloads",
        "•".dimmed()
    );
}
