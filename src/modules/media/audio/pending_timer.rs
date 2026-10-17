//! Hourly systemd user timer that sends audio language reminders and applies
//! expired decisions.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const TIMER_NAME: &str = "ryoiki-strip-pending.timer";
const SERVICE_NAME: &str = "ryoiki-strip-pending.service";

const SERVICE_CONTENT: &str = r"[Unit]
Description=Ryoiki Audio Language Pending Reminder
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/ryoiki audio pending --notify
";

const TIMER_CONTENT: &str = r"[Unit]
Description=Send Ryoiki audio language reminders hourly

[Timer]
OnBootSec=10min
OnUnitActiveSec=1h
Persistent=true

[Install]
WantedBy=timers.target
";

/// Deploys and activates the hourly audio language reminder timer.
pub fn deploy_pending_timer(home: &str) -> Result<()> {
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
            "  {} Audio language reminder timer enabled (hourly)",
            "✔".green().bold()
        );
    } else {
        println!(
            "  {} Failed to enable audio language reminder timer",
            "✖".red().bold()
        );
    }
    Ok(())
}

/// Checks whether the hourly audio language reminder timer is active.
#[must_use]
pub fn is_timer_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", TIMER_NAME])
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_timer_cadence() {
        assert!(TIMER_CONTENT.contains("OnUnitActiveSec=1h"));
    }

    #[test]
    fn test_pending_service_command() {
        assert!(SERVICE_CONTENT.contains("ryoiki audio pending --notify"));
    }
}
