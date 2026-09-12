use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

const TIMER_NAME: &str = "ryoiki-backup.timer";
const SERVICE_NAME: &str = "ryoiki-backup.service";

const SERVICE_CONTENT: &str = r"[Unit]
Description=Ryoiki Jellyfin Cloud Backup
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/ryoiki backup now
";

const TIMER_CONTENT: &str = r"[Unit]
Description=Run Ryoiki Jellyfin Backup Nightly

[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true

[Install]
WantedBy=timers.target
";

/// Deploys and activates the nightly systemd user timer for automated backups.
pub fn deploy_backup_timer(home: &str) -> Result<()> {
    let systemd_dir = Path::new(home).join(".config/systemd/user");
    fs::create_dir_all(&systemd_dir)
        .with_context(|| format!("Failed to create {}", systemd_dir.display()))?;

    let service_file = systemd_dir.join(SERVICE_NAME);
    fs::write(&service_file, SERVICE_CONTENT)
        .with_context(|| format!("Failed to write {}", service_file.display()))?;

    let timer_file = systemd_dir.join(TIMER_NAME);
    fs::write(&timer_file, TIMER_CONTENT)
        .with_context(|| format!("Failed to write {}", timer_file.display()))?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let status = Command::new("systemctl")
        .args(["--user", "enable", "--now", TIMER_NAME])
        .status()?;

    if status.success() {
        println!(
            "  {} Nightly backup timer enabled (runs daily at 03:00 AM)",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to enable backup timer", "✖".red().bold());
    }
    Ok(())
}

/// Checks whether the nightly systemd backup timer is active.
#[must_use]
pub fn is_timer_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", TIMER_NAME])
        .status()
        .is_ok_and(|s| s.success())
}
