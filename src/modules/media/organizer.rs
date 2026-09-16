pub mod pathing;

pub use pathing::{calculate_dest_dir, is_video_file, perform_move, resolve_unique_dest_path};

use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};

use super::ai::classify_media_ai;
use super::heuristic::classify_media_heuristic;
use super::{MediaType, OrganizeResult};
use crate::modules::torrent::api::{self, TorrentInfo};
use crate::runner::Runner;

pub fn organize_file(
    file_path: &Path,
    client: &Client,
    api_key: Option<&str>,
    dry_run: bool,
) -> Result<OrganizeResult> {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid filename")?;

    let probe = super::probe::probe_media_file(file_path);

    let mut media_info = match api_key {
        Some(key) if !key.is_empty() => classify_media_ai(client, key, file_name, probe.as_ref())
            .unwrap_or_else(|_| classify_media_heuristic(file_name)),
        _ => classify_media_heuristic(file_name),
    };

    if media_info.language.is_none() {
        if let Some(p) = &probe {
            if let Some(primary) = &p.primary_language {
                media_info.language = Some(primary.clone());
                media_info.clean_name =
                    super::ai::ensure_language_in_clean_name(&media_info.clean_name, primary);
            }
        }
    }

    if media_info.media_type != MediaType::Anime {
        if let Some(lang) = &media_info.language {
            if lang.eq_ignore_ascii_case("japanese") {
                media_info.media_type = MediaType::Anime;
            }
        }
    }

    let dest_dir = calculate_dest_dir(&media_info);
    let dest_path = resolve_unique_dest_path(file_path, &dest_dir, &media_info, dry_run);

    if dry_run {
        println!(
            "  • [dry-run] {} -> {}",
            file_path.display(),
            dest_path.display().to_string().cyan()
        );
        println!("    Engine: {}", media_info.engine);
        return Ok(OrganizeResult {
            source_path: file_path.to_path_buf(),
            dest_path,
            media_info,
        });
    }

    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("Failed to create destination dir: {}", dest_dir.display()))?;

    perform_move(file_path, &dest_path)?;

    super::audio::strip_audio_auto(&dest_path);

    Ok(OrganizeResult {
        source_path: file_path.to_path_buf(),
        dest_path,
        media_info,
    })
}

pub fn organize_path(
    target: &Path,
    client: &Client,
    api_key: Option<&str>,
    dry_run: bool,
) -> Result<Vec<OrganizeResult>> {
    let mut results = Vec::new();

    if target.is_file() {
        if is_video_file(target) {
            let res = organize_file(target, client, api_key, dry_run)?;
            results.push(res);
            if !dry_run {
                crate::modules::jellyfin::api::refresh_library_async();
            }
        }
        return Ok(results);
    }

    if !target.is_dir() {
        return Ok(results);
    }

    let mut video_files = Vec::new();
    find_videos_recursive(target, &mut video_files)?;

    for vf in video_files {
        match organize_file(&vf, client, api_key, dry_run) {
            Ok(res) => results.push(res),
            Err(e) => eprintln!("  ⚠️ Error organizing {}: {e}", vf.display()),
        }
    }

    if !results.is_empty() && !dry_run {
        crate::modules::jellyfin::api::refresh_library_async();
    }

    Ok(results)
}

pub fn find_videos_recursive(dir: &Path, list: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("Cannot read {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.eq_ignore_ascii_case("incomplete") || name.starts_with('.') {
                continue;
            }
            find_videos_recursive(&path, list)?;
        } else if is_video_file(&path) {
            list.push(path);
        }
    }
    Ok(())
}

pub fn resolve_torrent_source(torrent: &TorrentInfo, default_dl: &Path) -> PathBuf {
    if let Some(content_path) = &torrent.content_path {
        let rel = content_path
            .trim_start_matches("/downloads/")
            .trim_start_matches('/');
        let p = default_dl.join(rel);
        if p.exists() {
            return p;
        }
    }

    if let Some(save_path) = &torrent.save_path {
        let rel = save_path
            .trim_start_matches("/downloads/")
            .trim_start_matches('/');
        let p = default_dl.join(rel).join(&torrent.name);
        if p.exists() {
            return p;
        }
    }

    default_dl.join(&torrent.name)
}

pub fn organize_torrent(
    torrent: &TorrentInfo,
    client: &Client,
    api_key: Option<&str>,
    dry_run: bool,
) -> Result<Vec<OrganizeResult>> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default_dl = Path::new(&home).join("torrents");
    let source = resolve_torrent_source(torrent, &default_dl);

    if !source.exists() {
        return Ok(Vec::new());
    }

    organize_path(&source, client, api_key, dry_run)
}

pub fn organize_completed_torrent(
    client: &Client,
    torrent: &TorrentInfo,
    api_key: Option<&str>,
) -> Result<Option<OrganizeResult>> {
    let results = organize_torrent(torrent, client, api_key, false)?;
    Ok(results.into_iter().next())
}

pub fn run_organize_cli(target: &Path, dry_run: bool) -> Result<()> {
    println!(
        "\n  {} Scanning {} for media to organize...",
        "🎬".cyan(),
        target.display().to_string().bold()
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let api_key = super::config::get_or_prompt_gemini_key(!dry_run);

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

    let _ = super::config::get_or_prompt_gemini_key(!non_interactive);
    if target.exists() {
        run_organize_cli(&target, false)?;
    }
    Ok(())
}
