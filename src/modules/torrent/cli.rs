use crate::modules::torrent::{api, dedup, queue, scheduler, seedr};
use crate::notify::TelegramConfig;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::time::Duration;

/// Structured result of adding a magnet through the dual pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct MagnetResult {
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<usize>,
}

/// Runs the full dual-pipeline for a magnet (dedup, qBittorrent, Seedr).
///
/// Shared by the CLI and the HTTP API endpoint so both paths stay in sync.
pub fn add_magnet_pipeline(target: &str, config: &TelegramConfig) -> Result<MagnetResult> {
    let target = target.trim();
    if target.is_empty() {
        anyhow::bail!("magnet is required");
    }

    match dedup::check_already_available(target) {
        dedup::Availability::Local { paths, title } => {
            return Ok(MagnetResult {
                status: "already_available".to_string(),
                message: format!("Already available locally: {title}"),
                hash: None,
                paths: paths.into_iter().map(|p| p.display().to_string()).collect(),
                queue_position: None,
            });
        }
        dedup::Availability::Cloud { pairs, title } => {
            let restored = dedup::restore_from_cloud(&pairs)?;
            return Ok(MagnetResult {
                status: "restored".to_string(),
                message: format!("Restored from Google Drive: {title}"),
                hash: None,
                paths: restored.into_iter().map(|p| p.display().to_string()).collect(),
                queue_position: None,
            });
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
    match scheduler::submit(&hash, target, &name, config) {
        scheduler::SubmitOutcome::Started => Ok(MagnetResult {
            status: "started".to_string(),
            message: "Magnet started in Seedr; qBittorrent added as fallback.".to_string(),
            hash: if hash.is_empty() { None } else { Some(hash) },
            paths: Vec::new(),
            queue_position: None,
        }),
        scheduler::SubmitOutcome::Duplicate => Ok(MagnetResult {
            status: "duplicate".to_string(),
            message: "Magnet already tracked in the Seedr queue.".to_string(),
            hash: if hash.is_empty() { None } else { Some(hash) },
            paths: Vec::new(),
            queue_position: None,
        }),
        scheduler::SubmitOutcome::Queued(pos) => Ok(MagnetResult {
            status: "queued".to_string(),
            message: format!("Seedr slot busy — queued at position #{pos}."),
            hash: if hash.is_empty() { None } else { Some(hash) },
            paths: Vec::new(),
            queue_position: Some(pos),
        }),
        scheduler::SubmitOutcome::FailedStart => Ok(MagnetResult {
            status: "fallback".to_string(),
            message: "Seedr cloud unavailable/rejected — downloading directly in qBittorrent.".to_string(),
            hash: if hash.is_empty() { None } else { Some(hash) },
            paths: Vec::new(),
            queue_position: None,
        }),
    }
}

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

    let result = add_magnet_pipeline(target, &config)?;
    match result.status.as_str() {
        "already_available" => {
            println!(
                "  {} Already available locally: {}",
                "✔".green().bold(),
                result.message.trim_start_matches("Already available locally: ").cyan()
            );
            for p in &result.paths {
                println!("  📁 {p}");
            }
        }
        "restored" => {
            println!(
                "  ☁️ Found in Google Drive archive: {}",
                result.message.trim_start_matches("Restored from Google Drive: ").cyan()
            );
            println!("  {} Restored from Drive:", "✔".green().bold());
            for p in &result.paths {
                println!("  📍 {p}");
            }
        }
        "started" => println!(
            "  {} Magnet started in Seedr; qBittorrent added as fallback.",
            "✔".green().bold()
        ),
        "duplicate" => println!("  {} Magnet already tracked in the Seedr queue.", "•".dimmed()),
        "queued" => println!(
            "  {} Seedr slot busy — queued at position #{}.",
            "⏳".cyan().bold(),
            result.queue_position.unwrap_or(0)
        ),
        "fallback" => println!(
            "  {} Seedr cloud unavailable/rejected — downloading directly in qBittorrent.",
            "✔".green().bold()
        ),
        _ => println!("  {}", result.message),
    }
    Ok(())
}