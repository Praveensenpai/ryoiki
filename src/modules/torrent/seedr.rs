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

    if !organized_results.is_empty() {
        let mut tracked_files = Vec::new();
        for org in &organized_results {
            tracked_files.push(super::history::create_tracked_file(
                org.dest_path.clone(),
                "original",
            ));
            if let Some(ref mp) = org.multi_path {
                tracked_files.push(super::history::create_tracked_file(mp.clone(), "multi"));
            }
        }
        if let Some(first) = organized_results.first() {
            let _ = super::history::record_download_history(hash, &first.media_info, tracked_files);
        }
    }

    let cloud_result = cleanup_seedr_cloud(file_name);
    let cloud_msg = match &cloud_result {
        Ok(()) => "Removed from Seedr cloud".to_string(),
        Err(e) => format!("Cloud cleanup failed: {e}"),
    };

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
            ("Cloud:", &cloud_msg),
        ],
    );

    if let Some(h) = hash {
        remove_pending_magnet(h);
    }

    crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card)?;
    let _ = crate::modules::jellyfin::api::refresh_library_auto();

    if let Some(h) = hash {
        super::scheduler::handle_seedr_done(h, config);
    }
    Ok(())
}

/// Removes a completed item's folder from the Seedr cloud account.
///
/// Resolves the live folder/torrent ID by name (tolerating the `folder-`
/// prefix) so cleanup never depends on transient local task JSON, which is
/// already gone by the time the completion webhook fires.
///
/// # Errors
/// Returns a human-readable message when Seedr is unreachable or deletion fails.
pub fn cleanup_seedr_cloud(file_name: &str) -> Result<(), String> {
    let Some(list) = super::seedr_tasks::fetch_live_list() else {
        return Err("Seedr cloud unreachable".to_string());
    };

    let target = super::seedr_tasks::normalize(file_name);
    let stripped = target.strip_prefix("folder-").unwrap_or(&target);

    let matched = list
        .folders
        .iter()
        .map(|f| (f.id, &f.name))
        .chain(list.files.iter().map(|f| (f.id, &f.name)))
        .chain(list.torrents.iter().map(|t| (t.id, &t.name)))
        .find(|(_, name)| {
            let n = super::seedr_tasks::normalize(name);
            n == target || n == stripped
        });

    let Some((id, _)) = matched else {
        return Ok(());
    };

    let output = Command::new("seedr-dl")
        .args(["delete", &id.to_string(), "-y"])
        .output()
        .map_err(|e| format!("Failed to spawn seedr-dl delete: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "seedr-dl delete returned a non-zero exit".to_string()
        } else {
            detail
        })
    }
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
        let stem = Path::new(clean_target)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(clean_target);

        for t in torrents {
            let hash_match = hash.is_some_and(|h| t.hash.eq_ignore_ascii_case(h));
            let name_match = t.name.eq_ignore_ascii_case(clean_target)
                || t.name.eq_ignore_ascii_case(stem)
                || t.name.eq_ignore_ascii_case(file_name);

            if hash_match || name_match {
                let _ = api::delete_torrent(client, qb_url, &t.hash, true);
            }
        }
    }
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

    if let Some(h) = hash {
        super::scheduler::handle_seedr_failure(h, config);
    }
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
