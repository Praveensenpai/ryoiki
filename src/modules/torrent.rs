use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Sets up qBittorrent server directly with Docker without compose files.
pub fn setup(runner: &mut Runner) -> Result<()> {
    ensure_docker(runner)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = Path::new(&home).join(".config/qbittorrent");
    let download_dir = Path::new(&home).join("torrents");

    create_directories(runner, &config_dir, &download_dir)?;

    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };

    start_container(runner, &config_dir, &download_dir, (uid, gid))?;
    configure_firewall(runner);
    print_access_info(&home);

    Ok(())
}

fn ensure_docker(runner: &mut Runner) -> Result<()> {
    if !Runner::command_exists("docker") {
        super::docker::setup(runner)?;
    }
    Ok(())
}

fn create_directories(runner: &Runner, config_dir: &Path, download_dir: &Path) -> Result<()> {
    for dir in [config_dir, download_dir] {
        if !runner.dry_run && !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }
    }
    Ok(())
}

fn start_container(
    runner: &mut Runner,
    config: &Path,
    download: &Path,
    (uid, gid): (u32, u32),
) -> Result<()> {
    if runner.dry_run {
        println!("  • [dry-run] docker rm -f qbittorrent");
        println!("  • [dry-run] docker run -d --name qbittorrent --restart unless-stopped ...");
        return Ok(());
    }

    let _ = std::process::Command::new("docker")
        .args(["rm", "-f", "qbittorrent"])
        .output();

    let cfg_vol = format!("{}:/config", config.display());
    let dl_vol = format!("{}:/downloads", download.display());
    let user_id_env = format!("PUID={uid}");
    let group_id_env = format!("PGID={gid}");

    runner.exec_silent(
        "Starting qBittorrent container...",
        "docker",
        &[
            "run",
            "-d",
            "--name",
            "qbittorrent",
            "--restart",
            "unless-stopped",
            "-e",
            &user_id_env,
            "-e",
            &group_id_env,
            "-e",
            "TZ=Etc/UTC",
            "-e",
            "WEBUI_PORT=6881",
            "-e",
            "TORRENTING_PORT=6882",
            "-p",
            "6881:6881",
            "-p",
            "6882:6882",
            "-p",
            "6882:6882/udp",
            "-v",
            &cfg_vol,
            "-v",
            &dl_vol,
            "lscr.io/linuxserver/qbittorrent:latest",
        ],
    )?;

    std::thread::sleep(std::time::Duration::from_millis(1500));
    Ok(())
}

fn configure_firewall(runner: &mut Runner) {
    if !Runner::command_exists("ufw") {
        return;
    }

    let can_sudo = std::process::Command::new("sudo")
        .args(["-n", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if can_sudo {
        let _ = runner.exec_silent(
            "Allowing qBittorrent Web UI port (6881/tcp) in UFW...",
            "sudo",
            &["ufw", "allow", "6881/tcp"],
        );
        let _ = runner.exec_silent(
            "Allowing BitTorrent peer ports (6882) in UFW...",
            "sudo",
            &["ufw", "allow", "6882"],
        );
    }
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
    println!("  {} Downloads directory: {home}/torrents", "•".dimmed());
}
