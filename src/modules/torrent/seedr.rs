//! Seedr cloud downloader dual-pipeline integration.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::path::Path;
use std::process::Command;
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

    let hash_param = extract_btih_hash(magnet)
        .map(|h| format!("?hash={h}"))
        .unwrap_or_default();
    let callback_url = format!("http://127.0.0.1:{api_port}/seedr-webhook{hash_param}");
    let torrents_dir = Path::new(&home).join("torrents");

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
    let card = crate::notify::client::format_card(
        "Seedr",
        "✅ <b>SEEDR DOWNLOAD COMPLETE</b>",
        &[
            ("File:", file_name),
            ("Size:", &format!("{sz_mb} MB")),
            ("Path:", path_display),
            ("Status:", "Cleaned up from Seedr & qBittorrent"),
        ],
    );

    crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card)?;
    let _ = crate::modules::jellyfin::api::refresh_library_auto();
    Ok(())
}

fn cleanup_qbittorrent(client: &Client, qb_url: &str, hash: Option<&str>, file_name: &str) {
    if let Some(h) = hash {
        let _ = api::delete_torrent(client, qb_url, h, true);
    }

    if let Ok(torrents) = api::get_torrents(client, qb_url, None) {
        for t in torrents {
            let matches_name = t.name == file_name
                || file_name.contains(&t.name)
                || t.name.contains(file_name);
            if matches_name {
                let _ = api::delete_torrent(client, qb_url, &t.hash, true);
            }
        }
    }
}

/// Handles a failed Seedr download notification by alerting that qBittorrent remains active.
///
/// # Errors
/// Returns an error if Telegram alerting fails.
pub fn handle_seedr_failure(
    _hash: Option<&str>,
    error: &str,
    config: &TelegramConfig,
) -> Result<()> {
    let card = crate::notify::client::format_card(
        "Seedr",
        "⚠️ <b>SEEDR CLOUD FAILED</b>",
        &[
            ("Reason:", error),
            ("Action:", "qBittorrent is continuing download as fallback"),
        ],
    );

    crate::notify::client::send_alert(&config.bot_token, &config.chat_id, &card)?;
    Ok(())
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
