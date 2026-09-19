pub mod execute;
pub mod scan;

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::disk::{self, DiskUsage};

pub const COLD_ARCHIVE_DEST: &str = "gdrive:ryoiki-archive/media/";

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

    let mut candidates = scan::scan_media_candidates(&media_dir)?;
    scan::tag_watched_status(&mut candidates);
    sort_candidates(&mut candidates);

    if candidates.is_empty() {
        println!(
            "  {} No media candidates found in {}\n",
            "ℹ".cyan(),
            media_dir.display()
        );
        return Ok(());
    }

    execute::process_candidates(&candidates, bytes_to_free, opts, &media_dir);
    Ok(())
}

/// Dispatches media pruning directly from a Telegram bot command.
pub fn handle_bot_prune() -> Result<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let media_dir = Path::new(&home).join("jellyfin/media");
    let usage = disk::get_disk_usage(&media_dir)?;

    let opts = PruneOptions {
        threshold_pct: 80,
        target_pct: 70,
        dry_run: false,
    };

    let bytes_to_free = disk::compute_bytes_to_free(usage, opts.target_pct);
    let used_str = disk::format_bytes(usage.used_bytes);
    let total_str = disk::format_bytes(usage.total_bytes);

    if bytes_to_free == 0 {
        return Ok(format!(
            "🌊 <b>領域 RYOIKI • Media Pruner</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            💾 <b>SSD Usage:</b> {}% ({} / {})\n\
            🛡️ <b>Status:</b> Safe (<80% threshold). No pruning required.\n\
            ━━━━━━━━━━━━━━━━━━━━━━━",
            usage.used_pct, used_str, total_str
        ));
    }

    run_prune(opts)?;
    let new_usage = disk::get_disk_usage(&media_dir).unwrap_or(usage);
    let new_used_str = disk::format_bytes(new_usage.used_bytes);

    Ok(format!(
        "🌊 <b>領域 RYOIKI • Media Pruner</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🧹 <b>Pruning Complete!</b>\n\
        💾 <b>Previous SSD:</b> {}% ({})\n\
        💾 <b>Current SSD:</b> {}% ({})\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        usage.used_pct, used_str, new_usage.used_pct, new_used_str
    ))
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

pub fn sort_candidates(candidates: &mut [PruneCandidate]) {
    candidates.sort_by(|a, b| {
        b.is_watched
            .cmp(&a.is_watched)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| a.modified.cmp(&b.modified))
    });
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
