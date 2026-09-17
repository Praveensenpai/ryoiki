use anyhow::Result;
use colored::Colorize;
use reqwest::blocking::Client;
use std::path::Path;

use super::{organize_path, organize_torrent};
use crate::modules::media::OrganizeResult;
use crate::modules::torrent::api::{self, TorrentInfo};
use crate::runner::Runner;

pub fn run_organize_cli(target: &Path, dry_run: bool) -> Result<()> {
    println!(
        "\n  {} Scanning {} for media to organize...",
        "🎬".cyan(),
        target.display().to_string().bold()
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let api_key = crate::modules::media::config::get_or_prompt_gemini_key(!dry_run);

    let qb_url = crate::notify::TelegramConfig::load().map_or_else(
        |_| "http://localhost:6881".to_string(),
        |c| c.qbittorrent_url,
    );

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default_torrents = Path::new(&home).join("torrents");

    if target == default_torrents {
        if let Ok(torrents) = api::get_torrents(&client, &qb_url, None) {
            return process_qbittorrent_organize(
                &client,
                &qb_url,
                api_key.as_deref(),
                &torrents,
                dry_run,
            );
        }
    }

    let results = organize_path(target, &client, api_key.as_deref(), dry_run)?;
    let cleared = cleanup_matching_torrents(&client, &qb_url, &results);
    print_organize_summary(&results, cleared);

    Ok(())
}

fn process_qbittorrent_organize(
    client: &Client,
    qb_url: &str,
    api_key: Option<&str>,
    torrents: &[TorrentInfo],
    dry_run: bool,
) -> Result<()> {
    let (completed, incomplete): (Vec<_>, Vec<_>) = torrents.iter().partition(|t| t.is_completed());

    if completed.is_empty() {
        if !incomplete.is_empty() {
            println!(
                "  {} No completed torrents found in qBittorrent.\n    {} active torrent(s) currently downloading — skipping organize to protect active files.\n",
                "ℹ".cyan().bold(),
                incomplete.len()
            );
            return Ok(());
        }
        println!(
            "  {} No torrents found in qBittorrent to organize.",
            "•".dimmed()
        );
        return Ok(());
    }

    println!(
        "  {} Found {} completed torrent(s) in qBittorrent to organize:\n",
        "✔".green().bold(),
        completed.len()
    );

    let mut all_results = Vec::new();
    let mut cleared = 0;

    for t in &completed {
        println!("  • Ingesting completed torrent: {}", t.name.bold());
        let res = organize_torrent(t, client, api_key, dry_run)?;
        if !res.is_empty()
            && !dry_run
            && api::delete_torrent(client, qb_url, &t.hash, false).is_ok()
        {
            cleared += 1;
        }
        all_results.extend(res);
    }

    print_organize_summary(&all_results, cleared);
    Ok(())
}

fn print_organize_summary(results: &[OrganizeResult], cleared: usize) {
    if results.is_empty() {
        println!("  {} No new video files found to organize.", "•".dimmed());
    } else {
        println!(
            "\n  {} Successfully organized {} item(s):\n",
            "✔".green().bold(),
            results.len()
        );
        for res in results {
            println!(
                "  • {} ({})\n    {} -> {}",
                res.media_info.clean_name.bold(),
                res.media_info.engine,
                res.source_path.display().to_string().dimmed(),
                res.dest_path.display().to_string().cyan()
            );
        }
        println!();
    }

    if cleared > 0 {
        println!(
            "  {} Removed {} completed torrent(s) from qBittorrent history",
            "🗑".green().bold(),
            cleared
        );
    }
}

pub fn cleanup_matching_torrents(
    client: &Client,
    base_url: &str,
    organized_files: &[OrganizeResult],
) -> usize {
    let Ok(torrents) = api::get_torrents(client, base_url, None) else {
        return 0;
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default_dl = Path::new(&home).join("torrents");
    let mut cleared_count = 0;

    for t in &torrents {
        if !t.is_completed() {
            continue;
        }

        let was_organized = organized_files
            .iter()
            .any(|r| r.source_path.to_string_lossy().contains(&t.name));

        let source_missing = !default_dl.join(&t.name).exists();

        if (was_organized || source_missing)
            && api::delete_torrent(client, base_url, &t.hash, false).is_ok()
        {
            cleared_count += 1;
        }
    }

    cleared_count
}

pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let target = Path::new(&home).join("torrents");

    if runner.dry_run {
        println!(
            "  • [dry-run] Scan and organize media in {}",
            target.display()
        );
        return Ok(());
    }

    let _ = crate::modules::media::config::get_or_prompt_gemini_key(!non_interactive);
    if target.exists() {
        run_organize_cli(&target, false)?;
    }
    Ok(())
}
