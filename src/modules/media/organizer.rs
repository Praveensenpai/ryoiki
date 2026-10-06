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
use std::collections::{BTreeMap, HashMap};

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

    let use_ai = super::config::is_ai_enabled() || api_key.is_some_and(|k| !k.is_empty());

    let batch_ai_map: HashMap<String, super::MediaInfo> = if use_ai && files.len() > 1 {
        let items: Vec<(&str, Option<&MediaProbe>)> = names
            .iter()
            .zip(probes.iter())
            .map(|(n, p)| (*n, p.as_ref()))
            .collect();
        classify_media_batch(client, api_key, &items).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut out = Vec::with_capacity(files.len());
    for (i, file_path) in files.iter().enumerate() {
        let name = names[i];
        let probe = probes[i].clone();

        let mut media_info = if let Some(info) = batch_ai_map.get(name) {
            info.clone()
        } else if use_ai {
            classify_media_ai(client, api_key, name, probe.as_ref())
                .unwrap_or_else(|_| classify_media_heuristic(name))
        } else {
            classify_media_heuristic(name)
        };

        adjust_media_info_post_classify(&mut media_info, probe.as_ref());
        out.push((file_path.clone(), media_info, probe));
    }

    out
}

fn adjust_media_info_post_classify(info: &mut super::MediaInfo, probe: Option<&MediaProbe>) {
    if let Some(p) = probe {
        if p.audio_stream_count <= 1 {
            let lang = p
                .primary_language
                .as_deref()
                .filter(|l| !l.is_empty() && *l != "Multi")
                .map(std::string::ToString::to_string)
                .or_else(|| {
                    info.language
                        .as_deref()
                        .filter(|l| *l != "Multi")
                        .map(std::string::ToString::to_string)
                })
                .unwrap_or_else(|| "Original".to_string());

            info.clean_name = super::ai::ensure_language_in_clean_name(&info.clean_name, &lang);
            info.language = Some(lang);
        } else {
            info.language = Some("Multi".to_string());
            info.clean_name = super::ai::ensure_language_in_clean_name(&info.clean_name, "Multi");
        }
    }

    if info.media_type != MediaType::Anime {
        let is_japanese = probe.is_some_and(|p| {
            p.audio_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case("japanese"))
                || p.primary_language
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case("japanese"))
        }) || info
            .language
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case("japanese"));

        if is_japanese {
            info.media_type = MediaType::Anime;
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
            multi_path: None,
            media_info,
            audio_summary: super::audio::AudioStripSummary::default(),
        });
    }

    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("Failed to create destination dir: {}", dest_dir.display()))?;

    perform_move(file_path, &dest_path)?;

    let has_multiple_audio = probe.is_none_or(|p| p.audio_stream_count > 1);
    let is_anime = matches!(media_info.media_type, MediaType::Anime);
    let (final_dest, multi_path, audio_summary) = if has_multiple_audio {
        let outcome = super::audio::strip_audio_auto(&dest_path, !is_anime);
        let mut summary = super::audio::AudioStripSummary::default();
        summary.record(&outcome);
        match outcome {
            super::audio::StripOutcome::Stripped {
                original_path,
                multi_path,
                ..
            } => (original_path, multi_path, summary),
            _ => (dest_path, None, summary),
        }
    } else {
        (dest_path, None, super::audio::AudioStripSummary::default())
    };

    Ok(OrganizeResult {
        source_path: file_path.to_path_buf(),
        dest_path: final_dest,
        multi_path,
        media_info,
        audio_summary,
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

    // Classify each directory as its own batch. Sibling folders often hold
    // distinct series (a show and its spin-off, bundled companion shorts), and
    // mixing them lets the model merge them under one franchise title.
    for group in group_videos_by_directory(video_files) {
        let classified = classify_video_files(&group, client, api_key);
        for (vf, info, probe) in classified {
            match execute_file_organize(&vf, info, probe.as_ref(), dry_run) {
                Ok(res) => results.push(res),
                Err(e) => eprintln!("  ⚠️ Error organizing {}: {e}", vf.display()),
            }
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

/// Groups video files by their immediate parent directory.
///
/// Files in different folders (e.g. a show and its spin-off) end up in separate
/// groups so they are classified in separate AI batches. A flat torrent keeps
/// all files in one group, preserving the previous single-batch behavior.
fn group_videos_by_directory(files: Vec<PathBuf>) -> Vec<Vec<PathBuf>> {
    let mut groups: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for file in files {
        let key = file.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        groups.entry(key).or_default().push(file);
    }
    groups.into_values().collect()
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

#[cfg(test)]
mod tests;
