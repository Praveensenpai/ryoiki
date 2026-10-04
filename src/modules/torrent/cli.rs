use crate::modules::torrent::{api, dedup, queue, scheduler, seedr};
use crate::notify::TelegramConfig;
use anyhow::Result;
use colored::Colorize;
use std::time::Duration;

/// Handles the `ryoiki seedr <target>` CLI command.
pub fn handle_seedr_cli(target: &str) -> Result<()> {
    let config = TelegramConfig::load()?;
    if target.is_empty() || target == "status" {
        let tasks = seedr::get_active_seedr_tasks();
        let sec = seedr::format_seedr_tasks_section(&tasks);
        let pending = queue::load().pending_count();
        if sec.is_empty() && pending == 0 {
            println!("  🌱 No active Seedr downloads.");
        } else {
            if !sec.is_empty() {
                println!("{sec}");
            }
            if pending > 0 {
                println!("  ⏳ {pending} magnet(s) waiting in the Seedr queue.");
            }
        }
        return Ok(());
    }

    match dedup::check_already_available(target) {
        dedup::Availability::Local { paths, title } => {
            println!(
                "  {} Already available locally: {}",
                "✔".green().bold(),
                title.cyan()
            );
            for p in &paths {
                println!("  📁 {}", p.display());
            }
            return Ok(());
        }
        dedup::Availability::Cloud { pairs, title } => {
            println!("  ☁️ Found in Google Drive archive: {}", title.cyan());
            let restored = dedup::restore_from_cloud(&pairs)?;
            println!("  {} Restored from Drive:", "✔".green().bold());
            for p in &restored {
                println!("  📍 {}", p.display());
            }
            return Ok(());
        }
        dedup::Availability::NotAvailable { .. } => {}
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let _ = api::add_magnet(&client, &config.qbittorrent_url, target);

    let hash = seedr::extract_btih_hash(target).unwrap_or_default();
    let name = dedup::parse_magnet(target)
        .1
        .unwrap_or_else(|| hash.clone());
    match scheduler::submit(&hash, target, &name, &config) {
        scheduler::SubmitOutcome::Started => println!(
            "  {} Magnet started in Seedr; qBittorrent added as fallback.",
            "✔".green().bold()
        ),
        scheduler::SubmitOutcome::Duplicate => {
            println!("  {} Magnet already tracked in the Seedr queue.", "•".dimmed());
        }
        scheduler::SubmitOutcome::Queued(pos) => println!(
            "  {} Seedr slot busy — queued at position #{pos}. Downloading in qBittorrent meanwhile.",
            "⏳".cyan().bold()
        ),
    }
    Ok(())
}
