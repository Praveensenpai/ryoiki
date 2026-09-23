use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const TIMER_NAME: &str = "ryoiki-strip-retry.timer";
const SERVICE_NAME: &str = "ryoiki-strip-retry.service";

const SERVICE_CONTENT: &str = r"[Unit]
Description=Ryoiki Dubstrip Retry Queue Processor
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/ryoiki audio retry
";

const TIMER_CONTENT: &str = r"[Unit]
Description=Run Ryoiki Dubstrip Retry Queue every hour

[Timer]
OnBootSec=5min
OnUnitActiveSec=1h
Persistent=true

[Install]
WantedBy=timers.target
";

/// Deploys and activates the hourly systemd user timer for dubstrip retries.
pub fn deploy_retry_timer(home: &str) -> Result<()> {
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
            "  {} Dubstrip retry timer enabled (runs hourly, up to 24 retries per file)",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to enable strip retry timer", "✖".red().bold());
    }
    Ok(())
}

/// Checks whether the systemd strip retry timer is currently active.
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
    fn test_timer_content_cadence() {
        assert!(TIMER_CONTENT.contains("OnUnitActiveSec=1h"));
        assert!(TIMER_CONTENT.contains("OnBootSec=5min"));
    }

    #[test]
    fn test_service_content_command() {
        assert!(SERVICE_CONTENT.contains("ryoiki audio retry"));
    }
}
