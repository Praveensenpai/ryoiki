use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{notify, MediaItem, TransferDirection};
use crate::modules::media::disk;
use crate::runner::format_duration;

pub fn confirm_transfer(dir: TransferDirection, items: &[MediaItem]) -> bool {
    let count = items.len();
    let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
    let size_str = disk::format_bytes(total_bytes);

    let (verb, arrow, note) = match dir {
        TransferDirection::Push => (
            "MOVE".yellow().bold(),
            "Local SSD → Google Drive (Free SSD space)",
            "Local files will be deleted after successful cloud upload.",
        ),
        TransferDirection::Pull => (
            "PULL".cyan().bold(),
            "Google Drive → Local SSD (Zero-lag streaming)",
            "Files will be downloaded to ~/jellyfin/media/ for direct local playback.",
        ),
    };

    println!(
        "\n  {} {}",
        "📦".cyan(),
        "Media Transfer Confirmation".bold()
    );
    println!("  {}", "─".repeat(45).dimmed());
    println!("  • Action:    {verb} {count} item(s) ({size_str})");
    println!("  • Direction: {arrow}");
    println!("  • Note:      {note}\n");

    println!("  Selected items:");
    for (idx, item) in items.iter().take(8).enumerate() {
        let item_size = disk::format_bytes(item.size_bytes);
        println!(
            "    {}. [{}] {} ({})",
            idx + 1,
            item.category.as_str().dimmed(),
            item.title.bold(),
            item_size.cyan()
        );
    }
    if count > 8 {
        println!("    ... and {} more items", count - 8);
    }

    print!("\n  Proceed with transfer? [y/N]: ");
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    answer.trim().eq_ignore_ascii_case("y")
}

pub fn execute_transfer(dir: TransferDirection, items: &[MediaItem], home: &Path) -> Result<()> {
    let batch_start = std::time::Instant::now();
    let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
    let count = items.len();

    println!(
        "\n  {} Initiating transfer of {count} item(s) ({})",
        "▶".cyan().bold(),
        disk::format_bytes(total_bytes).cyan().bold()
    );
    notify::send_batch_initiated_notification(dir, items, total_bytes);

    let mut timings: Vec<(&MediaItem, Duration)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let item_start = std::time::Instant::now();
        let item_size_str = disk::format_bytes(item.size_bytes);
        println!(
            "\n  [{}/{}] {} {} ({item_size_str})...",
            idx + 1,
            count,
            if dir == TransferDirection::Push {
                "Uploading".yellow().bold()
            } else {
                "Downloading".cyan().bold()
            },
            item.title.bold()
        );

        match dir {
            TransferDirection::Push => execute_single_push(item, home)?,
            TransferDirection::Pull => execute_single_pull(item, home)?,
        }

        let item_dur = item_start.elapsed();
        let rate_str = notify::format_transfer_rate(item.size_bytes, item_dur);
        let dur_str = format_duration(item_dur);

        println!(
            "  {} [{}/{}] Transferred in {} ({})",
            "✔".green().bold(),
            idx + 1,
            count,
            dur_str.cyan().bold(),
            rate_str.yellow()
        );

        timings.push((item, item_dur));
        notify::send_item_notification(dir, item, idx + 1, count, item_dur);
    }

    let total_dur = batch_start.elapsed();
    refresh_jellyfin();
    notify::send_batch_completed_notification(dir, &timings, total_bytes, total_dur);

    print_cli_transfer_summary(dir, &timings, total_bytes, total_dur);
    Ok(())
}

fn execute_single_push(item: &MediaItem, home: &Path) -> Result<()> {
    let Some(local_path) = &item.local_path else {
        return Ok(());
    };
    let remote_dest = format!("gdrive:{}", item.remote_path);

    let status = Command::new("rclone")
        .args([
            "move",
            "--progress",
            &local_path.to_string_lossy(),
            &remote_dest,
        ])
        .status()
        .context("Failed to run rclone move")?;

    if status.success() {
        let base = home.join("jellyfin/media");
        clean_empty_parents(local_path, &base);
    }
    Ok(())
}

fn execute_single_pull(item: &MediaItem, home: &Path) -> Result<()> {
    let rel = item
        .remote_path
        .strip_prefix("media/")
        .unwrap_or(&item.remote_path);

    let rel_norm = if let Some(m) = rel.strip_prefix("movie/") {
        format!("movies/{m}")
    } else {
        rel.to_string()
    };

    let local_dest = home.join("jellyfin/media").join(rel_norm);
    let remote_src = format!("gdrive:{}", item.remote_path);

    let status = Command::new("rclone")
        .args([
            "copy",
            "--progress",
            &remote_src,
            &local_dest.to_string_lossy(),
        ])
        .status()
        .context("Failed to run rclone copy")?;

    if !status.success() {
        eprintln!("  ✖ Failed to download {}", item.title);
    }
    Ok(())
}

fn clean_empty_parents(path: &Path, base: &Path) {
    if path.is_dir() && fs::read_dir(path).is_ok_and(|mut i| i.next().is_none()) {
        let _ = fs::remove_dir(path);
    }
    let mut curr = path.parent();
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

fn refresh_jellyfin() {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return;
    };
    let _ = client.post("http://localhost:8096/Library/Refresh").send();
}

fn print_cli_transfer_summary(
    dir: TransferDirection,
    timings: &[(&MediaItem, Duration)],
    total_bytes: u64,
    total_dur: Duration,
) {
    let count = timings.len();
    let total_size_str = disk::format_bytes(total_bytes);
    let total_dur_str = format_duration(total_dur);
    let avg_rate = notify::format_transfer_rate(total_bytes, total_dur);

    println!("\n  {}", "─".repeat(56).dimmed());
    let title = match dir {
        TransferDirection::Push => format!("✨ Offloaded {count} item(s) to Google Drive"),
        TransferDirection::Pull => format!("✨ Restored {count} item(s) to Local SSD"),
    };
    println!("  {}", title.green().bold());
    println!("  {}", "─".repeat(56).dimmed());
    println!(
        "  • Total Volume:  {} ({})",
        total_size_str.cyan().bold(),
        avg_rate.yellow()
    );
    println!("  • Total Elapsed: {}", total_dur_str.cyan().bold());
    println!();
    println!("  Transferred Breakdown:");
    for (idx, (item, dur)) in timings.iter().enumerate() {
        let sz = disk::format_bytes(item.size_bytes);
        let d = format_duration(*dur);
        let r = notify::format_transfer_rate(item.size_bytes, *dur);
        println!(
            "    {:2}. {:<36} {} in {} ({})",
            idx + 1,
            crate::modules::media::interactive::ui::truncate_str(&item.title, 36),
            sz.cyan(),
            d.bold(),
            r.dimmed()
        );
    }
    println!("  {}\n", "─".repeat(56).dimmed());
}
