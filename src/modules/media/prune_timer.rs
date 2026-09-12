use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const TIMER_NAME: &str = "ryoiki-prune.timer";
pub const SERVICE_NAME: &str = "ryoiki-prune.service";

fn render_service_content(threshold: u8, target: u8) -> String {
    format!(
        r"[Unit]
Description=Ryoiki Media Storage Pruner
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/ryoiki prune --threshold {threshold} --target {target}
"
    )
}

const TIMER_CONTENT: &str = r"[Unit]
Description=Run Ryoiki Media Storage Pruner every 6 hours

[Timer]
OnBootSec=10min
OnUnitActiveSec=6h
Persistent=true

[Install]
WantedBy=timers.target
";

/// Deploys and activates the 6-hour systemd user timer for media storage pruning.
pub fn deploy_prune_timer(home: &str, threshold: u8, target: u8) -> Result<()> {
    let systemd_dir = Path::new(home).join(".config/systemd/user");
    fs::create_dir_all(&systemd_dir)
        .with_context(|| format!("Failed to create {}", systemd_dir.display()))?;

    let service_file = systemd_dir.join(SERVICE_NAME);
    let service_content = render_service_content(threshold, target);
    fs::write(&service_file, service_content)
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
            "  {} Automated pruner timer enabled (checks SSD every 6h, threshold: {threshold}%, target: {target}%)",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to enable pruner timer", "✖".red().bold());
    }
    Ok(())
}

/// Checks whether the systemd pruner timer is currently active.
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
    fn test_render_service_content() {
        let content = render_service_content(85, 75);
        assert!(content.contains("--threshold 85"));
        assert!(content.contains("--target 75"));
    }

    #[test]
    fn test_timer_content_has_6h_cadence() {
        assert!(TIMER_CONTENT.contains("OnUnitActiveSec=6h"));
        assert!(TIMER_CONTENT.contains("OnBootSec=10min"));
    }
}
