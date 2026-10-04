//! Seedr cloud downloader dual-pipeline integration.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::api;
use super::telegram::TelegramConfig;

/// Extracts the BTIH hash from a magnet link if present.
#[must_use]
pub fn extract_btih_hash(magnet: &str) -> Option<String> {
    let trimmed = magnet.trim();
    if trimmed.len() == 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(trimmed.to_lowercase());
    }

    let prefix = "urn:btih:";
    let idx = trimmed.find(prefix)?;
    let sub = &trimmed[idx + prefix.len()..];
    let end = sub.find('&').unwrap_or(sub.len());
    let hash = &sub[..end];
    if hash.is_empty() {
        None
    } else {
        Some(hash.to_lowercase())
    }
}

fn get_magnets_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".cache/seedr-dl/magnets")
}

pub fn save_pending_magnet(hash: &str, magnet: &str) {
    let dir = get_magnets_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(format!("{hash}.magnet")), magnet);
    }
}

pub fn load_pending_magnet(hash: &str) -> Option<String> {
    let path = get_magnets_dir().join(format!("{hash}.magnet"));
    std::fs::read_to_string(path)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

pub fn remove_pending_magnet(hash: &str) {
    let path = get_magnets_dir().join(format!("{hash}.magnet"));
    let _ = std::fs::remove_file(path);
}

/// Spawns a background seedr-dl process configured with the webhook callback.
///
/// # Errors
/// Returns an error if seedr-dl is not installed or process spawning fails.
pub fn spawn_seedr_download(magnet: &str, api_port: u16) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bin_path = Path::new(&home).join(".local/bin/seedr-dl");
    let exe = if bin_path.exists() {
        bin_path.to_string_lossy().to_string()
    } else {
        "seedr-dl".to_string()
    };

    let hash_opt = extract_btih_hash(magnet);
    if let Some(ref h) = hash_opt {
        save_pending_magnet(h, magnet);
    }

    let hash_param = hash_opt.map(|h| format!("?hash={h}")).unwrap_or_default();
    let callback_url = format!("http://127.0.0.1:{api_port}/seedr-webhook{hash_param}");
    let torrents_dir = Path::new(&home).join("torrents");
    let log_dir = Path::new(&home).join(".cache/seedr-dl/logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("seedr-daemon.log");
    let log_out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_or_else(|_| Stdio::null(), Stdio::from);
    let log_err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_or_else(|_| Stdio::null(), Stdio::from);

    Command::new(exe)
        .args([
            "download",
            magnet,
            "--background",
            "--yes",
            "--callback-url",
            &callback_url,
            "-o",
            &torrents_dir.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(log_out)
        .stderr(log_err)
        .spawn()
        .context("Failed to spawn seedr-dl background worker")?;

    Ok(())
}

/// Handles a successful Seedr download notification by cleaning up from qBittorrent and alerting.
///
/// # Errors
/// Returns an error if network dispatch or Telegram alerting fails.
pub fn handle_seedr_completion(
    hash: Option<&str>,
    file_name: &str,
    total_bytes: u64,
    dest_path: Option<&str>,
    config: &TelegramConfig,
) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client for qBittorrent cleanup")?;

    cleanup_qbittorrent(&client, &config.qbittorrent_url, hash, file_name);

    let sz_mb = total_bytes / 1_048_576;
    let path_display = dest_path.unwrap_or("torrents/");
    let display_name = if file_name.starts_with("folder-") {
        dest_path
            .and_then(|p| Path::new(p).file_name()?.to_str())
            .unwrap_or(file_name)
    } else {
        file_name
    };
    let (organized_count, organized_results) =
        if let Some(target) = resolve_seedr_target(file_name, dest_path) {
            let api_key = config.gemini_api_key.as_deref();
            crate::modules::media::organizer::organize_path(&target, &client, api_key, false)
                .map_or((0, Vec::new()), |r| (r.len(), r))
        } else {
            (0, Vec::new())
        };

    if let Some(first) = organized_results.into_iter().next() {
        let _ = super::dedup::record_download_history(hash, &first.media_info, &[first.dest_path]);
    }

    let status_msg = if organized_count > 0 {
        format!("Organized {organized_count} file(s) into Jellyfin")
    } else {
        "Cleaned up from Seedr & qBittorrent".to_string()
    };

    let card = crate::notify::client::format_card(
        "Seedr",
        "✅ <b>SEEDR DOWNLOAD COMPLETE</b>",
        &[
            ("File:", display_name),
            ("Size:", &format!("{sz_mb} MB")),
            ("Path:", path_display),
            ("Status:", &status_msg),
        ],
    );

    if let Some(h) = hash {
        remove_pending_magnet(h);
    }

    crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card)?;
    let _ = crate::modules::jellyfin::api::refresh_library_auto();
    Ok(())
}

fn resolve_seedr_target(file_name: &str, dest_path: Option<&str>) -> Option<PathBuf> {
    if let Some(dp) = dest_path {
        let p = PathBuf::from(dp);
        if p.exists() {
            return Some(p);
        }
    }

    let home = std::env::var("HOME").ok()?;
    let torrents = Path::new(&home).join("torrents");
    let candidate = torrents.join(file_name);
    if candidate.exists() {
        return Some(candidate);
    }

    if let Some(stripped) = file_name.strip_prefix("folder-") {
        let folder_candidate = torrents.join(stripped);
        if folder_candidate.exists() {
            return Some(folder_candidate);
        }
    }

    None
}

fn cleanup_qbittorrent(client: &Client, qb_url: &str, hash: Option<&str>, file_name: &str) {
    if let Some(h) = hash {
        let _ = api::delete_torrent(client, qb_url, h, true);
    }

    if let Ok(torrents) = api::get_torrents(client, qb_url, None) {
        let clean_target = file_name.strip_prefix("folder-").unwrap_or(file_name);
        let target_info = crate::modules::media::heuristic::classify_media_heuristic(clean_target);

        for t in torrents {
            let hash_match = hash.is_some_and(|h| t.hash.eq_ignore_ascii_case(h));
            let name_match = is_same_media(&t.name, clean_target, &target_info);

            if hash_match || name_match {
                let _ = api::delete_torrent(client, qb_url, &t.hash, true);
            }
        }
    }
}

fn is_same_media(
    torrent_name: &str,
    file_name: &str,
    file_info: &crate::modules::media::MediaInfo,
) -> bool {
    if torrent_name.eq_ignore_ascii_case(file_name)
        || torrent_name.contains(file_name)
        || file_name.contains(torrent_name)
    {
        return true;
    }

    let t_info = crate::modules::media::heuristic::classify_media_heuristic(torrent_name);
    if !t_info.title.is_empty()
        && t_info.title.eq_ignore_ascii_case(&file_info.title)
        && t_info.media_type == file_info.media_type
    {
        if t_info.season.is_some()
            && t_info.season == file_info.season
            && t_info.episode == file_info.episode
        {
            return true;
        }
        if t_info.year.is_some() && t_info.year == file_info.year {
            return true;
        }
    }
    false
}

/// Handles a failed Seedr download notification by automatically queueing into qBittorrent.
///
/// # Errors
/// Returns an error if Telegram alerting fails.
pub fn handle_seedr_failure(
    hash: Option<&str>,
    error: &str,
    config: &TelegramConfig,
) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client for qBittorrent fallback")?;

    let fallback_ok = if let Some(h) = hash {
        let magnet = load_pending_magnet(h).unwrap_or_else(|| format!("magnet:?xt=urn:btih:{h}"));
        let res = api::add_magnet(&client, &config.qbittorrent_url, &magnet);
        remove_pending_magnet(h);
        res.is_ok()
    } else {
        false
    };

    let action_msg = if fallback_ok {
        "Forwarded to qBittorrent for local download"
    } else {
        "Failed to forward to qBittorrent"
    };

    let card = crate::notify::client::format_card(
        "Seedr",
        "⚠️ <b>SEEDR CLOUD FAILED — FALLBACK ACTIVATED</b>",
        &[("Reason:", error), ("Fallback:", action_msg)],
    );

    crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card)?;
    Ok(())
}

pub use super::seedr_tasks::{format_seedr_tasks_section, get_active_seedr_tasks};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_btih_hash() {
        let magnet = "magnet:?xt=urn:btih:8A47A4DAC599B86BB196DE26F7EFFA2D7C0B9E1C&dn=Test";
        let hash = extract_btih_hash(magnet);
        assert_eq!(
            hash.as_deref(),
            Some("8a47a4dac599b86bb196de26f7effa2d7c0b9e1c")
        );
    }

    #[test]
    fn test_cleanup_qbittorrent_removes_matching_torrent() {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        let qb_url = "http://localhost:6881";
        if api::get_torrents(&client, qb_url, None).is_ok() {
            cleanup_qbittorrent(
                &client,
                qb_url,
                Some("3333333333333333333333333333333333333333"),
                "Karakuri Test Drive S01E01.mkv",
            );
            cleanup_qbittorrent(&client, qb_url, None, "Akkun to Kanojo S01E01.mkv");
            let after = api::get_torrents(&client, qb_url, None).unwrap_or_default();
            assert!(!after
                .iter()
                .any(|t| t.hash == "3333333333333333333333333333333333333333"));
            assert!(!after
                .iter()
                .any(|t| t.hash == "1111111111111111111111111111111111111111"));
        }
    }
}
