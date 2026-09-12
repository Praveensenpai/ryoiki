use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::modules::rclone;
use crate::notify::client::format_card;
use crate::notify::TelegramConfig;
use crate::runner::format_duration;

pub const TIMER_NAME: &str = "ryoiki-media-sync.timer";
pub const SERVICE_NAME: &str = "ryoiki-media-sync.service";

const SERVICE_CONTENT: &str = r"[Unit]
Description=Ryoiki Daily Media Cloud Sync to Google Drive
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/ryoiki media sync
";

const TIMER_CONTENT: &str = r"[Unit]
Description=Daily Google Drive Media Sync (30 mins after boot)

[Timer]
OnBootSec=30min
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
";

/// Deploys and activates the systemd user timer for post-boot daily media sync.
pub fn deploy_sync_timer(home: &str) -> Result<()> {
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
            "  {} Daily media sync timer enabled (runs 30m after boot & every 24h)",
            "✔".green().bold()
        );
    } else {
        println!("  {} Failed to enable media sync timer", "✖".red().bold());
    }
    Ok(())
}

/// Checks whether the systemd daily media sync timer is active.
#[must_use]
pub fn is_timer_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", TIMER_NAME])
        .status()
        .is_ok_and(|s| s.success())
}

/// Resolves remote destination folder for a given local media folder name.
#[must_use]
pub fn resolve_remote_category(folder_name: &str) -> String {
    match folder_name {
        "movies" => "media/movie".to_string(),
        "shows" => "media/shows".to_string(),
        "anime" => "media/anime".to_string(),
        other => format!("media/{other}"),
    }
}

/// Runs a non-destructive mirror copy of local media to Google Drive.
pub fn run_media_sync() -> Result<()> {
    println!(
        "\n  {} {}",
        "☁️".cyan(),
        "Daily Media Cloud Sync Engine".bold()
    );
    println!("  {}\n", "─".repeat(45).dimmed());

    if !rclone::is_remote_configured() {
        bail!("Google Drive remote 'gdrive:' is not configured. Run 'ryoiki rclone setup' first.");
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let media_base = Path::new(&home).join("jellyfin/media");

    if !media_base.exists() {
        println!(
            "  {} No media directory found at {}\n",
            "ℹ".cyan(),
            media_base.display()
        );
        return Ok(());
    }

    let start = Instant::now();
    let categories = sync_media_categories(&media_base)?;

    let elapsed_str = format_duration(start.elapsed());
    println!(
        "\n  {} Daily media sync completed in {}\n",
        "✨".green().bold(),
        elapsed_str.cyan().bold()
    );

    send_sync_notification(&elapsed_str, &categories);
    Ok(())
}

fn sync_media_categories(base: &Path) -> Result<Vec<String>> {
    let mut synced = Vec::new();
    let entries = fs::read_dir(base).with_context(|| format!("Cannot read {}", base.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let remote_rel = resolve_remote_category(&folder_name);
        let remote_dest = format!("gdrive:{remote_rel}/");

        println!(
            "  {} Syncing {} -> {}",
            "▶".cyan(),
            folder_name.bold(),
            remote_dest.dimmed()
        );

        copy_folder_to_remote(&path, &remote_dest)?;
        synced.push(folder_name);
    }
    Ok(synced)
}

fn copy_folder_to_remote(src: &Path, remote_dest: &str) -> Result<()> {
    let status = Command::new("rclone")
        .args([
            "copy",
            &src.to_string_lossy(),
            remote_dest,
            "--transfers=4",
            "--drive-chunk-size=32M",
        ])
        .status()
        .context("Failed to execute rclone copy")?;

    if !status.success() {
        bail!("Rclone copy failed for {}", src.display());
    }
    Ok(())
}

fn send_sync_notification(elapsed: &str, categories: &[String]) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };

    let cat_list = if categories.is_empty() {
        "None".to_string()
    } else {
        categories.join(", ")
    };

    let fields = [
        ("📁 Source:", "~/jellyfin/media/"),
        ("☁️ Destination:", "gdrive:media/"),
        ("🎬 Categories:", cat_list.as_str()),
        ("⏱️ Duration:", elapsed),
        ("🔒 Policy:", "Copy (Local preserved)"),
    ];

    let card = format_card(
        "Media Sync",
        "✨ <b>DAILY MEDIA BACKUP COMPLETE</b>",
        &fields,
    );
    let _ = crate::notify::client::send_alert(&cfg.bot_token, &cfg.chat_id, &card);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_remote_category_movies() {
        assert_eq!(resolve_remote_category("movies"), "media/movie");
    }

    #[test]
    fn test_resolve_remote_category_shows() {
        assert_eq!(resolve_remote_category("shows"), "media/shows");
    }

    #[test]
    fn test_resolve_remote_category_anime() {
        assert_eq!(resolve_remote_category("anime"), "media/anime");
    }

    #[test]
    fn test_resolve_remote_category_custom() {
        assert_eq!(resolve_remote_category("music"), "media/music");
    }

    #[test]
    fn test_timer_content_cadence() {
        assert!(TIMER_CONTENT.contains("OnBootSec=30min"));
        assert!(TIMER_CONTENT.contains("OnCalendar=daily"));
    }
}
