use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::{PruneCandidate, PruneOptions, COLD_ARCHIVE_DEST};
use crate::modules::media::disk;
use crate::notify::client::format_card;
use crate::notify::TelegramConfig;

pub fn process_candidates(
    candidates: &[PruneCandidate],
    target_bytes: u64,
    opts: PruneOptions,
    base: &Path,
) {
    let mut freed = 0u64;
    let mut processed_names = Vec::new();

    println!("  Identified Candidate Media Files:");
    for c in candidates {
        if target_bytes > 0 && freed >= target_bytes {
            break;
        }

        let tag = if c.is_backup {
            "🛡️ backup".magenta().bold()
        } else if c.is_watched {
            "👀 watched".green().bold()
        } else {
            "📦 cold".dimmed()
        };
        let size_str = disk::format_bytes(c.size_bytes);
        println!("    {} {} ({})", tag, c.name.bold(), size_str.cyan());

        if opts.dry_run {
            freed = freed.saturating_add(c.size_bytes);
            processed_names.push(c.name.clone());
            continue;
        }

        match archive_and_delete(c, base) {
            Ok(()) => {
                freed = freed.saturating_add(c.size_bytes);
                processed_names.push(c.name.clone());
            }
            Err(err_msg) => {
                eprintln!(
                    "  {} Failed to archive {}: {}",
                    "✘".red().bold(),
                    c.name,
                    err_msg.yellow()
                );
                send_prune_failure_alert(&c.name, &err_msg);
                break;
            }
        }
    }

    let freed_str = disk::format_bytes(freed);
    if freed > 0 {
        println!(
            "\n  {} Total space recovered: {}\n",
            "✔".green().bold(),
            freed_str.cyan().bold()
        );
        if !opts.dry_run && !processed_names.is_empty() {
            send_prune_alert(&processed_names, freed);
        }
    } else if !opts.dry_run {
        println!(
            "\n  {} No space recovered due to archival failure.\n",
            "✘".red().bold()
        );
    }
}

pub const BACKUP_RETENTION_DAYS: u32 = 7;
pub const CLOUD_BACKUP_DEST: &str = "gdrive:ryoiki-archive/backup_multi/";

/// Cleans up `backup_multi` safety copies older than 7 days from local storage and Google Drive.
pub fn cleanup_expired_backups(dry_run: bool) {
    cleanup_expired_local_backups(dry_run);
    cleanup_expired_cloud_backups(dry_run);
}

fn cleanup_expired_local_backups(dry_run: bool) {
    let backup_dir = crate::modules::media::organizer::pathing::get_jellyfin_backup_multi_dir();
    if !backup_dir.exists() {
        return;
    }

    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(
            u64::from(BACKUP_RETENTION_DAYS) * 24 * 3600,
        ))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut expired = Vec::new();
    collect_expired_files(&backup_dir, cutoff, &mut expired);

    for path in expired {
        if dry_run {
            println!(
                "  • [dry-run] Expired local backup would be deleted: {}",
                path.display()
            );
        } else if fs::remove_file(&path).is_ok() {
            println!(
                "  {} Cleaned up expired local backup (>7d): {}",
                "🧹".cyan(),
                path.display()
            );
            clean_empty_parents(&path, &backup_dir);
        }
    }
}

fn collect_expired_files(
    dir: &Path,
    cutoff: std::time::SystemTime,
    out: &mut Vec<std::path::PathBuf>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_expired_files(&path, cutoff, out);
            } else if path.is_file() {
                if let Ok(meta) = fs::metadata(&path) {
                    if let Ok(mtime) = meta.modified() {
                        if mtime <= cutoff {
                            out.push(path);
                        }
                    }
                }
            }
        }
    }
}

fn cleanup_expired_cloud_backups(dry_run: bool) {
    let age_flag = format!("{BACKUP_RETENTION_DAYS}d");
    if dry_run {
        println!("  • [dry-run] Checking expired cloud backups (>7d) on Google Drive");
        return;
    }

    let _ = Command::new("rclone")
        .args([
            "delete",
            CLOUD_BACKUP_DEST,
            "--min-age",
            &age_flag,
            "--drive-use-trash=true",
        ])
        .output();

    let _ = Command::new("rclone")
        .args(["rmdirs", CLOUD_BACKUP_DEST, "--leave-root"])
        .output();
}

fn archive_and_delete(c: &PruneCandidate, base: &Path) -> Result<(), String> {
    let (dest_prefix, base_dir) = if c.is_backup {
        (
            "gdrive:ryoiki-archive/backup_multi/",
            crate::modules::media::organizer::pathing::get_jellyfin_backup_multi_dir(),
        )
    } else {
        (COLD_ARCHIVE_DEST, base.to_path_buf())
    };

    let rel_parent = c
        .path
        .parent()
        .and_then(|p| p.strip_prefix(&base_dir).ok())
        .map_or(String::new(), |p| p.to_string_lossy().to_string());

    let dest = format!("{dest_prefix}{rel_parent}/");
    let output = Command::new("rclone")
        .args(["copy", &c.path.to_string_lossy(), &dest])
        .output()
        .map_err(|e| format!("Failed to spawn rclone: {e}"))?;

    if !output.status.success() {
        let (detail, action) = extract_error_detail(&output.stderr);
        return Err(format!("{detail} ({action})"));
    }

    if let Err(e) = fs::remove_file(&c.path) {
        return Err(format!("Local deletion failed: {e}"));
    }

    clean_empty_parents(&c.path, &base_dir);
    Ok(())
}

fn clean_empty_parents(file: &Path, base: &Path) {
    let mut curr = file.parent();
    while let Some(dir) = curr {
        if dir == base || !dir.starts_with(base) {
            break;
        }
        if fs::read_dir(dir).is_ok_and(|mut i| i.next().is_none()) {
            let _ = fs::remove_dir(dir);
            curr = dir.parent();
        } else {
            break;
        }
    }
}

pub fn send_prune_alert(names: &[String], freed_bytes: u64) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };
    let freed_str = disk::format_bytes(freed_bytes);
    let count_str = names.len().to_string();
    let preview = names.iter().take(3).cloned().collect::<Vec<_>>().join(", ");

    let fields = [
        ("🧹 Recovered Space:", freed_str.as_str()),
        ("🎬 Items Evicted:", count_str.as_str()),
        ("☁️ Cold Archive:", COLD_ARCHIVE_DEST),
        ("📁 Sample Titles:", preview.as_str()),
    ];

    let card = format_card("Media Pruner", "🧹 <b>LOCAL SSD CLEANED</b>", &fields);
    let _ = crate::notify::client::send_alert(&cfg.bot_token, &cfg.chat_id, &card);
}

pub fn send_prune_failure_alert(item_name: &str, error_detail: &str) {
    let Ok(cfg) = TelegramConfig::load() else {
        return;
    };

    let fields = [
        ("⚠️ Failure:", "Cold storage archival failed"),
        ("📁 Target Item:", item_name),
        ("❌ Error:", error_detail),
        ("☁️ Cold Archive:", COLD_ARCHIVE_DEST),
    ];

    let card = format_card("Media Pruner", "❌ <b>COLD ARCHIVE FAILED</b>", &fields);
    let _ = crate::notify::client::send_alert(&cfg.bot_token, &cfg.chat_id, &card);
}

pub fn extract_error_detail(stderr: &[u8]) -> (String, String) {
    let text = String::from_utf8_lossy(stderr);
    let mut detail = "Cold storage error".to_string();
    let mut action = "Check network or run 'rclone lsd gdrive:'".to_string();

    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.contains("invalid_grant") || trimmed.contains("token expired") {
            detail = "Google Drive token expired".to_string();
            action = "Run 'rclone config reconnect gdrive:'".to_string();
            return (detail, action);
        }
        if trimmed.contains("couldn't find root directory")
            || trimmed.contains("Failed to create file system")
        {
            detail = "Could not connect to Google Drive".to_string();
            action = "Run 'rclone config reconnect gdrive:'".to_string();
            return (detail, action);
        }
        if trimmed.contains("ERROR") {
            detail = trimmed.chars().take(80).collect();
            break;
        }
    }

    (detail, action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_error_detail_token_expired() {
        let err = b"ERROR+4: couldn't fetch token: invalid_grant: maybe token expired?";
        let (detail, action) = extract_error_detail(err);
        assert_eq!(detail, "Google Drive token expired");
        assert_eq!(action, "Run 'rclone config reconnect gdrive:'");
    }

    #[test]
    fn test_extract_error_detail_generic() {
        let err = b"ERROR: something else went wrong";
        let (detail, _) = extract_error_detail(err);
        assert!(detail.contains("ERROR: something else"));
    }
}
