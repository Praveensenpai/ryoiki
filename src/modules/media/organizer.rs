pub mod cli;
pub mod pathing;

pub use cli::{run_organize_cli, setup};
pub use pathing::{calculate_dest_dir, is_video_file, perform_move, resolve_unique_dest_path};

use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};

use super::ai::{classify_media_ai, classify_media_batch};
use super::heuristic::classify_media_heuristic;
use super::probe::MediaProbe;
use super::{MediaType, OrganizeResult};
use crate::modules::torrent::api::TorrentInfo;
use std::collections::HashMap;

pub fn organize_file(
    file_path: &Path,
    client: &Client,
    api_key: Option<&str>,
    dry_run: bool,
) -> Result<OrganizeResult> {
    let classified = classify_video_files(&[file_path.to_path_buf()], client, api_key);
    let Some((_, info, probe)) = classified.into_iter().next() else {
        anyhow::bail!("Failed to classify file: {}", file_path.display());
    };
    execute_file_organize(file_path, info, probe.as_ref(), dry_run)
}

fn classify_video_files(
    files: &[PathBuf],
    client: &Client,
    api_key: Option<&str>,
) -> Vec<(PathBuf, super::MediaInfo, Option<MediaProbe>)> {
    if files.is_empty() {
        return Vec::new();
    }

    let probes: Vec<Option<MediaProbe>> = files
        .iter()
        .map(|f| super::probe::probe_media_file(f))
        .collect();
    let names: Vec<&str> = files
        .iter()
        .map(|f| f.file_name().and_then(|n| n.to_str()).unwrap_or(""))
        .collect();

    let batch_ai_map: HashMap<String, super::MediaInfo> = match api_key {
        Some(key) if !key.is_empty() && files.len() > 1 => {
            let items: Vec<(&str, Option<&MediaProbe>)> = names
                .iter()
                .zip(probes.iter())
                .map(|(n, p)| (*n, p.as_ref()))
                .collect();
            classify_media_batch(client, key, &items).unwrap_or_default()
        }
        _ => HashMap::new(),
    };

    let mut out = Vec::with_capacity(files.len());
    for (i, file_path) in files.iter().enumerate() {
        let name = names[i];
        let probe = probes[i].clone();

        let mut media_info = if let Some(info) = batch_ai_map.get(name) {
            info.clone()
        } else {
            match api_key {
                Some(key) if !key.is_empty() => {
                    classify_media_ai(client, key, name, probe.as_ref())
                        .unwrap_or_else(|_| classify_media_heuristic(name))
                }
                _ => classify_media_heuristic(name),
            }
        };

        adjust_media_info_post_classify(&mut media_info, probe.as_ref());
        out.push((file_path.clone(), media_info, probe));
    }

    out
}

fn adjust_media_info_post_classify(info: &mut super::MediaInfo, probe: Option<&MediaProbe>) {
    if info.language.is_none() {
        if let Some(p) = probe {
            if let Some(primary) = &p.primary_language {
                info.language = Some(primary.clone());
                info.clean_name =
                    super::ai::ensure_language_in_clean_name(&info.clean_name, primary);
            }
        }
    }

    if info.media_type != MediaType::Anime {
        if let Some(lang) = &info.language {
            if lang.eq_ignore_ascii_case("japanese") {
                info.media_type = MediaType::Anime;
            }
        }
    }
}

pub fn execute_file_organize(
    file_path: &Path,
    media_info: super::MediaInfo,
    probe: Option<&MediaProbe>,
    dry_run: bool,
) -> Result<OrganizeResult> {
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

    let has_multiple_audio = probe.is_none_or(|p| p.audio_stream_count > 1);
    if has_multiple_audio {
        super::audio::strip_audio_auto(&dest_path);
    }

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

    let classified = classify_video_files(&video_files, client, api_key);
    for (vf, info, probe) in classified {
        match execute_file_organize(&vf, info, probe.as_ref(), dry_run) {
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
        if !content_path.trim().is_empty() {
            let rel = content_path
                .trim_start_matches("/downloads/")
                .trim_start_matches('/');
            let p = default_dl.join(rel);
            if p.exists() {
                return p;
            }
        }
    }

    if let Some(save_path) = &torrent.save_path {
        if !save_path.trim().is_empty() {
            let rel = save_path
                .trim_start_matches("/downloads/")
                .trim_start_matches('/');
            let p = default_dl.join(rel).join(&torrent.name);
            if p.exists() {
                return p;
            }
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
