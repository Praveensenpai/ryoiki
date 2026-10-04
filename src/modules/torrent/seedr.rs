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
    let prefix = "xt=urn:btih:";
    let idx = magnet.find(prefix)?;
    let sub = &magnet[idx + prefix.len()..];
    let end = sub.find('&').unwrap_or(sub.len());
    Some(sub[..end].to_lowercase())
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
    let organized_count = if let Some(target) = resolve_seedr_target(file_name, dest_path) {
        let api_key = config.gemini_api_key.as_deref();
        crate::modules::media::organizer::organize_path(&target, &client, api_key, false)
            .map_or(0, |r| r.len())
    } else {
        0
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
        for t in torrents {
            let matches_name =
                t.name == file_name || file_name.contains(&t.name) || t.name.contains(file_name);
            if matches_name {
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
    Ok(())
}

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct SeedrTaskState {
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: u64,
    pub status: String,
}

#[derive(Deserialize)]
struct SeedrCloudList {
    #[serde(default)]
    torrents: Vec<SeedrCloudTorrent>,
}

#[derive(Deserialize)]
struct SeedrCloudTorrent {
    name: String,
    #[serde(default)]
    progress: Option<f64>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    download_rate: Option<u64>,
}

#[must_use]
pub fn get_active_seedr_tasks() -> Vec<SeedrTaskState> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let tasks_dir = Path::new(&home).join(".cache/seedr-dl/tasks");
    let mut tasks = Vec::new();

    if let Ok(entries) = std::fs::read_dir(tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<SeedrTaskState>(&content) {
                        tasks.push(task);
                    }
                }
            }
        }
    }

    if tasks.is_empty() {
        if let Ok(output) = std::process::Command::new("seedr-dl")
            .args(["list", "--json"])
            .output()
        {
            if output.status.success() {
                if let Ok(cloud) = serde_json::from_slice::<SeedrCloudList>(&output.stdout) {
                    for t in cloud.torrents {
                        let total = t.size.unwrap_or(0);
                        let pct = t.progress.unwrap_or(0.0);
                        #[allow(
                            clippy::cast_precision_loss,
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss
                        )]
                        let downloaded = if total > 0 && pct > 0.0 {
                            ((pct / 100.0) * (total as f64)) as u64
                        } else {
                            0
                        };
                        let speed = t.download_rate.unwrap_or(0);
                        let eta = if speed > 0 && total > downloaded {
                            (total - downloaded) / speed
                        } else {
                            0
                        };
                        tasks.push(SeedrTaskState {
                            file_name: t.name,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                            speed_bps: speed,
                            eta_seconds: eta,
                            status: "Caching".to_string(),
                        });
                    }
                }
            }
        }
    }

    tasks
}

#[must_use]
pub fn format_seedr_tasks_section(tasks: &[SeedrTaskState]) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    let mut lines = vec!["🌱 <b>Seedr Cloud Downloads:</b>".to_string()];
    for t in tasks {
        #[allow(clippy::cast_precision_loss)]
        let pct = if t.total_bytes > 0 {
            (t.downloaded_bytes as f64 / t.total_bytes as f64) * 100.0
        } else {
            0.0
        };
        let blocks = format!("{:.0}", pct / 10.0)
            .parse::<usize>()
            .unwrap_or(0)
            .min(10);
        let bar = format!(
            "[{}{}] {pct:.1}%",
            "█".repeat(blocks),
            "░".repeat(10 - blocks)
        );
        let dl_mb = t.downloaded_bytes / 1_048_576;
        let tot_mb = t.total_bytes / 1_048_576;
        #[allow(clippy::cast_precision_loss)]
        let spd = t.speed_bps as f64 / 1_048_576.0;
        let clean_name = crate::notify::client::escape_html(&t.file_name);
        let (icon, label) = if t.status.eq_ignore_ascii_case("caching") {
            ("☁️", "Caching in Seedr Cloud")
        } else if t.status.eq_ignore_ascii_case("organizing") {
            ("📁", "Organizing Media")
        } else {
            ("📥", "Downloading to Disk")
        };
        lines.push(format!(
            "{icon} <b>{clean_name}</b>\n<code>{bar}</code> • <b>{label}</b>\nSize: {dl_mb}/{tot_mb} MB | Rate: {spd:.2} MB/s | ETA: {}s\n",
            t.eta_seconds
        ));
    }
    lines.join("\n")
}

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
}
