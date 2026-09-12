use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};

use super::ai::classify_media_ai;
use super::heuristic::classify_media_heuristic;
use super::{MediaInfo, MediaType, OrganizeResult};
use crate::modules::torrent::api::{self, TorrentInfo};
use crate::runner::Runner;

const VIDEO_EXTENSIONS: [&str; 8] = ["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm", "ts"];

pub fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

pub fn get_jellyfin_media_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join("jellyfin/media")
}

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

    let media_info = match api_key {
        Some(key) if !key.is_empty() => classify_media_ai(client, key, file_name)
            .unwrap_or_else(|_| classify_media_heuristic(file_name)),
        _ => classify_media_heuristic(file_name),
    };

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

    Ok(OrganizeResult {
        source_path: file_path.to_path_buf(),
        dest_path,
        media_info,
    })
}

fn resolve_unique_dest_path(
    src: &Path,
    dest_dir: &Path,
    info: &MediaInfo,
    dry_run: bool,
) -> PathBuf {
    let standard = dest_dir.join(&info.clean_name);
    if !standard.exists() {
        return standard;
    }

    let src_size = fs::metadata(src).map_or(0, |m| m.len());
    let dst_size = fs::metadata(&standard).map_or(0, |m| m.len());

    if src_size.abs_diff(dst_size) < 1024 {
        return standard;
    }

    let ext = standard
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");
    let stem = standard
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&info.clean_name);
    let size_tag = crate::modules::torrent::notify::format_size(src_size).replace(' ', "");

    let new_name = if let Some(prefix) = stem.strip_suffix(']') {
        format!("{prefix} - {size_tag}].{ext}")
    } else {
        format!("{stem} - [{size_tag}].{ext}")
    };

    if !dry_run && !stem.contains(" - ") {
        let old_size_tag = crate::modules::torrent::notify::format_size(dst_size).replace(' ', "");
        let old_name = if let Some(prefix) = stem.strip_suffix(']') {
            format!("{prefix} - {old_size_tag}].{ext}")
        } else {
            format!("{stem} - [{old_size_tag}].{ext}")
        };
        let _ = fs::rename(&standard, dest_dir.join(old_name));
    }

    dest_dir.join(new_name)
}

fn calculate_dest_dir(info: &MediaInfo) -> PathBuf {
    let base = get_jellyfin_media_dir();
    match info.media_type {
        MediaType::Movie => {
            let folder_name = match info.year {
                Some(yr) => format!("{} ({yr})", info.title),
                None => info.title.clone(),
            };
            base.join("movies").join(folder_name)
        }
        MediaType::Show => {
            let season_str = format!("Season {:02}", info.season.unwrap_or(1));
            base.join("shows").join(&info.title).join(season_str)
        }
    }
}

fn perform_move(src: &Path, dst: &Path) -> Result<()> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    fs::copy(src, dst)
        .with_context(|| format!("Failed to copy {} to {}", src.display(), dst.display()))?;
    fs::remove_file(src)
        .with_context(|| format!("Failed to remove source file {}", src.display()))?;
    Ok(())
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

    Ok(results)
}

fn find_videos_recursive(dir: &Path, list: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("Cannot read {}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_videos_recursive(&path, list)?;
        } else if is_video_file(&path) {
            list.push(path);
        }
    }
    Ok(())
}

pub fn organize_completed_torrent(
    client: &Client,
    torrent: &TorrentInfo,
    api_key: Option<&str>,
) -> Result<Option<OrganizeResult>> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default_dl = Path::new(&home).join("torrents");

    let source = if let Some(content_path) = &torrent.content_path {
        let rel = content_path
            .trim_start_matches("/downloads/")
            .trim_start_matches('/');
        default_dl.join(rel)
    } else if let Some(save_path) = &torrent.save_path {
        let rel = save_path
            .trim_start_matches("/downloads/")
            .trim_start_matches('/');
        default_dl.join(rel).join(&torrent.name)
    } else {
        default_dl.join(&torrent.name)
    };

    if !source.exists() {
        return Ok(None);
    }

    let organized = organize_path(&source, client, api_key, false)?;
    Ok(organized.into_iter().next())
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

    let results = organize_path(target, &client, api_key.as_deref(), dry_run)?;
    if results.is_empty() {
        println!("  {} No new video files found to organize.", "•".dimmed());
    } else {
        println!(
            "\n  {} Successfully organized {} item(s):\n",
            "✔".green().bold(),
            results.len()
        );
        for res in &results {
            println!(
                "  • {} ({})",
                res.media_info.clean_name.bold(),
                res.media_info.engine
            );
            println!(
                "    {} -> {}",
                res.source_path.display().to_string().dimmed(),
                res.dest_path.display().to_string().cyan()
            );
        }
        println!();
    }

    let qb_url = crate::notify::TelegramConfig::load().map_or_else(
        |_| "http://localhost:6881".to_string(),
        |c| c.qbittorrent_url,
    );
    let cleared = cleanup_matching_torrents(&client, &qb_url, &results);
    if cleared > 0 {
        println!(
            "  {} Removed {} completed torrent(s) from qBittorrent history",
            "🗑".green().bold(),
            cleared
        );
    }

    Ok(())
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
        if t.progress < 1.0 {
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
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let api_key = super::config::get_or_prompt_gemini_key(false);
        let results = organize_path(&target, &client, api_key.as_deref(), false)?;
        if !results.is_empty() {
            println!(
                "  {} Organized {} item(s) into Jellyfin",
                "✔".green().bold(),
                results.len()
            );
        }

        let qb_url = crate::notify::TelegramConfig::load().map_or_else(
            |_| "http://localhost:6881".to_string(),
            |c| c.qbittorrent_url,
        );
        let cleared = cleanup_matching_torrents(&client, &qb_url, &results);
        if cleared > 0 {
            println!(
                "  {} Removed {} completed torrent(s) from qBittorrent history",
                "🗑".green().bold(),
                cleared
            );
        }
    }
    Ok(())
}
