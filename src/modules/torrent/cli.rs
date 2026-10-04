use crate::modules::torrent::{api, dedup, seedr};
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
        if sec.is_empty() {
            println!("  🌱 No active Seedr downloads.");
        } else {
            println!("{sec}");
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
    seedr::spawn_seedr_download(target, config.api_port)?;
    println!(
        "  {} Queued magnet into qBittorrent and Seedr dual-pipeline.",
        "✔".green().bold()
    );
    Ok(())
}
