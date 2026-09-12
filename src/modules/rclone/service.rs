use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const SYSTEMD_SERVICE_NAME: &str = "rclone-gdrive.service";

const SYSTEMD_UNIT_CONTENT: &str = r"[Unit]
Description=Rclone Google Drive Mount
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStartPre=-/usr/bin/mkdir -p %h/gdrive
ExecStart=/usr/bin/rclone mount gdrive: %h/gdrive \
    --config=%h/.config/rclone/rclone.conf \
    --allow-other \
    --vfs-cache-mode full \
    --vfs-cache-max-size 10G \
    --vfs-cache-max-age 24h \
    --dir-cache-time 1000h \
    --buffer-size 32M \
    --poll-interval 15s \
    --umask 022
ExecStop=/usr/bin/fusermount3 -u %h/gdrive
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
";

/// Writes the systemd user service unit file to `~/.config/systemd/user/`.
pub fn deploy_systemd_service(runner: &Runner, home: &str) -> Result<()> {
    let systemd_user_dir = Path::new(home).join(".config/systemd/user");
    let service_path = systemd_user_dir.join(SYSTEMD_SERVICE_NAME);

    if !runner.dry_run {
        fs::create_dir_all(&systemd_user_dir)
            .with_context(|| format!("Failed to create {}", systemd_user_dir.display()))?;
        fs::write(&service_path, SYSTEMD_UNIT_CONTENT)
            .with_context(|| format!("Failed to write {}", service_path.display()))?;
    }
    println!("  {} Deployed systemd mount service unit", "✔".green());
    Ok(())
}

/// Enables linger and systemd user service for Google Drive mount.
pub fn enable_and_start_service(runner: &mut Runner) -> Result<()> {
    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
    runner.exec_bash(
        "Enabling user systemd linger (runs without active login)...",
        &format!("sudo loginctl enable-linger '{user}' || true"),
    )?;

    runner.exec_bash(
        "Reloading user systemd daemon...",
        "systemctl --user daemon-reload",
    )?;

    runner.exec_bash(
        "Enabling and starting rclone-gdrive service...",
        &format!("systemctl --user enable --now '{SYSTEMD_SERVICE_NAME}'"),
    )
}

/// Returns whether the systemd user service is currently active.
#[must_use]
pub fn is_service_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", SYSTEMD_SERVICE_NAME])
        .status()
        .is_ok_and(|s| s.success())
}

/// Returns whether a given path is an active mount point.
#[must_use]
pub fn is_path_mounted(path: &Path) -> bool {
    let out = Command::new("findmnt")
        .arg("-n")
        .arg("-o")
        .arg("TARGET")
        .arg(path)
        .output();
    out.is_ok_and(|o| o.status.success() && !o.stdout.is_empty())
}

/// Displays Google Drive storage quota info via Rclone.
pub fn print_quota_stats() {
    println!("\n  {} Storage Quota:", "📊".cyan());
    let output = Command::new("rclone").args(["about", "gdrive:"]).output();
    if let Ok(out) = output {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                println!("    {}", line.trim().dimmed());
            }
        }
    }
}

/// Starts the systemd user mount service.
pub fn start_mount() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "start", SYSTEMD_SERVICE_NAME])
        .status()?;
    if status.success() {
        println!(
            "  {} Started Google Drive mount service",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to start mount service", "✖".red().bold());
    }
    Ok(())
}

/// Stops the systemd user mount service.
pub fn stop_mount() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "stop", SYSTEMD_SERVICE_NAME])
        .status()?;
    if status.success() {
        println!(
            "  {} Stopped Google Drive mount service",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to stop mount service", "✖".red().bold());
    }
    Ok(())
}
