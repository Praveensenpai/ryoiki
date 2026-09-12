use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use super::disk::{self, DiskUsage};
use crate::notify::client::format_card;
use crate::notify::TelegramConfig;

const COLD_ARCHIVE_DEST: &str = "gdrive:ryoiki-archive/media/";

/// Runtime configuration options for media pruning.
#[derive(Debug, Clone, Copy)]
pub struct PruneOptions {
    pub threshold_pct: u8,
    pub target_pct: u8,
    pub dry_run: bool,
}

/// A media file candidate evaluated for potential archival and eviction.
#[derive(Debug, Clone)]
pub struct PruneCandidate {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub modified: SystemTime,
    pub is_watched: bool,
}

/// Executes the storage pruner against local SSD usage.
pub fn run_prune(opts: PruneOptions) -> Result<()> {
    println!(
        "\n  {} {}",
        "🧹".cyan(),
        "Smart Local SSD Media Pruner".bold()
    );
    println!("  {}\n", "─".repeat(40).dimmed());

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let media_dir = Path::new(&home).join("jellyfin/media");
    let usage = disk::get_disk_usage(&media_dir)?;

    print_disk_header(usage, opts);

    let bytes_to_free = disk::compute_bytes_to_free(usage, opts.target_pct);
    if bytes_to_free == 0 && !opts.dry_run {
        println!(
            "  {} Local SSD is within safe limit. No pruning required.\n",
            "✔".green().bold()
        );
        return Ok(());
    }

    let mut candidates = scan_media_candidates(&media_dir)?;
    tag_watched_status(&mut candidates);
    sort_candidates(&mut candidates);

    if candidates.is_empty() {
        println!(
            "  {} No media candidates found in {}\n",
            "ℹ".cyan(),
            media_dir.display()
        );
        return Ok(());
    }

    process_candidates(&candidates, bytes_to_free, opts, &media_dir)?;
    Ok(())
}

/// Dispatches media pruning directly from a Telegram bot command.
pub fn handle_bot_prune() -> Result<String> {
    let opts = PruneOptions {
        threshold_pct: 80,
        target_pct: 70,
        dry_run: false,
    };
    run_prune(opts)?;
    Ok("🧹 <b>Media Pruner executed</b>\nChecked 80% threshold against local SSD.".to_string())
}

fn print_disk_header(usage: DiskUsage, opts: PruneOptions) {
    let used_str = disk::format_bytes(usage.used_bytes);
    let total_str = disk::format_bytes(usage.total_bytes);
    println!(
        "  Current SSD Usage: {}% ({} / {})",
        usage.used_pct.to_string().bold(),
        used_str.cyan(),
        total_str
    );
    println!(
        "  High-water trigger: {}% • Target low-water: {}%",
        opts.threshold_pct.to_string().yellow(),
        opts.target_pct.to_string().green()
    );
    if opts.dry_run {
        println!(
            "  {} Dry-run simulation active (no files deleted)",
            "•".dimmed()
        );
    }
    println!();
}

fn scan_media_candidates(base: &Path) -> Result<Vec<PruneCandidate>> {
    let mut list = Vec::new();
    let video_exts = ["mkv", "mp4", "avi", "mov", "m4v", "ts"];

    for category in &["movies", "shows"] {
        let cat_dir = base.join(category);
        if !cat_dir.exists() {
            continue;
        }
        collect_dir_candidates(&cat_dir, &video_exts, &mut list)?;
    }
    Ok(list)
}

fn collect_dir_candidates(dir: &Path, exts: &[&str], list: &mut Vec<PruneCandidate>) -> Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_dir_candidates(&path, exts, list)?;
            } else if path.is_file() {
                check_and_add_candidate(&path, exts, list);
            }
        }
    }
    Ok(())
}

fn check_and_add_candidate(path: &Path, exts: &[&str], list: &mut Vec<PruneCandidate>) {
    let matches_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| exts.contains(&ext.to_ascii_lowercase().as_str()));

    if !matches_ext {
        return;
    }

    if let Ok(meta) = fs::metadata(path) {
        let size_bytes = meta.len();
        let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
        let name = path
            .file_name()
            .map_or("video".to_string(), |n| n.to_string_lossy().to_string());

        list.push(PruneCandidate {
            path: path.to_path_buf(),
            name,
            size_bytes,
            modified,
            is_watched: false,
        });
    }
}

fn tag_watched_status(candidates: &mut [PruneCandidate]) {
    let url = "http://localhost:8096/Items?Filters=IsPlayed&Recursive=true&Fields=Path";
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .ok();
    let resp = client.and_then(|c| c.get(url).send().ok());
    let json = resp.and_then(|r| r.json::<serde_json::Value>().ok());
    let Some(items) = json
        .as_ref()
        .and_then(|j| j.get("Items"))
        .and_then(|i| i.as_array())
    else {
        return;
    };
    for item in items {
        if let Some(p) = item.get("Path").and_then(|p| p.as_str()) {
            for c in candidates.iter_mut() {
                if c.path.to_string_lossy().contains(p) {
                    c.is_watched = true;
                }
            }
        }
    }
}

fn sort_candidates(candidates: &mut [PruneCandidate]) {
    candidates.sort_by(|a, b| {
        b.is_watched
            .cmp(&a.is_watched)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| a.modified.cmp(&b.modified))
    });
}

fn process_candidates(
    candidates: &[PruneCandidate],
    target_bytes: u64,
    opts: PruneOptions,
    base: &Path,
) -> Result<()> {
    let mut freed = 0u64;
    let mut processed_names = Vec::new();

    println!("  Identified Candidate Media Files:");
    for c in candidates {
        if target_bytes > 0 && freed >= target_bytes {
            break;
        }

        let tag = if c.is_watched {
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

        archive_and_delete(c, base)?;
        freed = freed.saturating_add(c.size_bytes);
        processed_names.push(c.name.clone());
    }

    let freed_str = disk::format_bytes(freed);
    println!(
        "\n  {} Total space recovered: {}\n",
        "✔".green().bold(),
        freed_str.cyan().bold()
    );

    if !opts.dry_run && !processed_names.is_empty() {
        send_prune_alert(&processed_names, freed);
    }
    Ok(())
}

fn archive_and_delete(c: &PruneCandidate, base: &Path) -> Result<()> {
    let rel_parent = c
        .path
        .parent()
        .and_then(|p| p.strip_prefix(base).ok())
        .map_or(String::new(), |p| p.to_string_lossy().to_string());

    let dest = format!("{COLD_ARCHIVE_DEST}{rel_parent}/");
    let status = Command::new("rclone")
        .args(["copy", &c.path.to_string_lossy(), &dest])
        .status()
        .context("Failed to copy to cold storage")?;

    if status.success() {
        let _ = fs::remove_file(&c.path);
        clean_empty_parents(&c.path, base);
    }
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

fn send_prune_alert(names: &[String], freed_bytes: u64) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_candidates_prefers_watched() {
        let now = SystemTime::now();
        let mut list = vec![
            PruneCandidate {
                path: PathBuf::from("a.mkv"),
                name: "Unwatched Large".into(),
                size_bytes: 5000,
                modified: now,
                is_watched: false,
            },
            PruneCandidate {
                path: PathBuf::from("b.mkv"),
                name: "Watched Small".into(),
                size_bytes: 1000,
                modified: now,
                is_watched: true,
            },
        ];
        sort_candidates(&mut list);
        assert_eq!(list[0].name, "Watched Small");
    }

    #[test]
    fn test_sort_candidates_falls_back_to_size() {
        let now = SystemTime::now();
        let mut list = vec![
            PruneCandidate {
                path: PathBuf::from("a.mkv"),
                name: "Small".into(),
                size_bytes: 1000,
                modified: now,
                is_watched: false,
            },
            PruneCandidate {
                path: PathBuf::from("b.mkv"),
                name: "Large".into(),
                size_bytes: 5000,
                modified: now,
                is_watched: false,
            },
        ];
        sort_candidates(&mut list);
        assert_eq!(list[0].name, "Large");
    }
}
