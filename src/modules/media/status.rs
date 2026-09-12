use colored::Colorize;
use std::path::Path;
use std::process::Command;

use super::disk::{self, DiskUsage};

/// Prints current storage status across local SSD, Google Drive, and automation timers.
pub fn show_storage_status() {
    println!(
        "\n  {} {}",
        "📊".cyan(),
        "Storage Utilization Overview".bold()
    );
    println!("  {}\n", "─".repeat(40).dimmed());

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let media_dir = Path::new(&home).join("jellyfin/media");

    if let Ok(usage) = disk::get_disk_usage(&media_dir) {
        print_local_usage(&media_dir, usage);
    }

    print_gdrive_usage();
    print_automation_status();
}

fn print_local_usage(media_dir: &Path, usage: DiskUsage) {
    let total = disk::format_bytes(usage.total_bytes);
    let used = disk::format_bytes(usage.used_bytes);
    let free = disk::format_bytes(usage.free_bytes);
    let badge = if usage.used_pct >= 80 {
        "⚠ High Watermark Exceeded".red().bold()
    } else {
        "✔ Healthy".green()
    };
    println!("  • Local NVMe/SSD ({})", media_dir.display());
    println!(
        "    Used: {:<12} Free: {:<12} Total: {}",
        used.cyan(),
        free.green(),
        total.bold()
    );
    println!("    Usage: {}% [{badge}]\n", usage.used_pct);
}

fn print_gdrive_usage() {
    println!("  • Google Drive Cloud Storage (gdrive:)");
    if let Ok(out) = Command::new("rclone").args(["about", "gdrive:"]).output() {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                println!("    {}", line.trim().dimmed());
            }
        }
    }
    println!();
}

fn print_automation_status() {
    let sync_st = if super::sync::is_timer_active() {
        "✔ active (daily, 30m post-boot)".green()
    } else {
        "✖ inactive".dimmed()
    };
    let prune_st = if super::prune_timer::is_timer_active() {
        "✔ active (every 6h)".green()
    } else {
        "✖ inactive".dimmed()
    };
    println!("  • Automation Timers:");
    println!("    Daily Media Sync: {sync_st}");
    println!("    Storage Pruner:   {prune_st}\n");
}
