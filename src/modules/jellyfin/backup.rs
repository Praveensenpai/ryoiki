use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use super::timer;
use crate::modules::rclone;
use crate::notify::client::format_card;
use crate::notify::TelegramConfig;

const CLOUD_BACKUP_DEST: &str = "gdrive:ryoiki-backups/jellyfin/";

/// CLI subcommands for cloud backup management.
#[derive(Subcommand, Debug, Clone, Copy)]
pub enum BackupSubcommand {
    /// Create an immediate compressed backup and upload to Google Drive
    Now,
    /// List existing cloud backups stored on Google Drive
    List,
    /// Enable and activate the nightly 03:00 AM automated backup timer
    Schedule,
}

/// Dispatches CLI subcommands for backup operations.
pub fn handle_cli(sub: BackupSubcommand) -> Result<()> {
    match sub {
        BackupSubcommand::Now => run_backup_now(),
        BackupSubcommand::List => list_backups(),
        BackupSubcommand::Schedule => setup_schedule(),
    }
}

/// Creates a compressed snapshot of Jellyfin config/databases and uploads to Google Drive.
pub fn run_backup_now() -> Result<()> {
    println!(
        "\n  {} {}",
        "📦".cyan(),
        "Jellyfin Cloud Backup Engine".bold()
    );
    println!("  {}\n", "─".repeat(40).dimmed());

    validate_prerequisites()?;

    let start = Instant::now();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let source_dir = Path::new(&home).join("jellyfin/config");

    let (archive_path, size_bytes) = create_archive(&source_dir)?;
    let archive_name = archive_path
        .file_name()
        .map_or("backup.tar.gz", |n| n.to_str().unwrap_or("backup.tar.gz"))
        .to_string();

    println!(
        "  {} Created snapshot: {} ({})",
        "✔".green(),
        archive_name.bold(),
        format_bytes(size_bytes).cyan()
    );

    upload_to_cloud(&archive_path)?;
    let _ = fs::remove_file(&archive_path);

    rotate_old_backups();

    let elapsed = crate::runner::format_duration(start.elapsed());
    println!(
        "  {} Cloud backup completed successfully in {}\n",
        "✨".green().bold(),
        elapsed.bold()
    );

    send_notification(&archive_name, size_bytes, &elapsed);
    Ok(())
}

/// Lists existing cloud backup archives on Google Drive.
pub fn list_backups() -> Result<()> {
    println!(
        "\n  {} Cloud Backups on Google Drive:\n",
        "☁️".cyan().bold()
    );
    let output = Command::new("rclone")
        .args(["lsl", CLOUD_BACKUP_DEST])
        .output()
        .context("Failed to query Google Drive backups")?;

    if !output.status.success() {
        bail!(
            "Failed to list backups: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        println!("    No backups found in {}", CLOUD_BACKUP_DEST.dimmed());
    } else {
        for line in text.lines() {
            println!("    {}", line.trim().dimmed());
        }
    }

    let active = timer::is_timer_active();
    let timer_status = if active {
        "✔ active (03:00 AM daily)".green()
    } else {
        "✖ inactive".dimmed()
    };
    println!("\n  {} Nightly Timer: {}\n", "⏱".cyan(), timer_status);
    Ok(())
}

/// Deploys the nightly automated backup timer.
pub fn setup_schedule() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    timer::deploy_backup_timer(&home)
}

fn validate_prerequisites() -> Result<()> {
    if !rclone::is_remote_configured() {
        bail!("Google Drive remote 'gdrive:' is not configured. Run 'ryoiki rclone setup' first.");
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = Path::new(&home).join("jellyfin/config");
    if !config_dir.exists() {
        bail!(
            "Jellyfin config directory not found at {}",
            config_dir.display()
        );
    }
    Ok(())
}

fn current_timestamp() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
        libc::localtime_r(&raw const t, tm.as_mut_ptr());
        let tm = tm.assume_init();
        let mut buf = [0u8; 32];
        let len = libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c"%Y-%m-%d-%H%M%S".as_ptr(),
            &raw const tm,
        );
        String::from_utf8_lossy(&buf[..len]).to_string()
    }
}

fn create_archive(source: &Path) -> Result<(PathBuf, u64)> {
    let timestamp = current_timestamp();
    let filename = format!("jellyfin-backup-{timestamp}.tar.gz");
    let temp_archive = std::env::temp_dir().join(&filename);

    println!("  {} Compressing database & configuration...", "▶".cyan());
    let status = Command::new("tar")
        .args([
            "-czf",
            &temp_archive.to_string_lossy(),
            "--exclude=log",
            "--exclude=transcodes",
            "--exclude=cache",
            "-C",
            &source.to_string_lossy(),
            ".",
        ])
        .status()
        .context("Failed to execute tar compression")?;

    if !status.success() {
        bail!("Tar compression failed with exit code {:?}", status.code());
    }

    let size = fs::metadata(&temp_archive).map_or(0, |m| m.len());
    Ok((temp_archive, size))
}

fn upload_to_cloud(archive: &Path) -> Result<()> {
    println!("  {} Uploading snapshot to Google Drive...", "▶".cyan());
    let status = Command::new("rclone")
        .args([
            "copy",
            &archive.to_string_lossy(),
            CLOUD_BACKUP_DEST,
            "--drive-chunk-size=32M",
        ])
        .status()
        .context("Failed to execute rclone copy")?;

    if !status.success() {
        bail!("Rclone upload failed with exit code {:?}", status.code());
    }
    Ok(())
}

fn rotate_old_backups() {
    println!("  {} Enforcing 7-day retention rotation...", "▶".cyan());
    let _ = Command::new("rclone")
        .args(["delete", "--min-age", "7d", CLOUD_BACKUP_DEST])
        .status();
}

fn send_notification(name: &str, size_bytes: u64, elapsed: &str) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };
    let size_str = format_bytes(size_bytes);
    let fields = [
        ("📁 Snapshot:", name),
        ("💾 Archive Size:", size_str.as_str()),
        ("⏱ Duration:", elapsed),
        ("☁️ Destination:", CLOUD_BACKUP_DEST),
        ("🔄 Retention:", "7 Days Rolling"),
    ];

    let card = format_card("Cloud Backup", "💾 <b>BACKUP COMPLETE</b>", &fields);
    let _ = crate::notify::client::send_alert(&cfg.bot_token, &cfg.chat_id, &card);
}

fn format_bytes(bytes: u64) -> String {
    const BYTES_PER_MIB: u64 = 1024 * 1024;
    const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
    if bytes < BYTES_PER_GIB {
        let mib = bytes / BYTES_PER_MIB;
        let dec = (bytes % BYTES_PER_MIB) * 10 / BYTES_PER_MIB;
        format!("{mib}.{dec} MiB")
    } else {
        let gib = bytes / BYTES_PER_GIB;
        let dec = (bytes % BYTES_PER_GIB) * 10 / BYTES_PER_GIB;
        format!("{gib}.{dec} GiB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_mib() {
        assert_eq!(format_bytes(1024 * 1024 * 15), "15.0 MiB");
    }

    #[test]
    fn test_format_bytes_gib() {
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 3), "3.0 GiB");
    }
}
